use super::{handlers, state::AppState};
use crate::memory_store::Session;
use crate::network::handlers::chat::ChatRequest;
use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Multipart, State,
        ws::{Message, WebSocketUpgrade},
    },
    http::{Method, StatusCode},
    response::IntoResponse,
    routing::{delete, get, post},
};
use futures::{sink::SinkExt, stream::StreamExt};
use rust_embed::Embed;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    collections::HashMap,
};
use sysinfo::System;
use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;
use tracing::warn;
use axum::body::Body;
use axum::http::Request;
use axum::response::Response;

/// Embedded frontend dist/ using rust-embed.
/// The build.rs ensures dist/ is populated before engine compiles.
#[derive(Embed)]
#[folder = "../frontend/dist/"]
#[prefix = ""]
struct FrontendAssets;

/// Minimal MIME type map for static file serving.
fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "css"  => "text/css; charset=utf-8",
        "js"   => "application/javascript",
        "svg"  => "image/svg+xml",
        "png"  => "image/png",
        "ico"  => "image/x-icon",
        "json" => "application/json",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf"  => "font/ttf",
        _      => "application/octet-stream",
    }
}

/// Serves installer/download files from a configurable directory on disk.
/// Uses AEGIS_DOWNLOADS_DIR env var, or defaults to `downloads/` next to the frontend dist.
async fn handle_download(axum::extract::Path(path): axum::extract::Path<String>) -> Response<Body> {
    let downloads_dir = std::env::var("AEGIS_DOWNLOADS_DIR").unwrap_or_else(|_| {
        // Default: alongside frontend/dist/ (repo root/downloads/)
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest.parent().unwrap_or(&manifest).join("downloads").to_string_lossy().to_string()
    });

    let file_path = PathBuf::from(&downloads_dir).join(&path);

    // Prevent directory traversal
    if file_path.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .body(Body::from("Forbidden"))
            .unwrap();
    }

    match tokio::fs::read(&file_path).await {
        Ok(bytes) => {
            let mime = mime_type(&file_path);
            Response::builder()
                .header("Content-Type", mime)
                .header("Content-Disposition", &format!("attachment; filename=\"{}\"", file_path.file_name().unwrap_or_default().to_string_lossy()))
                .body(Body::from(bytes))
                .unwrap_or_else(|_| Response::new(Body::from("Internal error")))
        }
        Err(_) => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("File not found"))
            .unwrap(),
    }
}

/// Serves the embedded React frontend from the compiled binary.
/// Returns 404 JSON for any /api/* path not matched by routes (API guard).
/// SPA fallback: any unknown non-API path returns index.html.
async fn handle_static(uri: axum::http::Uri) -> Response<Body> {
    let requested_path = uri.path().trim_start_matches('/');

    // API guard: if path starts with api/ and it wasn't caught by routes, return 404 JSON
    if requested_path.starts_with("api/") {
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"error":"API endpoint not found"}"#))
            .unwrap();
    }

    // Default to index.html for root or SPA routes
    let asset_path = if requested_path.is_empty() || !FrontendAssets::get(requested_path).is_some() {
        "index.html"
    } else {
        requested_path
    };

    match FrontendAssets::get(asset_path) {
        Some(content) => {
            let mime = mime_type(Path::new(asset_path));
            Response::builder()
                .header("Content-Type", mime)
                .header("Cache-Control", "no-cache, no-store, must-revalidate")
                .body(Body::from(content.data.to_vec()))
                .unwrap_or_else(|_| Response::new(Body::from("Internal error")))
        }
        None => {
            // Final SPA fallback: serve index.html
            match FrontendAssets::get("index.html") {
                Some(content) => Response::builder()
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(Body::from(content.data.to_vec()))
                    .unwrap(),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("Not found"))
                    .unwrap(),
            }
        }
    }
}

static SYSTEM_STATS: OnceLock<Mutex<System>> = OnceLock::new();
const MAX_INGEST_UPLOAD_BYTES: usize = 100 * 1024 * 1024;

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
            let mode = msg_data["mode"].as_str().map(|s| s.to_string());

            let mut full_ai_response = String::new();
            let (tx, mut rx) = mpsc::channel::<String>(100);

            let rag_enabled = msg_data["rag_enabled"].as_bool();
            let rag_top_k = msg_data["rag_top_k"].as_u64().map(|v| v as usize);
            let rag_similarity_threshold = msg_data["rag_similarity_threshold"].as_f64();

            let req = ChatRequest {
                session_id: None,
                message: user_query,
                attachments: vec![],
                edit_from_turn_index: None,
                mode,
                response_style: None,
                code_project_name: None,
                code_project_path: None,
                code_project_context: None,
                code_project_id: None,                rag_enabled,
                rag_top_k,
                rag_similarity_threshold,
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
    session: Option<Session>,
}

struct PendingUpload {
    file_name: String,
    data: axum::body::Bytes,
}

#[derive(Deserialize)]
struct DeleteIngestedDocumentRequest {
    session_id: String,
    stored_path: String,
}

#[derive(Serialize)]
struct DeleteIngestedDocumentResponse {
    status: &'static str,
    deleted_chunks: usize,
}

async fn handle_pdf_ingest(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<IngestResponse>, (StatusCode, String)> {
    let ingest_dir = ingest_storage_dir();
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
    let mut session_id = None;
    let mut uploads = Vec::new();

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("Could not read multipart upload: {error}"),
        )
    })? {
        if field.name() == Some("session_id") {
            let value = field.text().await.map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Could not read ingest session id: {error}"),
                )
            })?;
            let value = value.trim().to_string();
            if !value.is_empty() {
                session_id = Some(value);
            }
            continue;
        }

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

        uploads.push(PendingUpload { file_name, data });
    }

    let session_id = session_id.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "A session id is required before importing documents.".to_string(),
        )
    })?;

    for upload in uploads {
        let file_name = upload.file_name;
        let file_path = ingest_dir.join(&file_name);
        tokio::fs::write(&file_path, upload.data)
            .await
            .map_err(|error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Could not store uploaded file `{file_name}`: {error}"),
                )
            })?;

        let stored_path = file_path.to_string_lossy().to_string();
        let outcome = state
            .orchestrator
            .rag_client
            .ingest(stored_path.clone(), &session_id)
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

    let document_names = documents
        .iter()
        .map(|document| document.file_name.clone())
        .collect::<Vec<_>>();
    let session = match state
        .orchestrator
        .title_session_from_import(&session_id, &document_names)
        .await
    {
        Ok(session) => Some(session),
        Err(error) => {
            warn!(
                session_id,
                "Document import succeeded, but session title generation failed: {error}"
            );
            state
                .orchestrator
                .get_session(&session_id)
                .await
                .ok()
                .flatten()
        }
    };

    Ok(Json(IngestResponse {
        status: "indexed",
        total_chunks,
        documents,
        session,
    }))
}

async fn handle_ingest_document_delete(
    State(state): State<AppState>,
    Json(request): Json<DeleteIngestedDocumentRequest>,
) -> Result<Json<DeleteIngestedDocumentResponse>, (StatusCode, String)> {
    let session_id = request.session_id.trim().to_string();
    let stored_path = request.stored_path.trim().to_string();

    if session_id.is_empty() || stored_path.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Both session_id and stored_path are required to remove an indexed document."
                .to_string(),
        ));
    }

    let deleted_chunks = state
        .orchestrator
        .rag_client
        .delete_document(&session_id, &stored_path)
        .await
        .map_err(|error| {
            (
                StatusCode::BAD_GATEWAY,
                format!("Could not remove document chunks from RAG memory: {error}"),
            )
        })?;

    remove_stored_ingest_file(&stored_path).await?;

    Ok(Json(DeleteIngestedDocumentResponse {
        status: "deleted",
        deleted_chunks,
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

fn ingest_storage_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_default()
        .join("data")
        .join("ingest")
}

async fn remove_stored_ingest_file(stored_path: &str) -> Result<(), (StatusCode, String)> {
    let path = Path::new(stored_path);
    let Ok(canonical_path) = tokio::fs::canonicalize(path).await else {
        return Ok(());
    };

    let Ok(canonical_ingest_dir) = tokio::fs::canonicalize(ingest_storage_dir()).await else {
        return Ok(());
    };

    if !canonical_path.starts_with(canonical_ingest_dir) {
        return Ok(());
    }

    match tokio::fs::remove_file(&canonical_path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Removed document chunks, but could not delete stored upload file: {error}"),
        )),
    }
}

async fn handle_voice_transcribe(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<Value>, (StatusCode, String)> {
    let mut audio_data = None;

    while let Some(field) = multipart.next_field().await.map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            format!("Could not read multipart voice data: {error}"),
        )
    })? {
        if field.name() == Some("file") {
            audio_data = Some(field.bytes().await.map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    format!("Could not read audio bytes: {error}"),
                )
            })?);
            break;
        }
    }

    let audio_data = audio_data.ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            "No audio file provided.".to_string(),
        )
    })?;

    let text = state
        .orchestrator
        .rag_client
        .transcribe(audio_data.to_vec())
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    Ok(Json(json!({ "text": text })))
}

async fn handle_voice_synthesize(
    State(state): State<AppState>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<impl axum::response::IntoResponse, (StatusCode, String)> {
    let text = params.get("text").cloned().unwrap_or_default();
    if text.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "No text provided".to_string()));
    }

    let audio_bytes = state
        .orchestrator
        .rag_client
        .synthesize(text)
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    Ok((
        [(axum::http::header::CONTENT_TYPE, "audio/wav")],
        audio_bytes,
    ))
}

#[derive(Deserialize)]
struct VoiceConfigRequest {
    keep_cached: bool,
}

async fn handle_voice_config(
    State(state): State<AppState>,
    Json(payload): Json<VoiceConfigRequest>,
) -> Result<Json<Value>, (StatusCode, String)> {
    state
        .orchestrator
        .rag_client
        .configure_voice(payload.keep_cached)
        .await
        .map_err(|error| (StatusCode::INTERNAL_SERVER_ERROR, error.to_string()))?;

    Ok(Json(
        json!({ "status": "ok", "keep_cached": payload.keep_cached }),
    ))
}

pub fn create_router(state: AppState) -> Router {
    // CORS: in production (embedded frontend, same-origin), no CORS needed.
    // In dev mode (Vite on port 5173), allow only the dev server origin.
    // Gate permissive CORS behind AEGIS_DEV_CORS env var for debugging.
    let cors = if std::env::var_os("AEGIS_DEV_CORS").is_some() {
        CorsLayer::permissive()
    } else {
        CorsLayer::new()
            .allow_origin([
                "http://127.0.0.1:5173".parse().unwrap(),
                "http://localhost:5173".parse().unwrap(),
            ])
            .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::PATCH])
            .allow_headers([axum::http::header::CONTENT_TYPE, axum::http::header::AUTHORIZATION])
    };

    Router::new()
        // Legacy /health for backward compat
        .route("/health", get(handlers::health::health))
        // All API routes under /api prefix
        .route("/api/health", get(handlers::health::health))
        .route("/api/chat", post(handlers::chat::chat))
        .route(
            "/api/providers/current",
            get(handlers::providers::current_provider),
        )
        .route("/api/providers", get(handlers::providers::list_providers))
        .route(
            "/api/providers/select",
            post(handlers::providers::select_provider),
        )
        .route("/api/models", get(handlers::models::list_models))
        .route("/api/models/ollama", get(handlers::models::list_ollama_models))
        .route("/api/models/current", get(handlers::models::current_model))
        .route("/api/models/select", post(handlers::models::select_model))
        .route("/api/models/download", post(handlers::models::download_model))
        .route("/api/models/pull", post(handlers::models::pull_ollama_model))
        .route(
            "/api/profile",
            get(handlers::profile::get_profile).put(handlers::profile::save_profile),
        )
        .route("/api/context/usage", get(handlers::context::usage))
        .route("/api/calendar/event", post(handlers::calendar::create_event))
        .route(
            "/api/calendar/create-from-prompt",
            post(handlers::calendar::create_from_prompt),
        )
        .route(
            "/api/calendar/outlook/calendars",
            get(handlers::calendar::list_outlook_calendars),
        )
        .route(
            "/api/calendar/outlook/select",
            post(handlers::calendar::select_outlook_calendar),
        )
        .route("/api/system/stats", get(get_system_stats))
        .route(
            "/api/projects/ingest",
            post(handlers::projects::ingest_project_files),
        )
        .route(
            "/api/ingest",
            post(handle_pdf_ingest).layer(DefaultBodyLimit::max(MAX_INGEST_UPLOAD_BYTES)),
        )
        .route("/api/ingest/document", delete(handle_ingest_document_delete))
        .route("/api/index/progress", get(handle_progress_ws))
        .route("/api/chat/stream", get(handle_chat_ws))
        .route("/api/voice/transcribe", post(handle_voice_transcribe))
        .route("/api/voice/synthesize", get(handle_voice_synthesize))
        .route("/api/voice/config", post(handle_voice_config))
        .route(
            "/api/mcp/obsidian/validate",
            get(handlers::mcp::validate_obsidian_path),
        )
        .route(
            "/api/mcp/obsidian/graph",
            post(handlers::mcp::build_obsidian_graph),
        )
        .route(
            "/api/mcp/obsidian/list-notes",
            post(handlers::mcp::list_vault_notes),
        )
        .route(
            "/api/mcp/obsidian/read",
            post(handlers::mcp::read_vault_note),
        )
        .route(
            "/api/mcp/obsidian/file",
            get(handlers::mcp::serve_vault_file),
        )
        .route(
            "/api/mcp/obsidian/search",
            post(handlers::mcp::search_vault_notes),
        )
        .route(
            "/api/mcp/obsidian/write",
            post(handlers::mcp::write_vault_note),
        )
        .route(
            "/api/mcp/{provider}/{tool}",
            post(handlers::mcp::call_mcp_tool),
        )
        .route(
            "/api/sessions",
            get(handlers::sessions::list_sessions).post(handlers::sessions::create_session),
        )
        .route(
            "/api/sessions/{session_id}",
            get(handlers::sessions::get_session)
                .patch(handlers::sessions::rename_session)
                .delete(handlers::sessions::delete_session),
        )
        // Download route: serves installer binary from a configurable directory.
        // Set AEGIS_DOWNLOADS_DIR env var, or defaults to a `downloads/` folder
        // alongside the frontend dist/ directory.
        .route("/downloads/{*path}", get(handle_download))
        .fallback(handle_static)
        .layer(cors)
        .with_state(state)
}
