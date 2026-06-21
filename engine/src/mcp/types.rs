//! Shared MCP types used across the MCP infrastructure.
//! Defines JSON-RPC message structures, tool schemas, and capability descriptors.

use serde::{Deserialize, Serialize};

/// Standard JSON-RPC 2.0 request.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
    pub id: u64,
}

/// Standard JSON-RPC 2.0 response.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<JsonRpcError>,
    pub id: Option<u64>,
}

/// JSON-RPC error object.
#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(default)]
    pub data: Option<serde_json::Value>,
}

/// Describes an MCP tool that a provider exposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub input_schema: serde_json::Value,
}

/// Describes an MCP provider (a server that exposes tools).
#[derive(Debug, Clone)]
pub struct McpProvider {
    pub name: String,
    pub kind: McpProviderKind,
    pub tools: Vec<McpTool>,
    pub enabled: bool,
}

/// How the MCP server is launched.
#[derive(Debug, Clone)]
pub enum McpProviderKind {
    /// Subprocess via command + args (e.g., stdio-based MCP servers).
    Subprocess { command: String, args: Vec<String> },
    /// TCP/HTTP based server at a URL.
    Http { base_url: String },
}

/// Result from calling an MCP tool.
#[derive(Debug, Clone)]
pub struct McpToolResult {
    pub tool: String,
    pub provider: String,
    pub content: String,
    pub raw: serde_json::Value,
}

/// Configuration for an individual MCP provider.
#[derive(Debug, Clone, Deserialize)]
pub struct McpProviderConfig {
    pub name: String,
    pub enabled: bool,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// Top-level MCP configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct McpConfig {
    #[serde(default)]
    pub providers: Vec<McpProviderConfig>,
}
