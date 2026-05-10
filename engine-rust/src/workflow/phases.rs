pub enum Phase {
    Plan,
    Execute,
    Synthesize,
    RagRetrieve { top_k: usize },
    Chunk,
    Merge,
    Compact,
}
