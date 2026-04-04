// Model Registry — stores capability profiles for known models
//
// TODO: ModelProfile {
//     name:           String,
//     context_window: usize,   // total tokens the model supports
//     output_reserve: usize,   // tokens reserved for generation
// }
//
// TODO: impl ModelProfile
//   → usable_context() -> usize
//       context_window - output_reserve
//
// TODO: ModelRegistry struct
//
// TODO: impl ModelRegistry
//   → load()   — populate known profiles at startup
//   → get(name: &str) -> Option<&ModelProfile>
//   → get_active() -> &ModelProfile  — returns profile for currently configured model
//
// TODO: known profiles to hardcode:
//   → mistral:7b         context: 8192
//   → llama3:8b          context: 8192
//   → llama3:70b         context: 8192
//   → gemma:2b           context: 8192
//   → phi3:mini          context: 4096
//   → deepseek-coder:7b  context: 16384
//   → claude-*           context: 200000
//
// TODO: dynamic fetch fallback
//   → if model not in registry, query backend.list_models() for context size
