use std::sync::Arc;
use tokio::sync::mpsc;

use crate::classifier::Classifier;
use crate::workflow::registry::WorkflowRegistry;
use crate::workflow::phases::Phase;
use crate::compactor::Compactor;
use crate::prompt_builder::PromptBuilder;
use crate::inference::InferenceBackend;
use crate::plan_parser::PlanParser;
use crate::rag_client::RagClient;
use crate::tool_registry::ToolRegistry;
use crate::model_registry::ModelRegistry;
use crate::memory_store::MemoryStore;
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
        // 1. load or create session
        let session = self.memory_store.load_or_create(&req.session_id);

        // 2. load model profile
        let model = self.model_registry.get_active();

        // 3. build the central state object for this request
        let mut ctx = RequestContext::new(req.session_id.clone(), req.message.clone(), session.history, model);

        // 4. classify → which workflow to run
        let workflow_id = self.classifier.classify(&ctx);

        // 5. fetch the workflow phases
        let workflow = self.workflow_registry.get(workflow_id);

        // 6. run each phase in order
        for phase in &workflow.phases {
            match phase {
                Phase::Plan       => self.run_plan_phase(&mut ctx).await,
                Phase::Execute    => self.run_execute_phase(&mut ctx).await,
                Phase::Synthesize => self.run_synthesize_phase(&mut ctx, &tx).await,
                Phase::Compact    => self.compactor.compact(&mut ctx),
                _                 => {} // other phases handled later
            }
        }

        // 7. save the turn to memory
        self.memory_store.append_turn(&req.session_id, &ctx);
    }

    // ---------------------------------------------------------------
    // Private phase runners
    // ---------------------------------------------------------------

    async fn run_plan_phase(&self, ctx: &mut RequestContext) {
        // compact before touching the LLM — always
        self.compactor.compact(ctx);

        // TODO: build plan prompt
        // TODO: call inference
        // TODO: parse plan → ctx.slots
    }

    async fn run_execute_phase(&self, ctx: &mut RequestContext) {
        // TODO: read plan from ctx.slots
        // TODO: for each step:
        //   compact
        //   match action → rag_client / tool_registry / direct
        //   store result in ctx.slots
    }

    async fn run_synthesize_phase(&self, ctx: &mut RequestContext, tx: &mpsc::Sender<String>) {
        // compact before touching the LLM — always
        self.compactor.compact(ctx);

        // TODO: build synthesis prompt
        // TODO: inference.stream(prompt, tx) → tokens into channel → SSE → client
    }
}
