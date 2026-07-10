//! MCP JSON-RPC client for subprocess-based MCP servers.
//! Manages the lifecycle of an MCP server subprocess, performing the initialization
//! handshake and providing a `call_tool` method for invoking tools.

use anyhow::Result;
use serde_json::json;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::info;

use super::types::JsonRpcResponse;

/// A client that communicates with an MCP server via stdin/stdout JSON-RPC.
pub struct McpClient {
    command: String,
    args: Vec<String>,
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    stdout_reader: Option<BufReader<ChildStdout>>,
}

impl McpClient {
    pub fn new(command: &str, args: Vec<String>) -> Self {
        Self {
            command: command.to_string(),
            args,
            child: None,
            stdin: None,
            stdout_reader: None,
        }
    }

    pub async fn ensure_started(&mut self) -> Result<()> {
        if self.child.is_some() {
            return Ok(());
        }

        info!("Starting MCP server: {} {:?}", self.command, self.args);
        let mut child = Command::new(&self.command)
            .args(&self.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        // MCP initialization handshake
        let init_req = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "aegis-engine",
                    "version": "0.1.0"
                }
            }
        });

        let req_str = serde_json::to_string(&init_req)? + "\n";
        let mut stdin_owned = stdin;
        stdin_owned.write_all(req_str.as_bytes()).await?;
        stdin_owned.flush().await?;

        // Read response with a generous timeout (npx may need to download the package on first run)
        tokio::time::timeout(std::time::Duration::from_secs(60), async {
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader.read_line(&mut line).await
                    .map_err(|e| anyhow::anyhow!("Failed to read MCP response: {}", e))?;
                if n == 0 {
                    anyhow::bail!("EOF during MCP initialization");
                }
                if let Ok(val) = serde_json::from_str::<JsonRpcResponse>(&line) {
                    if let Some(error) = val.error {
                        anyhow::bail!("MCP initialization error: {:?}", error);
                    }
                    return Ok::<_, anyhow::Error>(());
                }
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("MCP initialization timed out after 60 seconds. The obsidian-mcp subprocess may still be starting."))?
        .map_err(|e: anyhow::Error| e)?;

        // Send initialized notification
        let notif = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let notif_str = serde_json::to_string(&notif)? + "\n";
        stdin_owned.write_all(notif_str.as_bytes()).await?;
        stdin_owned.flush().await?;

        self.child = Some(child);
        self.stdin = Some(stdin_owned);
        self.stdout_reader = Some(reader);
        Ok(())
    }

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
                "arguments": arguments
            }
        });

        let stdin = self.stdin.as_mut().unwrap();
        let reader = self.stdout_reader.as_mut().unwrap();

        let req_str = serde_json::to_string(&request)? + "\n";
        stdin.write_all(req_str.as_bytes()).await?;
        stdin.flush().await?;

        // Read response with 30-second timeout — accumulate lines until we find a matching JSON-RPC response
        let resp_json = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            let mut json_buf = String::new();
            loop {
                let mut line = String::new();
                let n = reader.read_line(&mut line).await?;
                if n == 0 {
                    anyhow::bail!("EOF while waiting for MCP response");
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                // Accumulate into buffer to handle multi-line JSON
                json_buf.push_str(trimmed);

                // Try to parse whatever we have as JSON
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_buf) {
                    if val.get("id").and_then(|id| id.as_u64()) == Some(req_id) {
                        return Ok(val);
                    } else if val.get("method").is_some() {
                        tracing::debug!("MCP notification: {}", trimmed);
                        json_buf.clear();
                        continue;
                    }
                }
                // If the buffer doesn't end with '}' yet, it might be multi-line — keep reading
                if !json_buf.ends_with('}') {
                    continue;
                }
                // Failed to parse so far — reset and keep trying
                tracing::debug!("Skipping non-JSON MCP output: {}", trimmed);
                json_buf.clear();
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("MCP tool call timed out after 30 seconds"))?
        .map_err(|e: anyhow::Error| e)?;

        let response: JsonRpcResponse = serde_json::from_value(resp_json)?;
        if let Some(error) = response.error {
            anyhow::bail!("MCP tool call failed: {}", error.message);
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("Empty MCP response"))
    }

    pub async fn list_tools(&mut self) -> Result<Vec<super::types::McpTool>> {
        self.ensure_started().await?;

        let req_id: u64 = rand::random();
        let request = json!({
            "jsonrpc": "2.0",
            "id": req_id,
            "method": "tools/list"
        });

        let stdin = self.stdin.as_mut().unwrap();
        let reader = self.stdout_reader.as_mut().unwrap();

        let req_str = serde_json::to_string(&request)? + "\n";
        stdin.write_all(req_str.as_bytes()).await?;
        stdin.flush().await?;

        let resp_json = tokio::time::timeout(std::time::Duration::from_secs(30), async {
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader
                    .read_line(&mut line)
                    .await
                    .map_err(|e| anyhow::anyhow!("Failed to read MCP tools response: {}", e))?;
                if n == 0 {
                    anyhow::bail!("EOF while listing MCP tools");
                }
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                    if val.get("id").and_then(|id| id.as_u64()) == Some(req_id) {
                        return Ok::<_, anyhow::Error>(val);
                    }
                }
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("MCP list_tools timed out after 30 seconds"))?
        .map_err(|e: anyhow::Error| e)?;

        let response: JsonRpcResponse = serde_json::from_value(resp_json)?;
        if let Some(error) = response.error {
            anyhow::bail!("MCP list_tools failed: {}", error.message);
        }

        let tools = response
            .result
            .and_then(|r| r.get("tools").cloned())
            .and_then(|t| serde_json::from_value::<Vec<super::types::McpTool>>(t).ok())
            .unwrap_or_default();

        Ok(tools)
    }

    /// Gracefully shut down the MCP server subprocess.
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(mut child) = self.child.take() {
            child.kill().await.ok();
            child.wait().await.ok();
        }
        self.stdin = None;
        self.stdout_reader = None;
        Ok(())
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            let pid = child.id();
            #[cfg(windows)]
            {
                // On Windows, cmd /c spawns a process tree (cmd → npx → node).
                // start_kill() only kills cmd.exe, leaving npx/node orphaned.
                // Use taskkill /T to terminate the entire tree.
                if let Some(id) = pid {
                    let _ = std::process::Command::new("taskkill")
                        .args(&["/F", "/T", "/PID", &id.to_string()])
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .status();
                }
            }
            #[cfg(not(windows))]
            {
                let _ = child.start_kill();
            }
        }
    }
}
