// Orchestrator — owns the full request lifecycle
//
// TODO: handle(req: IncomingRequest, tx: ResponseSender)
//   → load or create session from memory_store
//   → load model profile from model_registry
//   → build RequestContext
//   → call classifier to get WorkflowId
//   → fetch WorkflowDef from workflow_registry
//   → run each Phase in order
//       Phase::Plan       → run_plan_phase(&mut ctx)
//       Phase::Execute    → run_execute_phase(&mut ctx)
//       Phase::Synthesize → run_synthesize_phase(&mut ctx, tx)
//       Phase::Compact    → compactor::compact(&mut ctx)
//       Phase::RagRetrieve{ top_k } → rag_client::retrieve(...)
//       Phase::Chunk      → chunk large input slots
//       Phase::Merge      → merge multiple slot outputs
//   → save turn to memory_store
//
// TODO: run_plan_phase
//   → compactor::compact
//   → prompt_builder::build_plan_prompt
//   → inference::call
//   → plan_parser::parse → ExecutionPlan
//   → store plan in ctx.slots
//
// TODO: run_execute_phase
//   → iterate over ExecutionPlan.steps
//   → compactor::compact before each step
//   → dispatch: RagRetrieve | CallTool | DirectAnswer
//   → store results in ctx.slots
//   → append to ctx.trace
//
// TODO: run_synthesize_phase
//   → compactor::compact
//   → prompt_builder::build_synthesis_prompt
//   → inference::stream → ResponseSender
