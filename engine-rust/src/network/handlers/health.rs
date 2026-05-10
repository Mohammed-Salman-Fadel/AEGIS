use axum::{extract::State, Json};
use serde::Serialize;

use crate::network::state::AppState;

#[derive(Serialize)]
pub struct HealthResponse {
    status: &'static str,
    engine: &'static str,
    provider: String,
    model: String,
    sessions: usize,
    rag: ServiceHealth,
}

#[derive(Serialize)]
pub struct ServiceHealth {
    status: &'static str,
    mode: &'static str,
}

pub async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let rag_reachable = state.orchestrator.rag_client.health().await;
    Json(HealthResponse {
        status: if rag_reachable { "ok" } else { "degraded" },
        engine: "aegis-engine",
        provider: state.provider.clone(),
        model: state.orchestrator.active_model_name(),
        sessions: state.orchestrator.list_sessions().len(),
        rag: ServiceHealth {
            status: if rag_reachable { "ok" } else { "unreachable" },
            mode: "python-http-or-memory-fallback",
        },
    })
}
