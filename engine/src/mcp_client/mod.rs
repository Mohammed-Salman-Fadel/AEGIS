use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tracing::info;

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
}

impl McpClient {
    pub fn new(command: &str, args: Vec<&str>) -> Self {
        Self {
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            child: None,
            stdin: None,
            stdout_reader: None,
        }
    }

    async fn ensure_started(&mut self) -> Result<()> {
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

        let mut stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = BufReader::new(stdout);

        // Perform MCP initialization handshake
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
        stdin.write_all(req_str.as_bytes()).await?;
        stdin.flush().await?;

        // Read response, skipping non-JSON lines
        let _resp = loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;
            if n == 0 { anyhow::bail!("EOF during MCP initialization"); }
            if let Ok(val) = serde_json::from_str::<McpResponse>(&line) {
                if let Some(error) = val.error {
                    anyhow::bail!("MCP initialization error: {:?}", error);
                }
                break val;
            }
        };
        
        // Send initialized notification
        let notified = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });
        let notif_str = serde_json::to_string(&notified)? + "\n";
        stdin.write_all(notif_str.as_bytes()).await?;
        stdin.flush().await?;

        self.child = Some(child);
        self.stdin = Some(stdin);
        self.stdout_reader = Some(reader);
        Ok(())
    }

    pub async fn call_tool(&mut self, tool_name: &str, arguments: serde_json::Value) -> Result<serde_json::Value> {
        self.ensure_started().await?;
        
        let req_id = rand::random::<u64>();
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

        // Read response, skipping non-JSON lines (like warnings)
        let resp_json = loop {
            let mut line = String::new();
            let n = reader.read_line(&mut line).await?;
            if n == 0 { anyhow::bail!("EOF while waiting for MCP response"); }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                if val.get("id").and_then(|id| id.as_u64()) == Some(req_id) {
                    break val;
                } else if val.get("method").is_some() {
                    tracing::debug!("Received MCP notification: {}", line.trim());
                    continue;
                }
            }
            tracing::debug!("Skipping non-JSON output from MCP: {}", line.trim());
        };
        
        let response: McpResponse = serde_json::from_value(resp_json)?;
        if let Some(error) = response.error {
            anyhow::bail!("MCP tool call failed: {:?}", error);
        }

        response.result.ok_or_else(|| anyhow::anyhow!("Empty MCP response"))
    }
}
