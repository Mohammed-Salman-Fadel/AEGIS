// Tool Registry — registers available tools and dispatches calls
//
// TODO: ToolDef {
//     name:        String,
//     description: String,   // shown to LLM in plan prompt
//     input_schema: serde_json::Value,  // JSON schema for args
// }
//
// TODO: ToolRegistry struct
//
// TODO: impl ToolRegistry
//   → register(tool: ToolDef, handler: ToolHandler)
//   → register_defaults()  — register all built-in tools at startup
//   → execute(name: &str, args: serde_json::Value) -> Result<serde_json::Value>
//   → list() -> &[ToolDef]  — used by prompt_builder to inject tool schemas
//
// TODO: built-in tools to register:
//   → read_file(path)          — read a local file
//   → list_directory(path)     — list files in a directory
//   → run_shell(command)       — run a shell command (sandboxed)
//   → web_search(query)        — search the web (if enabled)
//   → calculator(expression)   — evaluate a math expression
//
// TODO: ToolHandler type alias
//   → async fn(args: serde_json::Value) -> Result<serde_json::Value>
