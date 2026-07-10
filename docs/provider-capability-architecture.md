# AEGIS Provider Capability Architecture

## Goal

AEGIS should switch between Ollama, LM Studio, and future providers without guessing which operations are supported. The CLI and UI should know whether a provider can chat, stream, list models, download models, unload models, or report context windows before offering an action.

## Provider Capabilities

The engine now exposes provider descriptors with these capability flags:

- `chat`
- `streaming`
- `model_listing`
- `model_download`
- `model_unload`
- `context_window_detection`
- `requires_external_app`

This metadata is returned by `/api/providers` and consumed by the CLI.

## Provider Defaults

Ollama:

- Best default for local-first installs.
- Supports chat, streaming, model list, model download, unload, keep-alive warmup, and context-window discovery.

LM Studio:

- Good local OpenAI-compatible runtime.
- Supports chat, streaming, model list, and AEGIS-managed downloads when the LM Studio management API is available.
- Context-window discovery is not consistently available through the OpenAI-compatible API.

OpenAI-compatible:

- Future extension point for remote or custom APIs.
- Supports chat, streaming, and model listing when `/v1/models` is available.
- AEGIS does not manage model download/unload for generic providers.

## UX Rules

- Hide or disable unsupported actions instead of letting users discover them through errors.
- When an action is unsupported, say which provider supports it.
- Provider health should eventually test provider-specific endpoints:
  - Ollama: `/api/tags`
  - LM Studio: `/v1/models`
  - OpenAI-compatible: `/v1/models`

## Future Work

- Consolidate provider construction into one factory module.
- Add runtime provider health checks with actionable remediation.
- Add tests for provider capability descriptors.
- Persist provider selection when the user changes provider through CLI/UI.
