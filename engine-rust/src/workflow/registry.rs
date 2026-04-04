// Workflow Registry — stores all hardcoded workflow definitions
//
// TODO: WorkflowRegistry struct
//
// TODO: impl WorkflowRegistry
//   → new() — register all workflows at startup
//   → get(id: WorkflowId) -> &WorkflowDef
//
// TODO: register all workflows:
//
//   Default:
//     [Plan, Execute, Synthesize]
//
//   DocumentQA:
//     [RagRetrieve { top_k: 5 }, Compact, Synthesize]
//
//   Summarize:
//     [Chunk, Plan, Execute, Merge, Synthesize]
//
//   CodeExplain:
//     [Synthesize]   ← single focused LLM call
//
//   CodeGenerate:
//     [Plan, Execute, Synthesize]
//
//   CodeDebug:
//     [Synthesize]   ← diagnose + fix in one pass for MVP
//
//   Writing:
//     [Plan, Execute, Synthesize]   ← outline → draft → refine
