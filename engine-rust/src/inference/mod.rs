pub mod backends;

// Inference — abstraction over all LLM providers
//
// TODO: InferenceBackend trait {
//     async fn call(req: InferenceRequest)   -> Result<InferenceResponse>
//     async fn stream(req: InferenceRequest, tx: ResponseSender) -> Result<()>
//     async fn list_models()                 -> Result<Vec<String>>
//     async fn health()                      -> Result<()>
// }
//
// TODO: InferenceRequest {
//     model:   String,
//     prompt:  Prompt,
//     options: InferenceOptions,
// }
//
// TODO: InferenceOptions {
//     temperature: f32,
//     max_tokens:  usize,
//     stop_tokens: Vec<String>,
// }
//
// TODO: InferenceResponse {
//     text:        String,
//     tokens_used: usize,
// }
//
// TODO: build_backend(cfg: &InferenceConfig) -> Box<dyn InferenceBackend>
//   → match cfg.backend:
//       "ollama"        → OllamaBackend
//       "openai_compat" → OpenAICompatBackend
//       "anthropic"     → AnthropicBackend
