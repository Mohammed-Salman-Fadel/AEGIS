// OpenAI-compatible backend — covers LM Studio, llama.cpp server, vLLM, Jan, etc.
//
// TODO: OpenAICompatBackend { base_url: String, api_key: Option<String>, client: reqwest::Client }
//
// TODO: impl InferenceBackend for OpenAICompatBackend
//   → call()
//       POST {base_url}/v1/chat/completions
//       serialize prompt → OpenAI messages format [{role, content}]
//       deserialize choices[0].message.content → InferenceResponse
//
//   → stream()
//       POST {base_url}/v1/chat/completions  with stream: true
//       read SSE stream
//       forward delta tokens to ResponseSender
//
//   → list_models()
//       GET {base_url}/v1/models
//
//   → health()
//       GET {base_url}/v1/models
//       check 200 OK
