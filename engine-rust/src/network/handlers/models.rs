//! Role: HTTP handlers for reading and changing the active engine model.
//! Called by: `network/router.rs` when the CLI or future web UI hits `/models/*`.
//! Calls into: `network/state.rs` and the orchestrator's model registry methods.
//! Owns: request/response shapes for model status and model switching.
//! Does not own: Ollama model discovery, inference execution, or CLI rendering.
//! Next TODOs: validate requested models against the provider catalog before accepting switches.

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::network::state::AppState;

#[derive(Serialize)]
pub struct CurrentModelResponse {
    model: String,
}

#[derive(Deserialize)]
pub struct SelectModelRequest {
    name: String,
}

#[derive(Serialize)]
pub struct SelectModelResponse {
    current: String,
    persisted: bool,
    message: String,
}

pub async fn current_model(State(state): State<AppState>) -> Json<CurrentModelResponse> {
    Json(CurrentModelResponse {
        model: state.orchestrator.current_model_name(),
    })
}

pub async fn select_model(
    State(state): State<AppState>,
    Json(payload): Json<SelectModelRequest>,
) -> Result<Json<SelectModelResponse>, StatusCode> {
    let next_model = payload.name.trim();
    if next_model.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    let previous_model = state.orchestrator.set_active_model(next_model);

    Ok(Json(SelectModelResponse {
        current: next_model.to_string(),
        persisted: true,
        message: format!("Switched from {previous_model} to {next_model}."),
    }))
}
