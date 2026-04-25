//! Role: HTTP handlers for persisted session lifecycle and retrieval.
//! Called by: `network/router.rs` when the CLI or web UI hits `/sessions*`.
//! Calls into: `network/state.rs` and orchestrator session methods.
//! Owns: JSON request/response shapes for session creation, listing, loading, and deletion.
//! Does not own: storage implementation details, inference execution, or CLI formatting.
//! Next TODOs: add pagination and richer session metadata endpoints when the UI needs them.

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::Serialize;

use crate::memory_store::{Session, SessionSummary};
use crate::network::state::AppState;

#[derive(Serialize)]
pub struct SessionsResponse {
    sessions: Vec<SessionSummary>,
}

#[derive(Serialize)]
pub struct DeleteSessionResponse {
    session_id: String,
    persisted: bool,
    message: String,
}

pub async fn create_session(
    State(state): State<AppState>,
) -> Result<Json<Session>, (StatusCode, String)> {
    state
        .orchestrator
        .create_session()
        .await
        .map(Json)
        .map_err(session_error)
}

pub async fn list_sessions(
    State(state): State<AppState>,
) -> Result<Json<SessionsResponse>, (StatusCode, String)> {
    let sessions = state
        .orchestrator
        .list_sessions()
        .await
        .map_err(session_error)?;

    Ok(Json(SessionsResponse { sessions }))
}

pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Session>, (StatusCode, String)> {
    let session = state
        .orchestrator
        .get_session(&session_id)
        .await
        .map_err(session_error)?;

    session
        .map(Json)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Session `{session_id}` was not found.")))
}

pub async fn delete_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<DeleteSessionResponse>, (StatusCode, String)> {
    let deleted = state
        .orchestrator
        .delete_session(&session_id)
        .await
        .map_err(session_error)?;

    if !deleted {
        return Err((
            StatusCode::NOT_FOUND,
            format!("Session `{session_id}` was not found."),
        ));
    }

    Ok(Json(DeleteSessionResponse {
        session_id: session_id.clone(),
        persisted: true,
        message: format!("Session `{session_id}` was deleted."),
    }))
}

fn session_error(error: anyhow::Error) -> (StatusCode, String) {
    let message = error.to_string();
    let status = if message.contains("Session persistence is unavailable") {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };

    (status, message)
}
