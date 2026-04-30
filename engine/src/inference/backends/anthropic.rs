// Anthropic backend — cloud fallback via Anthropic API
//
// TODO: AnthropicBackend { api_key: String, client: reqwest::Client }
//
// TODO: impl InferenceBackend for AnthropicBackend
//   → call()
//       POST https://api.anthropic.com/v1/messages
//       set headers: x-api-key, anthropic-version
//       serialize prompt → Anthropic messages format
//       deserialize content[0].text → InferenceResponse
//
//   → stream()
//       POST /v1/messages  with stream: true
//       read SSE stream (content_block_delta events)
//       forward delta tokens to ResponseSender
//
//   → list_models()
//       return hardcoded known model list
//
//   → health()
//       lightweight models list call
