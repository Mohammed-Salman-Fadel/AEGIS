// Classifier — maps an incoming request to a WorkflowId
//
// TODO: classify(ctx: &RequestContext) -> WorkflowId
//   → heuristics first (fast, no model call):
//       contains "summarize" / "tldr"        → WorkflowId::Summarize
//       contains "explain" + code block      → WorkflowId::CodeExplain
//       contains "write" / "generate" + code → WorkflowId::CodeGenerate
//       contains "debug" / "error" / "fix"   → WorkflowId::CodeDebug
//       attachments present                  → WorkflowId::DocumentQA
//       contains "write" / "draft" / "email" → WorkflowId::Writing
//   → fallback: WorkflowId::Default
//
// TODO: (post-MVP) small classification model call for ambiguous cases
//   → call inference with a lightweight classification prompt
//   → parse label from response
