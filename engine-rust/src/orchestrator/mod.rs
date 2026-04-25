use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::classifier::Classifier;
use crate::compactor::Compactor;
use crate::context::RequestContext;
use crate::inference::InferenceBackend;
use crate::memory_store::{MemoryStore, Session, SessionSummary};
use crate::model_registry::ModelRegistry;
use crate::network::handlers::chat::ChatRequest;
use crate::plan_parser::{PlanParser, StepResult};
use crate::prompt_builder::PromptBuilder;
use crate::rag_client::RagClient;
use crate::tool_registry::ToolRegistry;
use crate::user_profile;
use crate::workflow::registry::WorkflowRegistry;

/// The central orchestrator — coordinates every subsystem.
/// This is the primary entry point for all incoming chat requests.
pub struct Orchestrator {
    classifier: Classifier,
    workflow_registry: WorkflowRegistry,
    compactor: Compactor,
    prompt_builder: PromptBuilder,
    inference: Box<dyn InferenceBackend + Send + Sync>,
    plan_parser: PlanParser,
    pub rag_client: Arc<RagClient>, // Public access for the network layer to ingest files
    tool_registry: ToolRegistry,
    model_registry: ModelRegistry,
    memory_store: MemoryStore,
}

impl Orchestrator {
    /// Initializes the Orchestrator with required backend services.
    pub fn new(
        inference: Box<dyn InferenceBackend + Send + Sync>,
        rag_client: Arc<RagClient>,
        memory_store: MemoryStore,
    ) -> Self {
        Self {
            classifier: Classifier::new(),
            workflow_registry: WorkflowRegistry::new(),
            compactor: Compactor::new(),
            prompt_builder: PromptBuilder::new(),
            inference,
            plan_parser: PlanParser::new(),
            rag_client,
            tool_registry: ToolRegistry::new(),
            model_registry: ModelRegistry::new(),
            memory_store,
        }
    }

    /// Handles incoming requests, manages session IDs, and streams responses to the network layer.
    pub async fn handle(&self, req: ChatRequest, tx: mpsc::Sender<String>) {
        let persisted_session_id = req
            .session_id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_string());

        let working_session_id = persisted_session_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        match self
            .handle_fallback(req, working_session_id, persisted_session_id, tx.clone())
            .await
        {
            Ok(_) => {
                let _ = tx.send("[DONE]".to_string()).await;
            }
            Err(error) => {
                let _ = tx.send(format!("[ERROR] {error}")).await;
            }
        }
    }

    /// Creates a new persisted chat session.
    pub async fn create_session(&self) -> anyhow::Result<Session> {
        self.memory_store.create_session().await
    }

    /// Returns a list of all persisted chat sessions.
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        self.memory_store.list_sessions().await
    }

    /// Retrieves a specific stored session by its unique ID.
    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<Session>> {
        self.memory_store.get_session(session_id).await
    }

    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<bool> {
        self.memory_store.delete_session(session_id).await
    }

    pub fn current_model_name(&self) -> String {
        self.model_registry.current_model_name()
    }

    pub fn set_active_model(&self, name: &str) -> String {
        self.model_registry.set_active_model(name)
    }

    /// The core logic for handling queries using a fallback-to-synthesis approach.
    async fn handle_fallback(
        &self,
        req: ChatRequest,
        working_session_id: String,
        persisted_session_id: Option<String>,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let session = if let Some(session_id) = persisted_session_id.as_deref() {
            self.memory_store
                .get_session(session_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Session `{session_id}` was not found."))?
        } else {
            Session {
                session_id: working_session_id.clone(),
                title: "Temporary chat".to_string(),
                history: crate::context::ConversationHistory::empty(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
        };

        let model = self.model_registry.get_active();
        let mut ctx = RequestContext::new(
            working_session_id.clone(),
            req.message.clone(),
            session.history.clone(),
            model,
        );

        ctx.trace_summary("classify", "fallback");

        // 1. RAG (Retrieval-Augmented Generation): Retrieve relevant chunks from uploaded documents
        let relevant_chunks = self
            .rag_client
            .retrieve(&ctx.original_query, 5)
            .await
            .unwrap_or_default();
        let context_from_docs = relevant_chunks.join("\n---\n");

        // 2. Prompt Synthesis: Perform "Context Injection" if document data is available
        let synthesis_prompt = if !context_from_docs.is_empty() {
            ctx.trace_summary("rag", "context found and injected into prompt");
            format!(
                "You are the AEGIS AI Assistant. Below are excerpts from a document provided by the user. \
                Please use this information to answer the question as accurately as possible. \
                If the document does not contain the answer, use your general knowledge to assist.\n\n\
                DOCUMENT CONTENT:\n{}\n\nUSER QUERY: {}",
                context_from_docs, ctx.original_query
            )
        } else {
            ctx.trace_summary("rag", "no relevant document context found");
            self.prompt_builder
                .build_synthesis_prompt(&ctx.history, &ctx.original_query, &[])
        };
        let synthesis_prompt = user_profile::personalize_prompt(&synthesis_prompt);

        // 3. Inference: Call the local LLM and stream tokens back to the client
        let final_answer = self
            .inference
            .stream(&synthesis_prompt, &ctx.model.name, tx)
            .await?;

        if let Some(session_id) = persisted_session_id.as_deref() {
            self.memory_store
                .append_turn(
                    session_id,
                    &req.message,
                    &final_answer,
                    &ctx.model.name,
                    &ctx.trace,
                )
                .await?;
        }

        Ok(final_answer)
    }

    /// Executes specific atomic steps planned by the reasoning engine.
    async fn execute_steps(
        &self,
        ctx: &RequestContext,
        steps: Vec<crate::plan_parser::PlanStep>,
    ) -> anyhow::Result<Vec<StepResult>> {
        let mut results = Vec::new();

        for step in steps.into_iter().take(3) {
            let output = match step.tool.as_str() {
                "think" => {
                    let prompt = self.prompt_builder.build_step_prompt(
                        &ctx.history,
                        &ctx.original_query,
                        &step.input,
                    );
                    self.inference.call(&prompt, &ctx.model.name).await?
                }
                "rag" | "search" | "document" => {
                    let chunks = self
                        .rag_client
                        .retrieve(&step.input, 5)
                        .await
                        .unwrap_or_default();
                    if chunks.is_empty() {
                        "No relevant information was found in the document.".to_string()
                    } else {
                        chunks.join("\n---\n")
                    }
                }
                unsupported => {
                    format!("Unsupported tool usage detected: `{unsupported}`.")
                }
            };

            results.push(StepResult {
                step_id: step.id,
                output,
            });
        }

        Ok(results)
    }
}
