// Prompt Builder — assembles the correct prompt for each phase
//
// TODO: Prompt {
//     system:  String,
//     history: Vec<Turn>,
//     context: Option<String>,   // RAG chunks, tool results, etc.
//     user:    String,
// }
//
// TODO: build_plan_prompt(ctx: &RequestContext, tools: &[ToolDef]) -> Prompt
//   → system: role + available tool schemas + expected JSON return format
//   → history: ctx.history.recent_turns()
//   → user: ctx.original_query
//
// TODO: build_synthesis_prompt(ctx: &RequestContext) -> Prompt
//   → system: role + citation instructions
//   → context: format rag_chunks with [source, score] headers
//              format tool results
//   → history: ctx.history.recent_turns()
//   → user: ctx.original_query
//
// TODO: build_compaction_prompt(content: &str, max_tokens: usize) -> Prompt
//   → minimal prompt for slot summarization
//
// TODO: format_rag_chunks(chunks: &[RagChunk]) -> String
//   → sort by score descending
//   → format each: "[{source}]\n{text}\n"
//
// TODO: format_tool_results(results: &[ToolOutput]) -> String
//
// TODO: render_tool_schemas(tools: &[ToolDef]) -> String
//   → used in plan prompt system section
