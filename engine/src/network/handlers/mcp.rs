//! MCP tool handlers — proxy frontend requests to MCP providers via the McpManager.
//!
//! Called by: `POST /mcp/{provider}/{tool}` routes in `router.rs`
//! Calls into: `Orchestrator.mcp_manager.call_tool()`
//! Owns: Request/Response types for each MCP tool call.
//! Does not own: MCP client lifecycle, provider registration.

use std::collections::HashMap;
use std::path::Path;
use tokio::fs;
use axum::{Json, extract::{Query, State}, http::StatusCode};
use tracing::info;
use serde::{Deserialize, Serialize};

use crate::network::state::AppState;

#[derive(Deserialize)]
pub struct McpToolRequest {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub vault_path: Option<String>,
}

#[derive(Serialize)]
pub struct McpToolResponse {
    pub result: String,
}

/// Validate that a vault path exists on the filesystem.
/// Called by: `GET /mcp/obsidian/validate?path=...`
#[derive(Deserialize)]
pub struct ValidatePathParams {
    pub path: String,
}

#[derive(Serialize)]
pub struct ValidatePathResponse {
    pub valid: bool,
    pub message: String,
}

pub async fn validate_obsidian_path(
    Query(params): Query<ValidatePathParams>,
) -> Json<ValidatePathResponse> {
    let path = std::path::Path::new(&params.path);
    if path.exists() && path.is_dir() {
        Json(ValidatePathResponse {
            valid: true,
            message: "Vault path exists and is accessible.".to_string(),
        })
    } else if path.exists() && !path.is_dir() {
        Json(ValidatePathResponse {
            valid: false,
            message: "Path exists but is not a directory.".to_string(),
        })
    } else {
        Json(ValidatePathResponse {
            valid: false,
            message: "Path does not exist. Check that the vault folder is at this location.".to_string(),
        })
    }
}

/// Build an Obsidian graph from a vault path by reading all .md files
/// and parsing [[wikilinks]] to discover connections.
/// Called by: `POST /mcp/obsidian/graph`
#[derive(Deserialize)]
pub struct GraphRequest {
    pub vault_path: String,
    #[serde(default = "default_max_notes")]
    pub max_notes: usize,
}

fn default_max_notes() -> usize { 2000 }

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,
    pub name: String,
}

#[derive(Serialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
}

#[derive(Serialize)]
pub struct GraphResponse {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub total_files: usize,
    pub elapsed_ms: u64,
}

pub async fn build_obsidian_graph(
    Json(payload): Json<GraphRequest>,
) -> Result<Json<GraphResponse>, (StatusCode, String)> {
    let start = std::time::Instant::now();
    let vault = Path::new(&payload.vault_path);

    if !vault.exists() || !vault.is_dir() {
        return Err((StatusCode::BAD_REQUEST, "Vault path does not exist or is not a directory.".to_string()));
    }

    let mut nodes: HashMap<String, String> = HashMap::new(); // path -> display name
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut total = 0usize;

    let mut walk_stack = vec![vault.to_path_buf()];
    while let Some(dir) = walk_stack.pop() {
        let mut entries = match fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
            if total >= payload.max_notes {
                break;
            }
            let path = entry.path();
            if path.is_dir() {
                // Skip hidden dirs like .obsidian, .git
                if path.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with('.')).unwrap_or(false) {
                    continue;
                }
                walk_stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }

            total += 1;
            let rel_path = path.strip_prefix(vault).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

            nodes.entry(rel_path.clone()).or_insert_with(|| name.clone());

            // Read file content and manually parse [[wikilinks]] (no regex dependency needed)
            if let Ok(content) = fs::read_to_string(&path).await {
                let bytes = content.as_bytes();
                let mut i = 0;
                while i + 1 < bytes.len() {
                    if bytes[i] == b'[' && bytes[i+1] == b'[' {
                        let start = i + 2;
                        let mut end = start;
                        let mut has_pipe = false;
                        while end < bytes.len() {
                            if bytes[end] == b']' && end + 1 < bytes.len() && bytes[end+1] == b']' {
                                break;
                            }
                            if bytes[end] == b'|' && !has_pipe {
                                has_pipe = true;
                            }
                            end += 1;
                        }
                        if end < bytes.len() {
                            let link_bytes = if has_pipe {
                                &bytes[start..start + bytes[start..end].iter().position(|&b| b == b'|').unwrap_or(end - start)]
                            } else {
                                &bytes[start..end]
                            };
                            if let Ok(target) = String::from_utf8(link_bytes.to_vec()) {
                                let target = target.trim();
                                if !target.is_empty() {
                                    let target_file = if target.ends_with(".md") {
                                        target.to_string()
                                    } else {
                                        format!("{}.md", target)
                                    };
                                    let target_file = target_file.replace('\\', "/");
                                    if rel_path != target_file {
                                        edges.push(GraphEdge {
                                            source: rel_path.clone(),
                                            target: target_file,
                                        });
                                    }
                                }
                            }
                            i = end + 2;
                            continue;
                        }
                    }
                    i += 1;
                }
            }
        }
        if total >= payload.max_notes {
            break;
        }
    }

    let node_list: Vec<GraphNode> = nodes.into_iter()
        .map(|(id, name)| GraphNode { id, name })
        .collect();

    let elapsed = start.elapsed().as_millis() as u64;

    Ok(Json(GraphResponse {
        nodes: node_list,
        edges,
        total_files: total,
        elapsed_ms: elapsed,
    }))
}

/// List all .md notes in a vault (Rust-native, no MCP subprocess needed).
/// Called by: `POST /mcp/obsidian/list-notes`
#[derive(Deserialize)]
pub struct ListNotesRequest {
    pub vault_path: String,
    #[serde(default = "default_max_notes")]
    pub max_notes: usize,
}

#[derive(Serialize)]
pub struct ListNotesResponse {
    pub notes: Vec<ListNotesEntry>,
    pub total: usize,
    pub elapsed_ms: u64,
}

#[derive(Serialize)]
pub struct ListNotesEntry {
    pub id: String,
    pub name: String,
}

pub async fn list_vault_notes(
    Json(payload): Json<ListNotesRequest>,
) -> Result<Json<ListNotesResponse>, (StatusCode, String)> {
    let start = std::time::Instant::now();
    let vault = Path::new(&payload.vault_path);

    if !vault.exists() || !vault.is_dir() {
        return Err((StatusCode::BAD_REQUEST, "Vault path does not exist or is not a directory.".to_string()));
    }

    let mut notes = Vec::new();
    let mut total = 0usize;
    let mut walk_stack = vec![vault.to_path_buf()];

    while let Some(dir) = walk_stack.pop() {
        let mut entries = match fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
            if total >= payload.max_notes {
                break;
            }
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()).map(|n| n.starts_with('.')).unwrap_or(false) {
                    continue;
                }
                walk_stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            total += 1;
            let rel_path = path.strip_prefix(vault).unwrap_or(&path).to_string_lossy().replace('\\', "/");
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();
            notes.push(ListNotesEntry { id: rel_path, name });
        }
        if total >= payload.max_notes {
            break;
        }
    }

    let elapsed = start.elapsed().as_millis() as u64;
    Ok(Json(ListNotesResponse { notes, total, elapsed_ms: elapsed }))
}

/// Read a specific .md note from a vault (Rust-native, no MCP subprocess needed).
/// Called by: `POST /mcp/obsidian/read`
#[derive(Deserialize)]
pub struct ReadNoteRequest {
    pub vault_path: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct ReadNoteResponse {
    pub content: String,
    pub path: String,
}

pub async fn read_vault_note(
    Json(payload): Json<ReadNoteRequest>,
) -> Result<Json<ReadNoteResponse>, (StatusCode, String)> {
    let vault = Path::new(&payload.vault_path);
    if !vault.exists() || !vault.is_dir() {
        return Err((StatusCode::BAD_REQUEST, "Vault path does not exist or is not a directory.".to_string()));
    }
    let note_path = vault.join(&payload.path);
    if !note_path.exists() || !note_path.is_file() {
        return Err((StatusCode::NOT_FOUND, format!("Note not found: {}", payload.path)));
    }
    let normalized = note_path.canonicalize().map_err(|_| (StatusCode::BAD_REQUEST, "Invalid note path.".to_string()))?;
    if !normalized.starts_with(vault.canonicalize().unwrap_or_else(|_| vault.to_path_buf())) {
        return Err((StatusCode::FORBIDDEN, "Note path escapes the vault directory.".to_string()));
    }
    let content = tokio::fs::read_to_string(&note_path).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read note: {}", e)))?;
    let rel_path = payload.path.replace('\\', "/");
    Ok(Json(ReadNoteResponse { content, path: rel_path }))
}

/// Serve any file from the vault (images, attachments, etc).
/// Searches common subdirectories and the note's directory if supplied.
/// Called by: `GET /mcp/obsidian/file?vault_path=...&path=...&note_dir=...`
#[derive(Deserialize)]
pub struct VaultFileParams {
    pub vault_path: String,
    pub path: String,
    #[serde(default)]
    pub note_dir: Option<String>,
}

use axum::body::Body;
use axum::response::Response;

pub async fn serve_vault_file(
    Query(params): Query<VaultFileParams>,
) -> Result<Response<Body>, (StatusCode, String)> {
    let vault = std::path::Path::new(&params.vault_path);
    if !vault.exists() || !vault.is_dir() {
        return Err((StatusCode::BAD_REQUEST, "Vault path does not exist or is not a directory.".to_string()));
    }

    // Try the exact path, note-relative path, then common attachment folders
    let mut candidates = vec![
        vault.join(&params.path),
        vault.join("images").join(&params.path),
        vault.join("attachments").join(&params.path),
        vault.join("assets").join(&params.path),
        vault.join(".obsidian").join("attachments").join(&params.path),
    ];
    if let Some(ref nd) = params.note_dir {
        candidates.push(vault.join(nd).join(&params.path));
        candidates.push(vault.join(nd).join("images").join(&params.path));
    }

    let file_path = candidates.iter().find(|p| p.exists() && p.is_file());

    match file_path {
        None => return Err((StatusCode::NOT_FOUND, format!("File not found: {}", params.path))),
        Some(path) => {
            let normalized = path.canonicalize().map_err(|_| (StatusCode::BAD_REQUEST, "Invalid path.".to_string()))?;
            let vault_canon = vault.canonicalize().unwrap_or_else(|_| vault.to_path_buf());
            if !normalized.starts_with(&vault_canon) {
                return Err((StatusCode::FORBIDDEN, "Path escapes the vault directory.".to_string()));
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
            let mime = match ext.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "svg" => "image/svg+xml",
                "webp" => "image/webp",
                "ico" => "image/x-icon",
                "pdf" => "application/pdf",
                _ => "application/octet-stream",
            };
            let data = tokio::fs::read(path).await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to read file: {}", e)))?;
            Ok(Response::builder()
                .header("Content-Type", mime)
                .header("Cache-Control", "public, max-age=3600")
                .body(Body::from(data))
                .unwrap())
        }
    }
}

/// Derive the vault name that obsidian-mcp computes from a vault path.
/// obsidian-mcp's sanitizeVaultName takes the basename, lowercases it,
/// replaces non-alphanumeric chars with hyphens, collapses runs, and trims.
fn derive_vault_name(vault_path: &str) -> Option<String> {
    let basename = std::path::Path::new(vault_path)
        .file_name()
        .and_then(|n| n.to_str())?;
    let lowered = basename.to_lowercase();
    let with_hyphens: String = lowered
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' { c } else { '-' })
        .collect();
    let trimmed = with_hyphens.trim_matches('-');
    if trimmed.is_empty() {
        return None;
    }
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_hyphen = false;
    for c in trimmed.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    Some(result)
}

/// Split a combined path (e.g. "folder/note.md") into (filename, optional folder).
fn split_path<'a>(path: &'a str) -> (&'a str, Option<&'a str>) {
    if let Some(pos) = path.rfind('/') {
        let folder = &path[..pos];
        let filename = &path[pos + 1..];
        (filename, if folder.is_empty() { None } else { Some(folder) })
    } else if let Some(pos) = path.rfind('\\') {
        let folder = &path[..pos];
        let filename = &path[pos + 1..];
        (filename, if folder.is_empty() { None } else { Some(folder) })
    } else {
        (path, None)
    }
}

/// Handle a generic MCP tool call: `POST /mcp/{provider}/{tool}`
pub async fn call_mcp_tool(
    State(state): State<AppState>,
    axum::extract::Path((provider, tool)): axum::extract::Path<(String, String)>,
    Json(payload): Json<McpToolRequest>,
) -> Result<Json<McpToolResponse>, (StatusCode, String)> {
    // Dynamic provider registration: if this is the first call for "obsidian" with a vault_path,
    // register the provider on-the-fly so the frontend setting actually works.
    // The obsidian-mcp package takes vault paths as positional arguments (not --vault-path flags).
    if provider == "obsidian" && !state.orchestrator.mcp_manager.has_provider("obsidian").await {
        if let Some(vault_path) = &payload.vault_path {
            if !vault_path.trim().is_empty() {
                tracing::info!("Dynamically registering Obsidian MCP provider for vault: {}", vault_path);
                let (command, base_args) = crate::mcp::providers::obsidian::build_obsidian_command();
                // Append vault path as a positional argument (the obsidian-mcp package expects it this way)
                let mut args = base_args;
                args.push(vault_path.trim().to_string());
                state.orchestrator.mcp_manager.registry()
                    .register_provider(
                        "obsidian",
                        crate::mcp::types::McpProviderKind::Subprocess { command, args },
                        true,
                    )
                    .await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to start Obsidian MCP: {}", e)))?;
            }
        }
    }

    // Check the provider is registered before calling
    if !state.orchestrator.mcp_manager.has_provider(&provider).await {
        return Err((StatusCode::BAD_REQUEST, format!("MCP provider '{}' is not available. Configure it in Settings → Tools → Obsidian.", provider)));
    }

    // Build arguments from the request payload
    // The obsidian-mcp package expects:
    //   - search-vault: { vault, query }
    //   - read-note:    { vault, filename, folder? }
    //   - create-note:  { vault, filename, folder?, content }
    //   - list-available-vaults: {} (no args)
    let mut args = serde_json::Map::new();

    // Compute vault name from vault_path (only for tools that need it)
    if tool != "list-available-vaults" {
        if let Some(ref vp) = payload.vault_path {
            if let Some(vn) = derive_vault_name(vp) {
                args.insert("vault".to_string(), serde_json::Value::String(vn));
            }
        }
    }

    if let Some(query) = &payload.query {
        args.insert("query".to_string(), serde_json::Value::String(query.clone()));
    }
    if let Some(path) = &payload.path {
        // read-note and create-note expect filename + folder (separate), not a combined path
        if tool == "read-note" || tool == "create-note" || tool == "edit-note" || tool == "delete-note" || tool == "move-note" {
            let (filename, folder) = split_path(path);
            args.insert("filename".to_string(), serde_json::Value::String(filename.to_string()));
            if let Some(f) = folder {
                args.insert("folder".to_string(), serde_json::Value::String(f.to_string()));
            }
        } else {
            args.insert("path".to_string(), serde_json::Value::String(path.clone()));
        }
    }
    if let Some(content) = &payload.content {
        args.insert("content".to_string(), serde_json::Value::String(content.clone()));
    }

    let result = state
        .orchestrator
        .mcp_manager
        .call_tool(&provider, &tool, serde_json::Value::Object(args))
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("MCP tool call failed: {}", e),
            )
        })?;

    Ok(Json(McpToolResponse {
        result: result.content,
    }))
}
