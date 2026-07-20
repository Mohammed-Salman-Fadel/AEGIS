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
use serde_json::json;
use std::convert::Infallible;
use tokio::sync::mpsc;
use tokio::time::{Duration, sleep};
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
    quantization: Option<String>,
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

#[derive(Serialize)]
struct LmStudioDownloadRequest<'a> {
    model: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    quantization: Option<&'a str>,
}

#[derive(Deserialize)]
struct LmStudioDownloadStatus {
    job_id: Option<String>,
    status: String,
    total_size_bytes: Option<u64>,
    downloaded_bytes: Option<u64>,
    bytes_per_second: Option<u64>,
    error: Option<String>,
    message: Option<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    warning: Option<String>,
}

#[derive(Serialize)]
pub struct ModelResponse {
    name: String,
    description: String,
    active: bool,
    status: &'static str,
    provider: String,
    supports_managed_download: bool,
}

pub async fn list_models(
    State(state): State<AppState>,
) -> Result<Json<ModelListResponse>, (StatusCode, String)> {
    let provider = state.orchestrator.current_provider_name();
    let active_model = state.orchestrator.current_model_name();
    let (model_names, warning) = match state.orchestrator.list_available_models().await {
        Ok((_, model_names)) if !model_names.is_empty() => (model_names, None),
        Ok((_, _)) => (
            vec![active_model.clone()],
            Some(format!(
                "{provider} responded, but did not report any models. Showing the active model until its model catalog refreshes."
            )),
        ),
        Err(error) => (
            vec![active_model.clone()],
            Some(format!(
                "Could not reach {provider} to refresh installed models. Showing the active model instead: {error}"
            )),
        ),
    };
    let degraded = warning.is_some();

    Ok(Json(ModelListResponse {
        provider: provider.clone(),
        warning,
        models: model_names
            .into_iter()
            .map(|name| {
                let active = name.eq_ignore_ascii_case(&active_model);
                ModelResponse {
                    active,
                    description: if degraded {
                        "Provider model discovery is unavailable; this is the last active model known to AEGIS.".to_string()
                    } else if active {
                        "Currently active in the engine.".to_string()
                    } else {
                        String::new()
                    },
                    name,
                    status: if degraded {
                        "degraded"
                    } else if active {
                        "ready"
                    } else {
                        "installed"
                    },
                    provider: provider.clone(),
                    supports_managed_download: matches!(provider.as_str(), "ollama" | "lmstudio"),
                }
            })
            .collect(),
    }))
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
        format!(
            "`{}` is already the active model and has been warmed in memory.",
            outcome.current_model
        )
    } else if let Some(warning) = &outcome.unload_warning {
        format!(
            "Switched from {} to {}. {}",
            outcome.previous_model, outcome.current_model, warning
        )
    } else {
        format!(
            "Switched from {} to {}. The previous model was unloaded and the selected model is warmed in memory.",
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
        warning: None,
        models: model_names
            .into_iter()
            .map(|name| {
                let active = name.eq_ignore_ascii_case(&active_model);
                ModelResponse {
                    active,
                    description: if active {
                        "Currently active in the engine.".to_string()
                    } else {
                        String::new()
                    },
                    name,
                    status: if active { "ready" } else { "installed" },
                    provider: "ollama".to_string(),
                    supports_managed_download: true,
                }
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

pub async fn download_model(
    State(state): State<AppState>,
    Json(payload): Json<PullModelRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    let model = payload.name.trim().to_string();
    if model.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "The model name cannot be empty.".to_string(),
        ));
    }

    let provider = state.orchestrator.current_provider_name();
    let quantization = payload
        .quantization
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let (tx, rx) = mpsc::channel::<String>(32);
    tokio::spawn(async move {
        let result: anyhow::Result<()> = match provider.as_str() {
            "ollama" => stream_ollama_pull(&model, tx.clone()).await,
            "lmstudio" => {
                stream_lmstudio_download(&model, quantization.as_deref(), tx.clone()).await
            }
            other => Err(anyhow::anyhow!(
                "Model downloads are not supported for provider `{other}`. Switch to Ollama or LM Studio first."
            )),
        };

        if let Err(error) = result {
            let _ = tx
                .send(format!(
                    r#"{{"error":{},"status":"failed"}}"#,
                    serde_json::to_string(&error.to_string())
                        .unwrap_or_else(|_| { "\"Model download failed.\"".to_string() })
                ))
                .await;
        }
    });

    let stream = ReceiverStream::new(rx).map(|message| Ok(Event::default().data(message)));
    Ok(Sse::new(stream))
}

pub async fn pull_ollama_model(
    State(state): State<AppState>,
    payload: Json<PullModelRequest>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, String)> {
    download_model(State(state), payload).await
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

async fn stream_lmstudio_download(
    model: &str,
    quantization: Option<&str>,
    tx: mpsc::Sender<String>,
) -> anyhow::Result<()> {
    let base_url = lmstudio_management_base_url();
    let client = reqwest::Client::new();

    // HuggingFace models (no ':' separator, e.g. "mistral-7b-instruct-v0.3-gguf")
    // need the full HuggingFace URL instead of a bare model name
    let download_model = if model.contains(':') {
        // Ollama-style name like "llama3.2:1b" — pass as-is
        model.to_string()
    } else {
        // HuggingFace-style name — wrap in lmstudio-community URL
        let slug = model
            .trim()
            .to_lowercase()
            .replace('_', "-")
            .replace(' ', "-");
        format!("https://huggingface.co/lmstudio-community/{slug}")
    };

    let response = with_lmstudio_auth(
        client
            .post(format!("{base_url}/api/v1/models/download"))
            .json(&LmStudioDownloadRequest {
                model: &download_model,
                quantization,
            }),
    )
    .send()
    .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        // Try to extract a HuggingFace URL from LM Studio's error response
        let hint = if let Ok(val) = serde_json::from_str::<serde_json::Value>(&body) {
            val.pointer("/error/message")
                .and_then(|m| m.as_str())
                .and_then(|msg| {
                    msg.split_whitespace()
                        .find(|w| w.starts_with("https://huggingface.co/"))
                        .map(|url| format!("\n\nTo download this model, use the HuggingFace URL instead:\n  {url}"))
                })
                .unwrap_or_default()
        } else {
            String::new()
        };
        anyhow::bail!(
            "LM Studio download error {status} for model `{model}`:{hint}\n\nBody: {body}"
        );
    }

    let initial_status = response.json::<LmStudioDownloadStatus>().await?;
    send_lmstudio_status(&tx, &initial_status).await?;

    if is_lmstudio_terminal_success(&initial_status.status) {
        send_download_success(&tx).await;
        return Ok(());
    }

    if is_lmstudio_terminal_failure(&initial_status.status) {
        anyhow::bail!("{}", lmstudio_failure_message(model, &initial_status));
    }

    let Some(job_id) = initial_status.job_id.clone() else {
        anyhow::bail!(
            "LM Studio started a model download for `{model}` but did not return a job id."
        );
    };

    loop {
        sleep(Duration::from_secs(1)).await;

        let response = with_lmstudio_auth(
            client.get(format!("{base_url}/api/v1/models/download/status/{job_id}")),
        )
        .send()
        .await?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("LM Studio download status error {status} for `{model}`: {body}");
        }

        let download_status = response.json::<LmStudioDownloadStatus>().await?;
        send_lmstudio_status(&tx, &download_status).await?;

        if is_lmstudio_terminal_success(&download_status.status) {
            send_download_success(&tx).await;
            return Ok(());
        }

        if is_lmstudio_terminal_failure(&download_status.status) {
            anyhow::bail!("{}", lmstudio_failure_message(model, &download_status));
        }
    }
}

async fn send_lmstudio_status(
    tx: &mpsc::Sender<String>,
    status: &LmStudioDownloadStatus,
) -> anyhow::Result<()> {
    let label = match status.status.as_str() {
        "already_downloaded" => "already downloaded",
        "completed" => "completed",
        "downloading" => "downloading",
        "paused" => "paused",
        "failed" => "failed",
        other => other,
    };

    let payload = json!({
        "status": label,
        "total": status.total_size_bytes,
        "completed": status.downloaded_bytes,
        "bytes_per_second": status.bytes_per_second,
        "error": status.error.as_deref().or(status.message.as_deref()).filter(|_| {
            is_lmstudio_terminal_failure(&status.status)
        }),
    });

    if tx.send(payload.to_string()).await.is_err() {
        anyhow::bail!("download stream closed");
    }

    Ok(())
}

async fn send_download_success(tx: &mpsc::Sender<String>) {
    let _ = tx
        .send(r#"{"status":"success","completed":1,"total":1}"#.to_string())
        .await;
}

fn is_lmstudio_terminal_success(status: &str) -> bool {
    matches!(status, "completed" | "already_downloaded")
}

fn is_lmstudio_terminal_failure(status: &str) -> bool {
    matches!(status, "failed")
}

fn lmstudio_failure_message(model: &str, status: &LmStudioDownloadStatus) -> String {
    status
        .error
        .as_deref()
        .or(status.message.as_deref())
        .map(str::to_string)
        .unwrap_or_else(|| format!("LM Studio failed to download `{model}`."))
}

fn lmstudio_management_base_url() -> String {
    let raw = std::env::var("AEGIS_LM_STUDIO_URL")
        .or_else(|_| std::env::var("AEGIS_LMSTUDIO_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());

    raw.trim_end_matches('/')
        .strip_suffix("/v1")
        .unwrap_or(raw.trim_end_matches('/'))
        .to_string()
}

fn with_lmstudio_auth(builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
    let token = std::env::var("AEGIS_LM_STUDIO_API_TOKEN")
        .or_else(|_| std::env::var("LM_API_TOKEN"))
        .ok()
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty());

    if let Some(token) = token {
        builder.bearer_auth(token)
    } else {
        builder
    }
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
