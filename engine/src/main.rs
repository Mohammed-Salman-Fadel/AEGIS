#[path = "../../tools/calendar/mod.rs"]
mod calendar_tool;
mod classifier;
mod compactor;
mod config;
mod context;
mod inference;
mod mcp;
mod mcp_client;
mod memory_store;
mod model_registry;
mod network;
mod orchestrator;
mod plan_parser;
mod process_manager;
mod prompt_builder;
mod provider_registry;
mod rag_client;
mod react_loop;
mod response_style;
mod tool_registry;
mod user_profile;
mod workflow;

use config::{AppConfig, InferenceProvider};
use inference::InferenceBackend;
use tokio::time::{Duration, sleep, timeout};

const STARTUP_INITIALIZATION_TIMEOUT: Duration = Duration::from_secs(90);
const STARTUP_WARMUP_ATTEMPTS: usize = 3;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let config = AppConfig::from_env()?;
    let bind_addr = format!("{}:{}", config.server.host, config.server.port);

    let inference: Box<dyn InferenceBackend + Send + Sync> = match &config.inference.provider {
        InferenceProvider::Ollama => Box::new(inference::backends::ollama::OllamaBackend::new(
            config.inference.base_url.clone(),
        )),
        InferenceProvider::LmStudio => Box::new(
            inference::backends::openai_compat::OpenAiCompatBackend::new(
                config.inference.base_url.clone(),
                config.inference.api_key.clone(),
            ),
        ),
        InferenceProvider::OpenAiCompatible => Box::new(
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

    let rag_client = std::sync::Arc::new(rag_client::RagClient::new(config.rag.base_url.clone()));
    let startup_rag_client = std::sync::Arc::clone(&rag_client);
    let memory_store = memory_store::MemoryStore::new().await;

    let mcp_manager = mcp::McpManager::new().await;

    let orchestrator = orchestrator::Orchestrator::new(
        inference,
        rag_client,
        memory_store,
        config.inference.provider,
        config.inference.base_url,
        config.inference.api_key,
        config.semble_path,
        config.python_path,
        mcp_manager,
    );

    let state = network::state::AppState::new(orchestrator);
    let startup_orchestrator = std::sync::Arc::clone(&state.orchestrator);
    let app = network::router::create_router(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    tracing::info!("AEGIS engine listening on http://{bind_addr}");

    // Network availability is the first startup milestone. RAG initialization
    // and model loading can be slow on a cold machine, but neither should keep
    // the Web UI or recovery/settings endpoints offline.
    tokio::spawn(async move {
        match timeout(STARTUP_INITIALIZATION_TIMEOUT, startup_rag_client.init()).await {
            Ok(Ok(())) => tracing::info!("RAG client initialized in the background"),
            Ok(Err(error)) => tracing::warn!(
                error = %error,
                "RAG initialization is unavailable; the engine and UI remain usable."
            ),
            Err(_) => tracing::warn!(
                "RAG initialization timed out in the background; it will retry when retrieval is requested."
            ),
        }
    });

    tokio::spawn(async move {
        for attempt in 1..=STARTUP_WARMUP_ATTEMPTS {
            match timeout(
                STARTUP_INITIALIZATION_TIMEOUT,
                startup_orchestrator.warm_active_model(),
            )
            .await
            {
                Ok(Ok(())) => {
                    tracing::info!("Active model warmed in the background");
                    return;
                }
                Ok(Err(error)) if attempt < STARTUP_WARMUP_ATTEMPTS => {
                    tracing::warn!(
                        attempt,
                        error = %error,
                        "Active model is not ready yet; retrying background warmup."
                    );
                }
                Err(_) if attempt < STARTUP_WARMUP_ATTEMPTS => {
                    tracing::warn!(
                        attempt,
                        "Active model warmup timed out; retrying in the background."
                    );
                }
                Ok(Err(error)) => {
                    tracing::warn!(
                        error = %error,
                        "Active model could not be warmed during startup; the UI remains available for recovery."
                    );
                    return;
                }
                Err(_) => {
                    tracing::warn!(
                        "Active model warmup timed out during startup; the UI remains available for recovery."
                    );
                    return;
                }
            }
            sleep(Duration::from_secs(2)).await;
        }
    });

    axum::serve(listener, app).await?;
    Ok(())
}
