//! Project file ingestion — receives project files from the frontend,
//! saves them to a temp directory, and indexes them via the RAG service.
//! Called by: `POST /projects/ingest`
//! Calls into: `Orchestrator.rag_client`

use std::path::Path;
use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use crate::network::state::AppState;

#[derive(Deserialize)]
pub struct ProjectIngestRequest {
    pub project_id: String,
    pub files: Vec<ProjectFile>,
}

#[derive(Deserialize)]
pub struct ProjectFile {
    pub path: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ProjectIngestResponse {
    pub status: String,
    pub project_id: String,
    pub files_indexed: usize,
}

pub async fn ingest_project_files(
    State(state): State<AppState>,
    Json(payload): Json<ProjectIngestRequest>,
) -> Result<Json<ProjectIngestResponse>, (StatusCode, String)> {
    let project_dir = dirs_project_dir(&payload.project_id);
    tokio::fs::create_dir_all(&project_dir).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to create project directory: {}", e)))?;

    // Clean previous files
    if let Ok(mut entries) = tokio::fs::read_dir(&project_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let _ = tokio::fs::remove_file(entry.path()).await;
        }
    }

    // Canonicalize the project dir so we can validate paths against it.
    let canonical_project_dir = project_dir.canonicalize().unwrap_or(project_dir.clone());

    let mut indexed = 0usize;
    for file in &payload.files {
        // Path traversal guard: reject paths with `..` segments.
        if file.path.contains("..") {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Path traversal rejected: `{}`", file.path),
            ));
        }
        let file_path = canonical_project_dir.join(&file.path);
        // Verify the resolved path stays within the project directory.
        match file_path.canonicalize() {
            Ok(resolved) => {
                if !resolved.starts_with(&canonical_project_dir) {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        format!("Access denied: `{}` resolves outside project directory", file.path),
                    ));
                }
            }
            // File doesn't exist yet (will be created) — validate parent.
            Err(_) => {
                if let Some(parent) = file_path.parent() {
                    match parent.canonicalize() {
                        Ok(canon_parent) => {
                            if !canon_parent.starts_with(&canonical_project_dir) {
                                return Err((
                                    StatusCode::BAD_REQUEST,
                                    format!("Access denied: `{}` resolves outside project directory", file.path),
                                ));
                            }
                        }
                        Err(_) => {
                            return Err((
                                StatusCode::BAD_REQUEST,
                                format!("Parent directory does not exist for `{}`", file.path),
                            ));
                        }
                    }
                }
            }
        }
        if let Some(parent) = file_path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        tokio::fs::write(&file_path, &file.content).await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to write project file {}: {}", file.path, e)))?;

        // Index through RAG using a project-scoped session ID
        let session_id = format!("__project__{}", &payload.project_id);
        if let Err(e) = state.orchestrator.rag_client.ingest(
            file_path.to_string_lossy().to_string(),
            &session_id,
        ).await {
            tracing::warn!("Failed to index project file {}: {}", file.path, e);
        } else {
            indexed += 1;
        }
    }

    Ok(Json(ProjectIngestResponse {
        status: "indexed".to_string(),
        project_id: payload.project_id.clone(),
        files_indexed: indexed,
    }))
}

fn dirs_project_dir(project_id: &str) -> std::path::PathBuf {
    let sanitized: String = project_id.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let base = std::env::var("AEGIS_DATA_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_data_dir().join("AEGIS")
        });
    base.join("projects").join(sanitized)
}

fn dirs_data_dir() -> std::path::PathBuf {
    if cfg!(windows) {
        std::env::var("APPDATA")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
    } else {
        std::env::var("XDG_DATA_HOME")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| std::path::PathBuf::from(h).join(".local/share"))
                    .unwrap_or_else(|_| std::path::PathBuf::from("."))
            })
    }
}
