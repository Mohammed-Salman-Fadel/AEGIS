use super::{handlers, state::AppState};
use crate::network::handlers::chat::ChatRequest;
use axum::{
    Json, Router,
    extract::{
        Multipart, State,
        ws::{Message, WebSocketUpgrade},
    },
    http::{HeaderValue, Method, StatusCode},
    routing::{get, post},
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};
use sysinfo::System;
use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;

static SYSTEM_STATS: OnceLock<Mutex<System>> = OnceLock::new();

// SYSTEM RESOURCE MONITORING
async fn get_system_stats() -> Json<Value> {
    let stats = SYSTEM_STATS.get_or_init(|| {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        sys.refresh_memory();
        Mutex::new(sys)
    });

    let mut sys = match stats.lock() {
        Ok(sys) => sys,
        Err(poisoned) => poisoned.into_inner(),
    };

    sys.refresh_cpu_usage();
    sys.refresh_memory();

    let cpu_usage = sys.global_cpu_usage();
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let ram_usage = if total_mem > 0 {
        (used_mem as f32 / total_mem as f32) * 100.0
    } else {
        0.0
    };
    Json(json!({ "cpu": cpu_usage.round() as u32, "ram": ram_usage.round() as u32 }))
}

async fn handle_chat_ws(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|socket| async move {
        let (mut sender, mut receiver) = socket.split();

        while let Some(Ok(Message::Text(text))) = receiver.next().await {
            let msg_data: Value = serde_json::from_str(&text).unwrap_or_default();
            let user_query = msg_data["query"].as_str().unwrap_or("").to_string();

            let mut full_ai_response = String::new();
            let (tx, mut rx) = mpsc::channel::<String>(100);

            let req = ChatRequest {
                session_id: None,
                message: user_query,
                attachments: vec![],
                edit_from_turn_index: None,
            };

            let orchestrator = state.orchestrator.clone();
            tokio::spawn(async move {
                orchestrator.handle(req, tx).await;
            });

            while let Some(token) = rx.recv().await {
                if token == "[DONE]" {
                    break;
                }
                if token.starts_with("[ERROR]") {
                    full_ai_response = token;
                    break;
                }
                full_ai_response.push_str(&token);
            }

            let response = json!({
                "type": "token",
                "content": full_ai_response
            });

            let _ = sender
                .send(Message::Text(response.to_string().into()))
                .await;

            let trace = json!({ "type": "trace", "phase": "Complete" });
            let _ = sender.send(Message::Text(trace.to_string().into())).await;
        }
    })
}

// PROGRESS WS
async fn handle_progress_ws(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        let msg = Message::Text(json!({ "percentage": 100 }).to_string().into());
        let _ = socket.send(msg).await;
    })
}

#[derive(Serialize)]
struct IngestedDocument {
    file_name: String,
    stored_path: String,
    chunks_added: usize,
}

#[derive(Serialize)]
struct IngestResponse {
    status: &'static str,
    total_chunks: usize,
    documents: Vec<IngestedDocument>,
}

async fn handle_pdf_ingest(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<IngestResponse>, (StatusCode, String)> {
    let ingest_dir = std::env::current_dir()
        .unwrap_or_default()
        .join("data")
        .join("ingest");
    tokio::fs::create_dir_all(&ingest_dir)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not create ingest directory: {error}"),
            )
        })?;

    let mut documents = Vec::new();
    let mut total_chunks = 0;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("Could not read multipart upload: {error}"),
        )
    })? {
        let Some(file_name) = safe_upload_file_name(field.file_name()) else {
            continue;
        };

        if !is_supported_ingest_file(&file_name) {
            return Err((
                StatusCode::BAD_REQUEST,
                format!("Unsupported file type for `{file_name}`. Only PDF and TXT are supported."),
            ));
        }

        let data = field.bytes().await.map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                format!("Could not read uploaded file `{file_name}`: {error}"),
            )
        })?;

        let file_path = ingest_dir.join(&file_name);
        tokio::fs::write(&file_path, data).await.map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not store uploaded file `{file_name}`: {error}"),
            )
        })?;

        let stored_path = file_path.to_string_lossy().to_string();
        let outcome = state
            .orchestrator
            .rag_client
            .ingest(stored_path.clone())
            .await
            .map_err(|error| {
                (
                    StatusCode::BAD_GATEWAY,
                    format!(
                        "Document was uploaded, but RAG indexing failed for `{file_name}`: {error}"
                    ),
                )
            })?;

        if outcome.chunks_added == 0 {
            return Err((
                StatusCode::UNPROCESSABLE_ENTITY,
                format!(
                    "Document `{file_name}` was uploaded, but no readable text chunks were indexed."
                ),
            ));
        }

        total_chunks += outcome.chunks_added;
        documents.push(IngestedDocument {
            file_name,
            stored_path,
            chunks_added: outcome.chunks_added,
        });
    }

    if documents.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "No supported document files were received.".to_string(),
        ));
    }

    Ok(Json(IngestResponse {
        status: "indexed",
        total_chunks,
        documents,
    }))
}

fn safe_upload_file_name(raw_file_name: Option<&str>) -> Option<String> {
    let raw_file_name = raw_file_name?.trim();
    if raw_file_name.is_empty() {
        return None;
    }

    Path::new(raw_file_name)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}

fn is_supported_ingest_file(file_name: &str) -> bool {
    let lower = file_name.to_lowercase();
    lower.ends_with(".pdf") || lower.ends_with(".txt")
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:5173".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/chat", post(handlers::chat::chat))
        .route(
            "/providers/current",
            get(handlers::providers::current_provider),
        )
        .route("/providers", get(handlers::providers::list_providers))
        .route(
            "/providers/select",
            post(handlers::providers::select_provider),
        )
        .route("/models", get(handlers::models::list_models))
        .route("/models/current", get(handlers::models::current_model))
        .route("/models/select", post(handlers::models::select_model))
        .route("/calendar/event", post(handlers::calendar::create_event))
        .route(
            "/calendar/create-from-prompt",
            post(handlers::calendar::create_from_prompt),
        )
        .route(
            "/calendar/outlook/calendars",
            get(handlers::calendar::list_outlook_calendars),
        )
        .route(
            "/calendar/outlook/select",
            post(handlers::calendar::select_outlook_calendar),
        )
        .route("/system/stats", get(get_system_stats))
        .route("/ingest", post(handle_pdf_ingest)) // State desteği eklendi
        .route("/index/progress", get(handle_progress_ws))
        .route("/chat/stream", get(handle_chat_ws))
        .route(
            "/sessions",
            get(handlers::sessions::list_sessions).post(handlers::sessions::create_session),
        )
        .route(
            "/sessions/{session_id}",
            get(handlers::sessions::get_session)
                .patch(handlers::sessions::rename_session)
                .delete(handlers::sessions::delete_session),
        )
        .layer(cors)
        .with_state(state)
}
