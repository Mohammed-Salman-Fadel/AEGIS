// Compactor — fits context into model memory before every LLM call
// RULE: this runs before every single inference call. No exceptions.
//
// TODO: compact(ctx: &mut RequestContext)
//   → measure current token usage:
//       tokens(history) + tokens(slots) + tokens(trace)
//   → compare against ctx.model.usable_context()
//   → if over budget:
//       1. compress oldest history turns first
//       2. summarize large slots (replace slot value with summary)
//       3. drop lowest-score RAG chunks if still over
//
// TODO: estimate_tokens(text: &str) -> usize
//   → fast approximation: chars / 4
//   → can be swapped for a proper tokenizer later
//
// TODO: summarize_slot(value: &SlotValue) -> String
//   → call inference with a minimal compression prompt:
//       "Compress the following under {max_tokens} tokens.
//        Preserve key facts. No commentary.\n{content}"
//
// TODO: CompactionResult {
//     tokens_before: usize,
//     tokens_after:  usize,
//     slots_summarized: Vec<String>,
//     turns_dropped: usize,
// }
