use crate::mcp_client::McpClient;
use anyhow::Result;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ToolRegistry {
    semble_mcp: Arc<Mutex<McpClient>>,
    zotero_mcp: Arc<Mutex<McpClient>>,
}

impl ToolRegistry {
    pub fn new(python_path: &str, semble_path: &str) -> Self {
        // Initialize Semble MCP
        let client = McpClient::new(
            python_path,
            vec!["-c", "from semble.cli import main; main()", semble_path],
        );

        // Initialize Zotero MCP
        // We run it as a module via the unified python environment
        let zotero_client = McpClient::new(python_path, vec!["-m", "zotero_mcp.cli", "serve"]);

        Self {
            semble_mcp: Arc::new(Mutex::new(client)),
            zotero_mcp: Arc::new(Mutex::new(zotero_client)),
        }
    }

    pub async fn execute(&self, tool: &str, input: &str) -> Result<String> {
        match tool {
            "code_search" | "semble" => {
                let mut client = self.semble_mcp.lock().await;
                let args = json!({ "query": input });
                let result = client.call_tool("search", args).await?;

                // Format the output for the LLM
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
            "zotero" | "citation" | "research" => {
                let mut client = self.zotero_mcp.lock().await;

                // Determine which tool to call based on keywords in the input
                let tool_name = if input.to_lowercase().contains("recent") {
                    "zotero_get_recent"
                } else {
                    "zotero_search_items"
                };

                tracing::info!("Calling Zotero MCP tool: {}", tool_name);
                let args = if tool_name.contains("get_recent") {
                    json!({ "limit": 10 })
                } else {
                    json!({ "query": input, "limit": 10 })
                };

                // Try calling the tool. Some versions use 'search_items', others 'zotero_search_items'
                let result = match client.call_tool(tool_name, args.clone()).await {
                    Ok(res) => res,
                    Err(_) => {
                        // Fallback to a simpler name if the prefixed one fails
                        let fallback = if tool_name == "zotero_get_recent" {
                            "get_recent"
                        } else {
                            "search_items"
                        };
                        client.call_tool(fallback, args).await?
                    }
                };

                if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
                    if content.is_empty() {
                        return Ok("No items found in Zotero library for this query.".to_string());
                    }
                    let mut formatted = String::new();
                    for item in content {
                        if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                            formatted.push_str(&format!("---\n{}\n", text));
                        }
                    }
                    Ok(formatted)
                } else {
                    let res_str = result.to_string();
                    if res_str == "{}" || res_str == "{\"content\":[]}" {
                        Ok("No results found in Zotero.".to_string())
                    } else {
                        Ok(res_str)
                    }
                }
            }
            _ => anyhow::bail!("Unsupported tool: {}", tool),
        }
    }
}
