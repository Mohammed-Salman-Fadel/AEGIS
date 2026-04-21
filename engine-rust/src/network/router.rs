use std::path::PathBuf;

use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        Multipart, State,
    },
    http::{Method, StatusCode},
    routing::{get, post},
    Json, Router,
};
use futures::{sink::SinkExt, stream::StreamExt};
use serde_json::{json, Value};
use sysinfo::System;
use tokio::sync::mpsc;
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
};

use super::{handlers, state::AppState};
use crate::network::handlers::chat::ChatRequest;

async fn get_system_stats() -> Json<Value> {
    let mut sys = System::new_all();
    sys.refresh_all();
    let cpu_usage = sys.global_cpu_usage();
    let total_mem = sys.total_memory();
    let used_mem = sys.used_memory();
    let ram_usage = if total_mem > 0 {
        (used_mem as f32 / total_mem as f32) * 100.0
    } else {
        0.0
    };

    Json(json!({
        "cpu": cpu_usage.round() as u32,
        "ram": ram_usage.round() as u32
    }))
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

            let _ = sender.send(Message::Text(response.to_string().into())).await;

            let trace = json!({ "type": "trace", "phase": "Complete" });
            let _ = sender.send(Message::Text(trace.to_string().into())).await;
        }
    })
}

async fn handle_progress_ws(ws: WebSocketUpgrade) -> impl axum::response::IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        let msg = Message::Text(json!({ "percentage": 100 }).to_string().into());
        let _ = socket.send(msg).await;
    })
}

async fn handle_pdf_ingest(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> StatusCode {
    while let Ok(Some(field)) = multipart.next_field().await {
        if let Ok(data) = field.bytes().await {
            let text = String::from_utf8_lossy(&data).to_string();
            let _ = state.orchestrator.rag_client.ingest(text).await;
        }
    }

    StatusCode::OK
}

pub fn create_router(state: AppState, ui_dir: Option<PathBuf>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers(Any);

    let api = Router::new()
        .route("/health", get(handlers::health::health))
        .route("/chat", post(handlers::chat::chat))
        .route("/system/stats", get(get_system_stats))
        .route("/ingest", post(handle_pdf_ingest))
        .route("/index/progress", get(handle_progress_ws))
        .route("/chat/stream", get(handle_chat_ws))
        .route("/sessions", get(handlers::sessions::list_sessions))
        .route("/sessions/{session_id}", get(handlers::sessions::get_session))
        .layer(cors)
        .with_state(state);

    if let Some(ui_dir) = ui_dir.filter(|dir| dir.join("index.html").exists()) {
        let index = ui_dir.join("index.html");
        api.fallback_service(
            ServeDir::new(ui_dir)
                .append_index_html_on_directories(true)
                .not_found_service(ServeFile::new(index)),
        )
    } else {
        api
    }
}
