//! Role: HTTP handlers for reading and changing the active engine model.
//! Called by: `network/router.rs` when the CLI or future web UI hits `/models/*`.
//! Calls into: `network/state.rs` and the orchestrator's model registry methods.
//! Owns: request/response shapes for model status and model switching.
//! Does not own: Ollama model discovery, inference execution, or CLI rendering.
//! Next TODOs: validate requested models against the provider catalog before accepting switches.

use axum::{Json, extract::State, http::StatusCode};
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
    previous: String,
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
) -> Result<Json<SelectModelResponse>, (StatusCode, String)> {
    let next_model = payload.name.trim();
    if next_model.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "The requested model name cannot be empty.".to_string(),
        ));
    }

    let outcome = state
        .orchestrator
        .switch_active_model(next_model)
        .await
        .map_err(model_switch_error)?;

    let message = if !outcome.changed {
        format!("`{}` is already the active model.", outcome.current_model)
    } else if let Some(warning) = &outcome.unload_warning {
        format!(
            "Switched from {} to {}. {}",
            outcome.previous_model, outcome.current_model, warning
        )
    } else {
        format!(
            "Switched from {} to {}.",
            outcome.previous_model, outcome.current_model
        )
    };

    Ok(Json(SelectModelResponse {
        previous: outcome.previous_model,
        current: outcome.current_model,
        persisted: true,
        message,
    }))
}

fn model_switch_error(error: anyhow::Error) -> (StatusCode, String) {
    let message = error.to_string();
    let status = if message.contains("warm") {
        StatusCode::BAD_GATEWAY
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };

    (status, message)
}
