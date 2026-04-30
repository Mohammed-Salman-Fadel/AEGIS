use axum::{Json, extract::{Path, State}, http::StatusCode};
use serde::Serialize;

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
            .map(|provider| ProviderResponse {
                name: provider.name,
                description: provider.description,
                active: provider.active,
            })
            .collect(),
    })
}

pub async fn select_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<ProviderSelectResponse>, (StatusCode, String)> {
    let result = state
        .orchestrator
        .select_provider(&name)
        .map_err(provider_error)?;

    Ok(Json(ProviderSelectResponse {
        previous: result.previous,
        current: result.current,
        persisted: false,
        message: result.message,
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
