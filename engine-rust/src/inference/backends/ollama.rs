// Ollama backend — talks to a local Ollama instance
//
// TODO: OllamaBackend { base_url: String, client: reqwest::Client }
//
// TODO: impl InferenceBackend for OllamaBackend
//   → call()
//       POST {base_url}/api/generate
//       serialize InferenceRequest → Ollama request format
//       deserialize response → InferenceResponse
//
//   → stream()
//       POST {base_url}/api/generate  with stream: true
//       read NDJSON stream
//       forward tokens to ResponseSender
//
//   → list_models()
//       GET {base_url}/api/tags
//       return model name list
//
//   → health()
//       GET {base_url}/
//       check 200 OK
