use axum::{
    Json,
    extract::State,
    response::sse::{Event, Sse},
};
use futures::Stream;
use serde::Deserialize;
use std::convert::Infallible;
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_stream::StreamExt;
use tokio_stream::wrappers::ReceiverStream;

use crate::network::state::AppState;

/// Resolve the project filesystem path from a project ID.
/// Mirrors the logic in `projects.rs::dirs_project_dir`.
fn resolve_project_path(project_id: &str) -> Option<PathBuf> {
    let sanitized: String = project_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let base = std::env::var("AEGIS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                std::env::var("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("AEGIS")
            } else {
                std::env::var("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        std::env::var("HOME")
                            .map(|h| PathBuf::from(h).join(".local/share"))
                            .unwrap_or_else(|_| PathBuf::from("."))
                    })
                    .join("AEGIS")
            }
        });
    Some(base.join("projects").join(sanitized))
}

/// Incoming JSON body for POST /chat
#[derive(Deserialize)]
pub struct ChatRequest {
    pub session_id: Option<String>,
    pub message: String,
    #[serde(default)]
    pub attachments: Vec<String>,
    #[serde(default)]
    pub edit_from_turn_index: Option<usize>,
    pub mode: Option<String>,
    #[serde(default)]
    pub response_style: Option<String>,
    #[serde(default)]
    pub code_project_name: Option<String>,
    #[serde(default)]
    pub code_project_path: Option<String>,
    #[serde(default)]
    pub code_project_context: Option<String>,
    #[serde(default)]
    pub code_project_id: Option<String>,
    // RAG Settings
    #[serde(default)]
    pub rag_enabled: Option<bool>,
    #[serde(default)]
    pub rag_top_k: Option<usize>,
    #[serde(default)]
    pub rag_similarity_threshold: Option<f64>,
    #[serde(default)]
    pub reasoning_enabled: bool,
}

/// Handler for POST /chat
///
/// Returns an SSE stream — tokens are pushed into it by the orchestrator
/// as the LLM generates them, and forwarded to the client in real time.
pub async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // channel: orchestrator writes tokens into tx, we read from rx
    // buffer of 32 means up to 32 tokens can queue before orchestrator blocks
    let (tx, rx) = mpsc::channel::<String>(32);

    // spawn the orchestrator as a separate async task
    // it runs concurrently while we stream rx back to the client
    tokio::spawn(async move {
        state.orchestrator.handle(req, tx).await;
    });

    // convert the receiver into a Stream of SSE Events
    let stream = ReceiverStream::new(rx).map(|token| Ok(Event::default().data(token)));

    Sse::new(stream)
}
