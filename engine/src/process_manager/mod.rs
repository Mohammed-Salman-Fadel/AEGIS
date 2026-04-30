// Process Manager — owns the Python RAG subprocess lifecycle
//
// TODO: RagProcess { child: tokio::process::Child, base_url: String }
//
// TODO: spawn_rag(config: &RagConfig) -> Result<RagProcess>
//   → spawn Python process: python rag-python/main.py --port {port}
//   → wait for ready signal (poll GET /health until 200 OK)
//   → return RagProcess handle
//
// TODO: monitor(process: &mut RagProcess)
//   → watch for unexpected exit
//   → on crash: log error, attempt restart (up to N times)
//   → on restart failure: escalate error, mark RAG as unavailable
//
// TODO: shutdown(process: RagProcess) -> Result<()>
//   → send SIGTERM to child process
//   → wait for graceful exit (timeout)
//   → SIGKILL if still alive after timeout
//
// TODO: health_check(process: &RagProcess) -> bool
//   → poll GET {base_url}/health
//   → used by network /health endpoint
