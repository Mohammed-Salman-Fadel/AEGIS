// RAG Client — HTTP client to the Python RAG subsystem
//
// TODO: RagClient { base_url: String, client: reqwest::Client }
//
// TODO: RagChunk {
//     text:   String,
//     source: String,   // filename + page
//     score:  f32,      // relevance score
//     doc_id: String,
// }
//
// TODO: impl RagClient
//
//   → retrieve(query: &str, top_k: usize) -> Result<Vec<RagChunk>>
//       POST {base_url}/retrieve
//       body: { query, top_k }
//       returns sorted chunks (highest score first)
//
//   → ingest(path: &str, doc_id: &str) -> Result<()>
//       POST {base_url}/ingest
//       body: { path, doc_id }
//       waits for ready signal
//
//   → health() -> Result<()>
//       GET {base_url}/health
//
// TODO: retry logic — if RAG process not yet ready, retry with backoff
