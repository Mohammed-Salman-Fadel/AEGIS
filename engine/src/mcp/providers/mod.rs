//! MCP provider implementations.
//! Each sub-module implements the logic for a specific MCP tool provider
//! (e.g., Obsidian, filesystem, databases, etc.).
//!
//! Providers are registered with the [`McpRegistry`](crate::mcp::registry::McpRegistry)
//! during engine startup and can be discovered and called dynamically.

pub mod obsidian;
