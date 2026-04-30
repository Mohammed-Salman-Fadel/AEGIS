mod classifier;
mod compactor;
mod config;
mod context;
mod inference;
mod memory_store;
mod model_registry;
mod network;
mod orchestrator;
mod plan_parser;
mod process_manager;
mod prompt_builder;
mod rag_client;
mod tool_registry;
mod user_profile;
mod workflow;

use anyhow::Context;
use config::{AppConfig, InferenceProvider};
use inference::InferenceBackend;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = AppConfig::from_env()?;
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);

    let inference: Box<dyn InferenceBackend + Send + Sync> = match &config.inference.provider {
        InferenceProvider::Ollama => Box::new(inference::backends::ollama::OllamaBackend::new(
            config.inference.base_url.clone(),
        )),
        InferenceProvider::LmStudio | InferenceProvider::OpenAiCompatible => Box::new(
            inference::backends::openai_compat::OpenAiCompatBackend::new(
                config.inference.base_url.clone(),
                config.inference.api_key.clone(),
            ),
        ),
    };

    tracing::info!(
        provider = ?config.inference.provider,
        base_url = %config.inference.base_url,
        "configured inference backend"
    );

    let rag_client = std::sync::Arc::new(rag_client::RagClient::new());
    let memory_store = memory_store::MemoryStore::new().await;

    let orchestrator = orchestrator::Orchestrator::new(inference, rag_client, memory_store);
    orchestrator
        .warm_active_model()
        .await
        .context("AEGIS engine startup aborted because the active model could not be warmed.")?;

    let state = network::state::AppState::new(orchestrator);
    let app = network::router::create_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("AEGIS engine listening on http://{bind_addr}");

    axum::serve(listener, app).await?;
    Ok(())
}
