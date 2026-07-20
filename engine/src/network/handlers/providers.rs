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
    capabilities: ProviderCapabilitiesResponse,
    notes: Vec<String>,
}

#[derive(Serialize)]
pub struct ProviderCapabilitiesResponse {
    chat: bool,
    streaming: bool,
    model_listing: bool,
    model_download: bool,
    model_unload: bool,
    context_window_detection: bool,
    requires_external_app: bool,
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
    previous_model: String,
    current_model: String,
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
                capabilities: ProviderCapabilitiesResponse {
                    chat: provider.capabilities.chat,
                    streaming: provider.capabilities.streaming,
                    model_listing: provider.capabilities.model_listing,
                    model_download: provider.capabilities.model_download,
                    model_unload: provider.capabilities.model_unload,
                    context_window_detection: provider.capabilities.context_window_detection,
                    requires_external_app: provider.capabilities.requires_external_app,
                },
                notes: provider.capabilities.notes,
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
        let mut message = format!(
            "Switched from {} to {} and selected `{}` as the active model.",
            outcome.previous_provider, outcome.current_provider, outcome.current_model
        );
        if let Some(warning) = &outcome.unload_warning {
            message.push(' ');
            message.push_str(warning);
        }
        if let Some(warning) = &outcome.warm_warning {
            message.push(' ');
            message.push_str(warning);
        }
        message
    } else {
        format!(
            "`{}` is already the active provider using `{}`.",
            outcome.current_provider, outcome.current_model
        )
    };

    Ok(Json(ProviderSelectResponse {
        previous: outcome.previous_provider,
        current: outcome.current_provider,
        previous_model: outcome.previous_model,
        current_model: outcome.current_model,
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
