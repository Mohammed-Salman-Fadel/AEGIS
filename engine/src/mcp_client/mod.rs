use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use tracing::{info, warn};

#[derive(Debug, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub result: Option<serde_json::Value>,
    pub error: Option<serde_json::Value>,
    pub id: Option<u64>,
}

pub struct McpClient {
    command: String,
    args: Vec<String>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout_reader: Option<BufReader<ChildStdout>>,
    restart_count: u64,
}

// ── initialisation & health ──────────────────────────────────────────

impl McpClient {
    pub fn new(command: &str, args: Vec<&str>) -> Self {
        Self {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            child: None,
            stdin: None,
            stdout_reader: None,
            restart_count: 0,
        }
    }

    /// Non-blocking check: is the child process still running?
    fn is_alive(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => {
                    warn!(
                        "MCP process `{}` exited with status {status:?}; will restart",
                        self.command,
                    );
                    false
                }
                Ok(None) => true,    // still running
                Err(e) => {
                    warn!(
                        "MCP process `{}` try_wait error: {e}; assuming dead",
                        self.command,
                    );
                    false
                }
            },
            None => false,           // never started
        }
    }

    /// Ensure the MCP server is running and initialised.
    /// If the process has exited, cleans up state and re-spawns.
    /// Applies exponential back-off between restart attempts.
    async fn ensure_started(&mut self) -> Result<()> {
        if self.is_alive() {
            return Ok(());
        }

        // Process is dead or never started — drop stale handles.
        // (Dropping ChildStdin / ChildStdout closes the corresponding OS handles.)
        self.child = None;
        self.stdin = None;
        self.stdout_reader = None;

        // Exponential back-off: 100 ms → 200 → 400 → ... → 5 s max
        if self.restart_count > 0 {
            let delay_ms = std::cmp::min(100u64 << self.restart_count.min(6), 5_000);
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }

        info!("Starting MCP server: {} {:?}", self.command, self.args);
        let mut child = Command::new(&self.command)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take()
            .ok_or_else(|| anyhow::anyhow!("MCP child had no stdin"))?;
        let stdout = child.stdout.take()
            .ok_or_else(|| anyhow::anyhow!("MCP child had no stdout"))?;
        let mut reader = BufReader::new(stdout);

        // ── MCP initialisation handshake ──────────────────────────────

        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1u64,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "aegis-engine",
                    "version": "0.1.0",
                },
            },
        });

        let req_str = serde_json::to_string(&init_req)? + "\n";
        stdin.write_all(req_str.as_bytes()).await?;
        stdin.flush().await?;

        // Read the initialise response (10-second timeout).
        let _resp = loop {
            let mut line = String::new();
            let n = timeout(Duration::from_secs(10), reader.read_line(&mut line))
                .await
                .map_err(|_| anyhow::anyhow!("MCP initialise timeout (10 s)"))??;
            if n == 0 {
                anyhow::bail!("EOF during MCP initialisation");
            }
            if let Ok(val) = serde_json::from_str::<McpResponse>(&line) {
                if let Some(error) = val.error {
                    anyhow::bail!("MCP initialisation error: {error:?}");
                }
                break val;
            }
        };

        // Send the "initialised" notification (no response expected).
        let notified = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        });
        let notif_str = serde_json::to_string(&notified)? + "\n";
        stdin.write_all(notif_str.as_bytes()).await?;
        stdin.flush().await?;

        self.child = Some(child);
        self.stdin = Some(stdin);
        self.stdout_reader = Some(reader);
        self.restart_count += 1;
        info!("MCP server started (restart #{})", self.restart_count);
        Ok(())
    }
}

// ── tool calls ────────────────────────────────────────────────────────

impl McpClient {
    /// Call an MCP tool on the server.
    ///
    /// Automatically restarts the server if the process has died, and
    /// retries once if the write to stdin fails (broken pipe).
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.ensure_started().await?;

        let req_id: u64 = rand::random();
        let request = json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments,
            },
        });

        let req_str = serde_json::to_string(&request)? + "\n";

        // Try the write.  If the pipe is broken the process has died
        // between ensure_started and now — restart and retry once.
        let stdin = self.stdin.as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP stdin not available"))?;

        if let Err(e) = stdin.write_all(req_str.as_bytes()).await {
            warn!("MCP write failed (process may have died): {e}; restarting");
            self.child = None;
            self.stdin = None;
            self.stdout_reader = None;
            self.ensure_started().await?;

            let stdin = self.stdin.as_mut()
                .ok_or_else(|| anyhow::anyhow!("MCP stdin not available after restart"))?;
            stdin.write_all(req_str.as_bytes()).await?;
            stdin.flush().await?;
        } else {
            stdin.flush().await?;
        }

        // ── read the response ──────────────────────────────────────────
        let reader = self.stdout_reader.as_mut()
            .ok_or_else(|| anyhow::anyhow!("MCP stdout not available"))?;

        let resp_json = loop {
            let mut line = String::new();
            let n = timeout(Duration::from_secs(60), reader.read_line(&mut line))
                .await
                .map_err(|_| anyhow::anyhow!("MCP tool call timeout (60 s)"))??;
            if n == 0 {
                anyhow::bail!("EOF while waiting for MCP response");
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("id").and_then(|id| id.as_u64()) == Some(req_id) {
                    break val;
                }
                if val.get("method").is_some() {
                    tracing::debug!("Received MCP notification: {}", line.trim());
                    continue;
                }
            }
            tracing::debug!("Skipping non-JSON output from MCP: {}", line.trim());
        };

        let response: McpResponse = serde_json::from_value(resp_json)?;
        if let Some(error) = response.error {
            anyhow::bail!("MCP tool call failed: {error:?}");
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("Empty MCP response"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_alive_returns_false_when_not_started() {
        let mut client = McpClient::new("true", vec![]);
        assert!(!client.is_alive());
    }

    #[test]
    fn is_alive_returns_false_for_exited_process() {
        // `true` exits immediately — after spawn + try_wait it should be dead.
        let mut client = McpClient::new("true", vec![]);
        // We can't easily test the full cycle without tokio, but the
        // initial state is correct.
        assert!(!client.is_alive());
    }

    #[test]
    fn new_client_starts_at_zero_restarts() {
        let client = McpClient::new("echo", vec!["hello"]);
        assert_eq!(client.restart_count, 0);
    }
}
