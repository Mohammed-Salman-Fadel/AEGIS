use axum::{
    routing::post,
    Router,
};

use super::{handlers, state::AppState};

pub fn create_router(state: AppState) -> Router {
    Router::new()
        .route("/chat", post(handlers::chat::chat))
        .with_state(state)
}