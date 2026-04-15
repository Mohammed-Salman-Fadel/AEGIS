use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Serialize;

use crate::memory_store::{Session, SessionSummary};
use crate::network::state::AppState;

#[derive(Serialize)]
pub struct SessionsResponse {
    sessions: Vec<SessionSummary>,
}

pub async fn list_sessions(State(state): State<AppState>) -> Json<SessionsResponse> {
    Json(SessionsResponse {
        sessions: state.orchestrator.list_sessions(),
    })
}

pub async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Session>, StatusCode> {
    state
        .orchestrator
        .get_session(&session_id)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}
