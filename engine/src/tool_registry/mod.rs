use crate::mcp_client::McpClient;
use anyhow::Result;
use serde_json::json;
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct ToolRegistry {
    semble_mcp: Arc<Mutex<McpClient>>,
}

impl ToolRegistry {
    pub fn new(semble_path: &str) -> Self {
        // Find the Python executable in the virtual environment
        // Try root path first, then parent path (if running from within 'engine/')
        let python_path = if std::path::Path::new("rag-python/rag-env/Scripts/python.exe").exists() {
            "rag-python/rag-env/Scripts/python.exe".to_string()
        } else if std::path::Path::new("../rag-python/rag-env/Scripts/python.exe").exists() {
            "../rag-python/rag-env/Scripts/python.exe".to_string()
        } else {
            "python".to_string() // Fallback to system python
        };
        
        let client = McpClient::new(&python_path, vec![
            "-c", "from semble.cli import main; main()", 
            &semble_path 
        ]);
        Self {
            semble_mcp: Arc::new(Mutex::new(client)),
        }
    }

    pub async fn execute(&self, tool: &str, input: &str) -> Result<String> {
        match tool {
            "code_search" | "semble" => {
                let mut client = self.semble_mcp.lock().await;
                let args = json!({ "query": input });
                let result = client.call_tool("search", args).await?;
                
                // Format the output for the LLM
                // Semble's 'search' tool usually returns a list of snippets
                if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                    let mut formatted = String::new();
                    for snippet in content {
                        if let Some(text) = snippet.get("text").and_then(|t| t.as_str()) {
                            formatted.push_str(&format!("---\n{}\n", text));
                        }
                    }
                    Ok(formatted)
                } else {
                    Ok(result.to_string())
                }
            }
            _ => anyhow::bail!("Unsupported tool: {}", tool),
        }
    }
}
