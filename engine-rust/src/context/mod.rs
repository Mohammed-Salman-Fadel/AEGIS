// Context — central state object for the entire request lifecycle
//
// TODO: RequestContext {
//     request_id:     Uuid,
//     session_id:     String,
//     original_query: String,
//     history:        ConversationHistory,
//     model:          ModelProfile,
//     slots:          HashMap<String, SlotValue>,
//     trace:          Vec<TraceEntry>,
// }
//
// TODO: SlotValue enum {
//     Text(String),
//     Chunks(Vec<RagChunk>),
//     ToolOutput(serde_json::Value),
//     Plan(ExecutionPlan),
// }
//
// TODO: TraceEntry {
//     phase:       String,
//     tokens_used: usize,
//     summary:     Option<String>,   // compressed version if output was large
// }
//
// TODO: ConversationHistory {
//     turns: Vec<Turn>,
// }
//
// TODO: Turn {
//     query:    String,
//     response: String,
// }
//
// TODO: impl RequestContext
//   → slots helpers: get, insert, large_slots()
//   → trace helpers: push, total_tokens_used()
