// Process Manager â€” owns the Python RAG subprocess lifecycle
//
// TODO: RagProcess { child: tokio::process::Child, base_url: String }
//
// TODO: spawn_rag(config: &RagConfig) -> Result<RagProcess>
//   â†’ spawn Python process: python python-services/main.py --port {port}
//   â†’ wait for ready signal (poll GET /health until 200 OK)
//   â†’ return RagProcess handle
//
// TODO: monitor(process: &mut RagProcess)
//   â†’ watch for unexpected exit
//   â†’ on crash: log error, attempt restart (up to N times)
//   â†’ on restart failure: escalate error, mark RAG as unavailable
//
// TODO: shutdown(process: RagProcess) -> Result<()>
//   â†’ send SIGTERM to child process
//   â†’ wait for graceful exit (timeout)
//   â†’ SIGKILL if still alive after timeout
//
// TODO: health_check(process: &RagProcess) -> bool
//   â†’ poll GET {base_url}/health
//   â†’ used by network /health endpoint
