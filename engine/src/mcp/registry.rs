//! MCP provider registry — dynamically manages registered MCP providers,
//! their lifecycle, tool discovery, and routing.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use super::client::McpClient;
use super::types::{McpProvider, McpProviderKind, McpToolResult, McpTool};

/// Manages a collection of MCP providers.
pub struct McpRegistry {
    providers: RwLock<Vec<McpProvider>>,
    clients: RwLock<HashMap<String, Arc<RwLock<McpClient>>>>,
}

impl McpRegistry {
    pub fn new() -> Self {
        Self {
            providers: RwLock::new(Vec::new()),
            clients: RwLock::new(HashMap::new()),
        }
    }

    /// Register a new MCP provider. This will spawn its subprocess
    /// and discover its available tools.
    pub async fn register_provider(
        &self,
        name: &str,
        kind: McpProviderKind,
        enabled: bool,
    ) -> anyhow::Result<()> {
        match &kind {
            McpProviderKind::Subprocess { command, args } => {
                let mut client = McpClient::new(command, args.clone());
                client.ensure_started().await?;

                // Discover tools from the server
                let tools = client.list_tools().await?;

                let provider = McpProvider {
                    name: name.to_string(),
                    kind,
                    tools,
                    enabled,
                };

                self.providers.write().await.push(provider);
                self.clients
                    .write()
                    .await
                    .insert(name.to_string(), Arc::new(RwLock::new(client)));

                info!(
                    "Registered MCP provider '{}' with {} tools",
                    name,
                    self.providers.read().await.last().map(|p| p.tools.len()).unwrap_or(0)
                );
            }
            McpProviderKind::Http { .. } => {
                anyhow::bail!("HTTP-based MCP providers are not yet supported");
            }
        }

        Ok(())
    }

    /// Get all registered providers and their tools.
    pub async fn list_providers(&self) -> Vec<McpProvider> {
        self.providers.read().await.clone()
    }

    /// Find a registered provider by name.
    pub async fn find_provider(&self, name: &str) -> Option<McpProvider> {
        self.providers
            .read()
            .await
            .iter()
            .find(|p| p.name == name)
            .cloned()
    }

    /// Check if a provider with the given name is registered.
    pub async fn has_provider(&self, name: &str) -> bool {
        self.providers
            .read()
            .await
            .iter()
            .any(|p| p.name == name)
    }

    /// Call a tool on a specific provider.
    pub async fn call_tool(
        &self,
        provider_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolResult> {
        let clients = self.clients.read().await;
        let client = clients
            .get(provider_name)
            .ok_or_else(|| anyhow::anyhow!("MCP provider '{}' not found", provider_name))?;

        let raw = {
            let mut client_lock = client.write().await;
            client_lock.call_tool(tool_name, arguments.clone()).await?
        };

        // Extract text content from the response
        let content = extract_text_content(&raw);

        Ok(McpToolResult {
            tool: tool_name.to_string(),
            provider: provider_name.to_string(),
            content,
            raw,
        })
    }

    /// Get the tools available for a specific provider.
    pub async fn get_provider_tools(&self, provider_name: &str) -> anyhow::Result<Vec<McpTool>> {
        let providers = self.providers.read().await;
        let provider = providers
            .iter()
            .find(|p| p.name == provider_name)
            .ok_or_else(|| anyhow::anyhow!("MCP provider '{}' not found", provider_name))?;
        Ok(provider.tools.clone())
    }

    /// Shut down all MCP provider subprocesses.
    pub async fn shutdown_all(&self) {
        let clients = self.clients.read().await;
        for (name, client) in clients.iter() {
            let mut client_lock = client.write().await;
            if let Err(e) = client_lock.shutdown().await {
                warn!("Failed to shut down MCP provider '{}': {}", name, e);
            } else {
                info!("Shut down MCP provider '{}'", name);
            }
        }
    }

    /// Enable or disable a registered provider.
    pub async fn set_provider_enabled(&self, name: &str, enabled: bool) -> anyhow::Result<()> {
        let mut providers = self.providers.write().await;
        let provider = providers
            .iter_mut()
            .find(|p| p.name == name)
            .ok_or_else(|| anyhow::anyhow!("MCP provider '{}' not found", name))?;
        provider.enabled = enabled;
        Ok(())
    }
}

/// Extract human-readable text content from an MCP tool response.
fn extract_text_content(result: &serde_json::Value) -> String {
    if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
        let mut formatted = String::new();
        for item in content {
            if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                formatted.push_str(&format!("---\n{}\n", text));
            }
        }
        if !formatted.is_empty() {
            return formatted;
        }
    }
    result.to_string()
}
