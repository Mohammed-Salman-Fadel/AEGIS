use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::classifier::Classifier;
use crate::workflow::registry::WorkflowRegistry;
use crate::compactor::Compactor;
use crate::prompt_builder::PromptBuilder;
use crate::inference::InferenceBackend;
use crate::plan_parser::{ParsedPlan, PlanParser, StepResult};
use crate::rag_client::RagClient;
use crate::tool_registry::ToolRegistry;
use crate::model_registry::ModelRegistry;
use crate::memory_store::{MemoryStore, Session, SessionSummary};
use crate::context::RequestContext;
use crate::network::handlers::chat::ChatRequest;


/// The central orchestrator — coordinates every subsystem.
/// This is the only module the network layer talks to.
pub struct Orchestrator {
    classifier:        Classifier,
    workflow_registry: WorkflowRegistry,
    compactor:         Compactor,
    prompt_builder:    PromptBuilder,
    inference:         Box<dyn InferenceBackend + Send + Sync>,
    plan_parser:       PlanParser,
    rag_client:        Arc<RagClient>,
    tool_registry:     ToolRegistry,
    model_registry:    ModelRegistry,
    memory_store:      MemoryStore,
}

impl Orchestrator {
    pub fn new(
        inference:    Box<dyn InferenceBackend + Send + Sync>,
        rag_client:   Arc<RagClient>,
        memory_store: MemoryStore,
    ) -> Self {
        Self {
            classifier:        Classifier::new(),
            workflow_registry: WorkflowRegistry::new(),
            compactor:         Compactor::new(),
            prompt_builder:    PromptBuilder::new(),
            inference,
            plan_parser:       PlanParser::new(),
            rag_client,
            tool_registry:     ToolRegistry::new(),
            model_registry:    ModelRegistry::new(),
            memory_store,
        }
    }

    /// The only public method — called by the network layer.
    /// Runs the full request lifecycle and streams tokens into tx.
    pub async fn handle(&self, req: ChatRequest, tx: mpsc::Sender<String>) {
        let session_id = req
            .session_id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        match self.handle_fallback(req, session_id, tx.clone()).await {
            Ok(_) => {
                let _ = tx.send("[DONE]".to_string()).await;
            }
            Err(error) => {
                let _ = tx.send(format!("[ERROR] {error}")).await;
            }
        }
    }

    pub fn list_sessions(&self) -> Vec<SessionSummary> {
        self.memory_store.list_sessions()
    }

    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        self.memory_store.get_session(session_id)
    }

    async fn handle_fallback(
        &self,
        req: ChatRequest,
        session_id: String,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        let session = self.memory_store.load_or_create(&session_id);
        let model = self.model_registry.get_active();
        let mut ctx = RequestContext::new(
            session_id.clone(),
            req.message.clone(),
            session.history.clone(),
            model,
        );

        let workflow_id = self.classifier.classify(&ctx);
        let workflow = self.workflow_registry.get(workflow_id);
        ctx.trace_summary("classify", "fallback");
        ctx.trace_summary("workflow", format!("{} phase(s)", workflow.phases.len()));
        if !req.attachments.is_empty() {
            ctx.trace_summary(
                "attachments",
                "attachments are accepted by the MVP API but not executed yet",
            );
        }

        self.compactor.compact(&mut ctx);

        let planning_prompt = self
            .prompt_builder
            .build_planning_prompt(&ctx.history, &ctx.original_query);
        let planner_output = self
            .inference
            .call(&planning_prompt, &ctx.model.name)
            .await?;
        ctx.trace_summary("plan", "planner response received");

        let final_answer = match self.plan_parser.parse(&planner_output) {
            ParsedPlan::Final { answer } => {
                ctx.trace_summary("plan_parse", "model returned final");
                tx.send(answer.clone()).await.ok();
                answer
            }
            ParsedPlan::Steps { steps } => {
                ctx.trace_summary("plan_parse", format!("{} step(s)", steps.len()));
                let step_results = self.execute_steps(&ctx, steps).await?;
                ctx.trace_summary("execute", format!("{} step result(s)", step_results.len()));

                let synthesis_prompt = self.prompt_builder.build_synthesis_prompt(
                    &ctx.history,
                    &ctx.original_query,
                    &step_results,
                );

                self.inference
                    .stream(&synthesis_prompt, &ctx.model.name, tx)
                    .await?
            }
        };

        self.memory_store.append_turn(
            &session_id,
            req.message,
            final_answer.clone(),
        );

        Ok(final_answer)
    }

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
                unsupported => {
                    format!("Unsupported MVP tool `{unsupported}`. Only `think` is enabled.")
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
