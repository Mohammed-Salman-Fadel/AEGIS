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
use serde_json::{Value, json};
use sysinfo::System;
use tokio::sync::mpsc;
use tower_http::cors::CorsLayer;

// SYSTEM RESOURCE MONITORING
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

async fn handle_pdf_ingest(State(state): State<AppState>, mut multipart: Multipart) -> StatusCode {
    let ingest_dir = std::env::current_dir().unwrap_or_default().join("data").join("ingest");
    let _ = tokio::fs::create_dir_all(&ingest_dir).await;

    while let Ok(Some(field)) = multipart.next_field().await {
        let file_name = field.file_name().unwrap_or("upload.txt").to_string();
        if let Ok(data) = field.bytes().await {
            let file_path = ingest_dir.join(&file_name);
            if tokio::fs::write(&file_path, data).await.is_ok() {
                let path_str = file_path.to_string_lossy().to_string();
                let _ = state.orchestrator.rag_client.ingest(path_str).await;
            }
        }
    }
    StatusCode::OK
}

pub fn create_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin("http://localhost:5173".parse::<HeaderValue>().unwrap())
        .allow_methods([Method::GET, Method::POST, Method::DELETE])
        .allow_headers(tower_http::cors::Any);

    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/chat", post(handlers::chat::chat))
        .route("/models/current", get(handlers::models::current_model))
        .route("/models/select", post(handlers::models::select_model))
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
            get(handlers::sessions::get_session).delete(handlers::sessions::delete_session),
        )
        .layer(cors)
        .with_state(state)
}
