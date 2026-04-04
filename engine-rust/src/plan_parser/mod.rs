// Plan Parser — parses the structured execution plan returned by the LLM
//
// TODO: ExecutionPlan {
//     steps: Vec<ExecutionStep>,
// }
//
// TODO: ExecutionStep {
//     action: ActionType,
//     args:   serde_json::Value,
//     reason: String,
// }
//
// TODO: ActionType enum {
//     RagRetrieve,       // args: { query: string, top_k: int }
//     CallTool(String),  // args: tool-specific
//     DirectAnswer,      // no tool needed, go straight to Synthesize
// }
//
// TODO: parse(raw: &str) -> Result<ExecutionPlan>
//   → deserialize JSON from LLM response
//   → validate each step has a known action type
//   → return error if format is invalid (triggers retry in orchestrator)
//
// TODO: handle malformed output gracefully
//   → if JSON parse fails → return ExecutionPlan with single DirectAnswer step
//   → log the parse failure for debugging
