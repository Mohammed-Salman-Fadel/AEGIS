//! Role: HTTP handler for reporting model context-window usage.
//! Called by: `network/router.rs` when the web UI needs the composer token meter.
//! Calls into: `orchestrator/mod.rs` for active model metadata and stored session usage.
//! Owns: request/response shapes for context usage only.
//! Does not own: tokenization, model switching, or session persistence.
//! Next TODOs: expose draft-prompt tokenization if the active backend adds a cheap tokenizer API.

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};

use crate::network::state::AppState;

#[derive(Deserialize)]
pub struct ContextUsageQuery {
    session_id: Option<String>,
}

#[derive(Serialize)]
pub struct ContextUsageResponse {
    provider: String,
    model: String,
    used_tokens: usize,
    context_window: usize,
    usage_source: String,
}

pub async fn usage(
    State(state): State<AppState>,
    Query(query): Query<ContextUsageQuery>,
) -> Result<Json<ContextUsageResponse>, (StatusCode, String)> {
    let session_id = query
        .session_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let usage = state
        .orchestrator
        .context_usage(session_id)
        .await
        .map_err(|error| (StatusCode::BAD_GATEWAY, error.to_string()))?;

    Ok(Json(ContextUsageResponse {
        provider: usage.provider,
        model: usage.model,
        used_tokens: usage.used_tokens,
        context_window: usage.context_window,
        usage_source: usage.usage_source,
    }))
}
