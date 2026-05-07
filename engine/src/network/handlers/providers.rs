use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};

use crate::network::state::AppState;

#[derive(Serialize)]
pub struct ProviderListResponse {
    providers: Vec<ProviderResponse>,
}

#[derive(Serialize)]
pub struct ProviderResponse {
    name: String,
    description: String,
    active: bool,
}

#[derive(Serialize)]
pub struct CurrentProviderResponse {
    provider: String,
}

#[derive(Deserialize)]
pub struct SelectProviderRequest {
    name: String,
}

#[derive(Serialize)]
pub struct ProviderSelectResponse {
    previous: String,
    current: String,
    persisted: bool,
    message: String,
}

pub async fn list_providers(State(state): State<AppState>) -> Json<ProviderListResponse> {
    Json(ProviderListResponse {
        providers: state
            .orchestrator
            .list_providers()
            .into_iter()
            .map(|(name, description, active)| ProviderResponse {
                name,
                description,
                active,
            })
            .collect(),
    })
}

pub async fn current_provider(State(state): State<AppState>) -> Json<CurrentProviderResponse> {
    Json(CurrentProviderResponse {
        provider: state.orchestrator.current_provider_name(),
    })
}

pub async fn select_provider(
    State(state): State<AppState>,
    Json(payload): Json<SelectProviderRequest>,
) -> Result<Json<ProviderSelectResponse>, (StatusCode, String)> {
    let outcome = state
        .orchestrator
        .switch_provider(&payload.name)
        .await
        .map_err(provider_error)?;

    let message = if outcome.changed {
        format!(
            "Switched from {} to {}.",
            outcome.previous_provider, outcome.current_provider
        )
    } else {
        format!(
            "`{}` is already the active provider.",
            outcome.current_provider
        )
    };

    Ok(Json(ProviderSelectResponse {
        previous: outcome.previous_provider,
        current: outcome.current_provider,
        persisted: true,
        message,
    }))
}

fn provider_error(error: anyhow::Error) -> (StatusCode, String) {
    let message = error.to_string();
    let status = if message.contains("unsupported inference provider") {
        StatusCode::BAD_REQUEST
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    };

    (status, message)
}
