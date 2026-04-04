// Phase definitions — the building blocks of every workflow
//
// TODO: Phase enum {
//     Plan,                          // LLM produces structured execution plan
//     Execute,                       // run steps from the plan
//     Synthesize,                    // LLM produces final answer
//     RagRetrieve { top_k: usize },  // direct RAG call, no planning step
//     Chunk,                         // split large input into processable pieces
//     Merge,                         // combine outputs from multiple chunks
//     Compact,                       // explicit compaction step in workflow
// }
