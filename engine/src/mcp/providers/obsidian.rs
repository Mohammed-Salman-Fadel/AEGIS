//! Obsidian MCP provider.
//!
//! Connects to an Obsidian vault via the `obsidian-mcp` server, exposing
//! tools for reading, searching, creating, and updating notes.
//!
//! ## Configuration
//! - `AEGIS_MCP_OBSIDIAN_COMMAND` — The command to start the Obsidian MCP server
//!   (default: `"npx"`)
//! - `AEGIS_MCP_OBSIDIAN_ARGS` — Arguments for the command (default:
//!   `["-y", "obsidian-mcp"]`)
//! - `AEGIS_MCP_OBSIDIAN_VAULT_PATH` — Path to the Obsidian vault directory
//!   (optional, passed as a positional argument, NOT as `--vault-path`).
//!   The vault path can also be set via the AEGIS Settings UI and is sent
//!   dynamically on the first API call.
//!
//! ## Exposed Tools (kebab-case names, as defined by the `obsidian-mcp` package)
//! | Tool | Description |
//! |------|-------------|
//! | `read-note` | Read the contents of a specific note by path |
//! | `search-vault` | Search notes by keyword or phrase |
//! | `create-note` | Create a new note with given path and content |
//! | `edit-note` | Edit an existing note |
//! | `delete-note` | Delete a note |
//! | `move-note` | Move a note to a different location |
//! | `create-directory` | Create a new directory |
//! | `add-tags` | Add tags to a note |
//! | `remove-tags` | Remove tags from a note |
//! | `rename-tag` | Rename a tag across all notes |
//! | `manage-tags` | List and organize tags |
//! | `list-available-vaults` | List all available vaults |
//!
//! ## Usage from AEGIS
//! When chat mode is set to `general` and an Obsidian query is detected,
//! the orchestrator calls `McpRegistry::call_tool("obsidian", ...)`.

use crate::mcp::registry::McpRegistry;

/// The registered name for the Obsidian provider in the MCP registry.
pub const PROVIDER_NAME: &str = "obsidian";

/// Default command to start the Obsidian MCP server.
const DEFAULT_COMMAND: &str = "npx";

/// Default arguments for the Obsidian MCP server.
/// The vault path is appended as a positional argument at runtime.
const DEFAULT_ARGS: [&str; 2] = ["-y", "obsidian-mcp"];

/// Environment variable for the Obsidian MCP command.
const ENV_COMMAND: &str = "AEGIS_MCP_OBSIDIAN_COMMAND";

/// Environment variable for the Obsidian MCP command arguments.
const ENV_ARGS: &str = "AEGIS_MCP_OBSIDIAN_ARGS";

/// Environment variable for the Obsidian vault path.
const ENV_VAULT_PATH: &str = "AEGIS_MCP_OBSIDIAN_VAULT_PATH";

/// Build the command and arguments to start the Obsidian MCP server.
/// On Windows, wraps the command in `cmd /c` so that `.cmd` batch files
/// (like those installed by npm global installs) resolve correctly.
pub fn build_obsidian_command() -> (String, Vec<String>) {
    let raw_command = std::env::var(ENV_COMMAND)
        .unwrap_or_else(|_| DEFAULT_COMMAND.to_string());

    let mut raw_args: Vec<String> = if let Ok(args_str) = std::env::var(ENV_ARGS) {
        args_str.split_whitespace().map(|s| s.to_string()).collect()
    } else {
        DEFAULT_ARGS.iter().map(|s| s.to_string()).collect()
    };

    // Only append vault path from env var (the API-driven path is appended by the handler)
    if let Ok(vault_path) = std::env::var(ENV_VAULT_PATH) {
        if !vault_path.trim().is_empty() {
            raw_args.push(vault_path.trim().to_string());
        }
    }

    // On Windows, wrap via `cmd /c` so that `npx.cmd` / `obsidian-mcp.cmd` resolve properly.
    // This avoids Rust Command not resolving PATHEXT extensions.
    if cfg!(windows) {
        let mut cmd_args = vec!["/c".to_string(), raw_command];
        cmd_args.extend(raw_args);
        ("cmd".to_string(), cmd_args)
    } else {
        (raw_command, raw_args)
    }
}

/// Register the Obsidian MCP provider with the registry.
pub async fn register(registry: &McpRegistry) -> anyhow::Result<()> {
    let (command, args) = build_obsidian_command();

    registry
        .register_provider(
            PROVIDER_NAME,
            crate::mcp::types::McpProviderKind::Subprocess { command, args },
            true,
        )
        .await
}

/// Helper: search Obsidian notes by query string.
pub async fn search_notes(
    registry: &McpRegistry,
    query: &str,
) -> anyhow::Result<String> {
    let result = registry
        .call_tool(
            PROVIDER_NAME,
            "search-vault",
            serde_json::json!({ "query": query }),
        )
        .await?;
    Ok(result.content)
}

/// Helper: read the contents of a specific note.
pub async fn read_note(
    registry: &McpRegistry,
    note_path: &str,
) -> anyhow::Result<String> {
    let result = registry
        .call_tool(
            PROVIDER_NAME,
            "read-note",
            serde_json::json!({ "path": note_path }),
        )
        .await?;
    Ok(result.content)
}

/// Helper: create a new note with the given path and content.
pub async fn create_note(
    registry: &McpRegistry,
    note_path: &str,
    content: &str,
) -> anyhow::Result<String> {
    let result = registry
        .call_tool(
            PROVIDER_NAME,
            "create-note",
            serde_json::json!({
                "path": note_path,
                "content": content
            }),
        )
        .await?;
    Ok(result.content)
}

/// Helper: edit an existing note by overwriting content.
pub async fn edit_note(
    registry: &McpRegistry,
    note_path: &str,
    content: &str,
) -> anyhow::Result<String> {
    let result = registry
        .call_tool(
            PROVIDER_NAME,
            "edit-note",
            serde_json::json!({
                "path": note_path,
                "content": content
            }),
        )
        .await?;
    Ok(result.content)
}
