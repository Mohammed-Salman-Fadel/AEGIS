//! Role: HTTP handlers for reading and changing the active engine model.
//! Called by: `network/router.rs` when the CLI or future web UI hits `/models/*`.
//! Calls into: `network/state.rs` and the orchestrator's model registry methods.
//! Owns: request/response shapes for model status and model switching.
//! Does not own: Ollama model discovery, inference execution, or CLI rendering.
//! Next TODOs: validate requested models against the provider catalog before accepting switches.

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::sse::{Event, Sse},
};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;

use crate::network::state::AppState;

#[derive(Serialize)]
pub struct CurrentModelResponse {
    model: String,
}

#[derive(Deserialize)]
pub struct SelectModelRequest {
    name: String,
}

#[derive(Deserialize)]
pub struct PullModelRequest {
    name: String,
}

#[derive(Serialize)]
struct OllamaPullRequest<'a> {
    model: &'a str,
    stream: bool,
}

#[derive(Deserialize, Serialize)]
struct OllamaPullChunk {
    status: Option<String>,
    digest: Option<String>,
    total: Option<u64>,
    completed: Option<u64>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModelEntry>,
}

#[derive(Deserialize)]
struct OllamaModelEntry {
    name: String,
}

#[derive(Serialize)]
pub struct SelectModelResponse {
    previous: String,
    current: String,
    persisted: bool,
    message: String,
}

#[derive(Serialize)]
pub struct ModelListResponse {
    provider: String,
    models: Vec<ModelResponse>,
}

#[derive(Serialize)]
pub struct ModelResponse {
    name: String,
    description: String,
    active: bool,
}

pub async fn list_models(State(state): State<AppState>) -> Json<ModelListResponse> {
    let (provider, model_names) = state
        .orchestrator
        .list_available_models()
        .await
        .unwrap_or_else(|_| (state.orchestrator.current_provider_name(), vec![]));
    let active_model = state.orchestrator.current_model_name();

    Json(ModelListResponse {
        provider,
        models: model_names
            .into_iter()
            .map(|name| ModelResponse {
                active: name.eq_ignore_ascii_case(&active_model),
                description: if name.eq_ignore_ascii_case(&active_model) {
                    "Currently active in the engine.".to_string()
                } else {
                    String::new()
                },
                name,
            })
            .collect(),
    })
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

pub async fn list_ollama_models(State(state): State<AppState>) -> Json<ModelListResponse> {
    let active_model = state.orchestrator.current_model_name();
    let model_names = fetch_ollama_models().await.unwrap_or_default();

    Json(ModelListResponse {
        provider: "ollama".to_string(),
        models: model_names
            .into_iter()
            .map(|name| ModelResponse {
                active: name.eq_ignore_ascii_case(&active_model),
                description: if name.eq_ignore_ascii_case(&active_model) {
                    "Currently active in the engine.".to_string()
                } else {
                    String::new()
                },
                name,
            })
            .collect(),
    })
}

async fn fetch_ollama_models() -> anyhow::Result<Vec<String>> {
    let base_url = ollama_base_url();
    let response = reqwest::Client::new()
        .get(format!("{base_url}/api/tags"))
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("ollama tags error {status}: {body}");
    }

    let payload = response.json::<OllamaTagsResponse>().await?;
    Ok(payload.models.into_iter().map(|model| model.name).collect())
}

pub async fn pull_ollama_model(
    Json(payload): Json<PullModelRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let model = payload.name.trim().to_string();
    if model.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "The model name cannot be empty.".to_string(),
        ));
    }

    let (tx, rx) = mpsc::channel::<String>(32);
    tokio::spawn(async move {
        if let Err(error) = stream_ollama_pull(&model, tx.clone()).await {
            let _ = tx
                .send(format!(
                    r#"{{"error":{},"status":"failed"}}"#,
                    serde_json::to_string(&error.to_string()).unwrap_or_else(|_| {
                        "\"Model download failed.\"".to_string()
                    })
                ))
                .await;
        }
    });

    let stream = ReceiverStream::new(rx).map(|message| Ok(Event::default().data(message)));
    Ok(Sse::new(stream))
}

async fn stream_ollama_pull(model: &str, tx: mpsc::Sender<String>) -> anyhow::Result<()> {
    let base_url = ollama_base_url();

    let response = reqwest::Client::new()
        .post(format!("{base_url}/api/pull"))
        .json(&OllamaPullRequest {
            model,
            stream: true,
        })
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("ollama pull error {status} for model `{model}`: {body}");
    }

    let mut pending = String::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        pending.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(newline_index) = pending.find('\n') {
            let line = pending[..newline_index].trim().to_string();
            pending = pending[newline_index + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            let parsed: OllamaPullChunk = serde_json::from_str(&line)?;
            if let Some(error) = parsed.error.as_deref() {
                anyhow::bail!("ollama pull error for model `{model}`: {error}");
            }

            if tx.send(serde_json::to_string(&parsed)?).await.is_err() {
                return Ok(());
            }
        }
    }

    if !pending.trim().is_empty() {
        let parsed: OllamaPullChunk = serde_json::from_str(pending.trim())?;
        if tx.send(serde_json::to_string(&parsed)?).await.is_err() {
            return Ok(());
        }
    }

    let _ = tx
        .send(r#"{"status":"success","completed":1,"total":1}"#.to_string())
        .await;
    Ok(())
}

fn ollama_base_url() -> String {
    std::env::var("AEGIS_OLLAMA_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string())
        .trim_end_matches('/')
        .to_string()
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
