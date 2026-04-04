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
async fn main() {
    // TODO: load config
    // TODO: process_manager::spawn_rag()
    // TODO: model_registry::load()
    // TODO: tool_registry::register_defaults()
    // TODO: inference::build_backend(&config)
    // TODO: network::serve()
}
