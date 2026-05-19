## System Flow

1. The user sends a request through the CLI or Web UI.
2. The Rust engine receives the request.
3. The Rust engine determines the correct execution path.
4. If the request is a direct chat request, it is sent to Ollama.
5. If the request is document-related, the Rust engine invokes the Python RAG subsystem.
6. If the request requires a local tool, the Rust engine uses the appropriate MCP integration.
7. The Rust engine assembles the final context and sends it to Ollama.
8. Ollama generates the response.
9. The Rust engine streams the response back to the user interface.
