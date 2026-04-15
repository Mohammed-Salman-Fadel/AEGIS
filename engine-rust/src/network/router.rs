use axum::{
    routing::{get, post},
    Router,
};

use super::{handlers, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(handlers::health::health))
        .route("/chat", post(handlers::chat::chat))
        .route("/sessions", get(handlers::sessions::list_sessions))
        .route("/sessions/{session_id}", get(handlers::sessions::get_session))
        .with_state(state)
}
