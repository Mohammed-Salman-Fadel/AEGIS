//! MCP (Model Context Protocol) infrastructure.
//!
//! Provides a scalable framework for registering and interacting with MCP tool
//! providers. The [`McpManager`] is the top-level entry point, holding the
//! [`McpRegistry`] and exposing provider-agnostic tool execution.
//!
//! ## Architecture
//! ```text
//! Orchestrator
//!   └── McpManager
//!         ├── McpRegistry          ← dynamic provider registry
//!         │     ├── McpClient      ← JSON-RPC subprocess client
//!         │     └── McpClient
//!         └── providers/
//!               ├── obsidian.rs
//!               └── ... (future providers)
//! ```
//!
//! ## Adding a new MCP provider
//! 1. Create a new file in `providers/` (e.g., `providers/my_tool.rs`)
//! 2. Implement a `register(registry: &McpRegistry)` function
//! 3. Add helper functions for each tool the provider exposes
//! 4. Call the register function from `McpManager::register_defaults()`
//! 5. Add the corresponding env var config to [`crate::config::AppConfig`]

pub mod client;
pub mod providers;
pub mod registry;
pub mod types;

use registry::McpRegistry;
use types::{McpProvider, McpToolResult};

/// Top-level manager for all MCP provider interactions.
/// Held by the [`Orchestrator`](crate::orchestrator::Orchestrator) as a single field.
pub struct McpManager {
    registry: McpRegistry,
}

impl McpManager {
    /// Create a new McpManager and register the default set of providers.
    pub async fn new() -> Self {
        let manager = Self {
            registry: McpRegistry::new(),
        };
        manager.register_defaults().await;
        manager
    }

    /// Register the default set of MCP providers based on environment config.
    async fn register_defaults(&self) {
        // Register Obsidian if configured
        if std::env::var("AEGIS_MCP_OBSIDIAN_COMMAND").is_ok()
            || std::env::var("AEGIS_MCP_OBSIDIAN_VAULT_PATH").is_ok()
        {
            match providers::obsidian::register(&self.registry).await {
                Ok(()) => tracing::info!("Obsidian MCP provider registered"),
                Err(e) => tracing::warn!("Failed to register Obsidian MCP provider: {}", e),
            }
        } else {
            tracing::debug!(
                "Obsidian MCP provider not configured (set AEGIS_MCP_OBSIDIAN_VAULT_PATH)"
            );
        }
    }

    /// Call a tool on a specific provider.
    pub async fn call_tool(
        &self,
        provider_name: &str,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolResult> {
        self.registry
            .call_tool(provider_name, tool_name, arguments)
            .await
    }

    /// List all registered providers and their tools.
    pub async fn list_providers(&self) -> Vec<McpProvider> {
        self.registry.list_providers().await
    }

    /// Get tools for a specific provider.
    pub async fn get_provider_tools(
        &self,
        provider_name: &str,
    ) -> anyhow::Result<Vec<types::McpTool>> {
        self.registry.get_provider_tools(provider_name).await
    }

    /// Check if a specific provider is registered.
    pub async fn has_provider(&self, name: &str) -> bool {
        self.registry.has_provider(name).await
    }

    /// Shut down all MCP provider subprocesses.
    pub async fn shutdown(&self) {
        self.registry.shutdown_all().await;
    }

    /// Get a reference to the underlying registry (for direct provider registration).
    pub fn registry(&self) -> &McpRegistry {
        &self.registry
    }
}
