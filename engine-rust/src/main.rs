mod config;
mod network;
mod orchestrator;
mod context;
mod classifier;
mod workflow;
mod compactor;
mod prompt_builder;
mod inference;
mod plan_parser;
mod rag_client;
mod tool_registry;
mod model_registry;
mod memory_store;
mod process_manager;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let host = std::env::var("AEGIS_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = std::env::var("AEGIS_ENGINE_PORT").unwrap_or_else(|_| "8080".to_string());
    let bind_addr = format!("{host}:{port}");

    let ollama_base_url =
        std::env::var("AEGIS_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());

    let inference = Box::new(inference::backends::ollama::OllamaBackend::new(ollama_base_url));
    let rag_client = std::sync::Arc::new(rag_client::RagClient::new());
    let memory_store = memory_store::MemoryStore::new();

    let orchestrator = orchestrator::Orchestrator::new(inference, rag_client, memory_store);
    let state = network::state::AppState::new(orchestrator);
    let app = network::router::create_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("AEGIS engine listening on http://{bind_addr}");

    axum::serve(listener, app).await?;
    Ok(())
}
