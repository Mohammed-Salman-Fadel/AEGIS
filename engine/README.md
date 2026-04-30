# AEGIS Rust Engine

The `engine-rust` module is the central orchestration layer of AEGIS. It acts as the main backend of
the system and coordinates communication between the user interfaces, the local inference backend,
the document retrieval subsystem, and selected MCP-based local tools.

The engine is responsible for controlling the request lifecycle rather than directly implementing
every subsystem internally. Its role is to receive requests, determine the required execution path,
invoke supporting components when needed, and return responses consistently to both the CLI and web
UI.

## Responsibilities

The Rust engine is responsible for:

- receiving requests from the CLI and web UI
- managing the overall system flow
- coordinating inference through Ollama or an OpenAI-compatible local backend such as LM Studio
- invoking the RAG subsystem for document-based queries
- invoking MCP-based local tools when required
- managing session state and context
- streaming responses back to the caller
- handling fallback behavior and runtime errors
- exposing a unified backend flow for all user interfaces

## MVP Scope

For the initial MVP, the Rust engine should support:

- a local API for the CLI and web UI
- basic direct chat flow through Ollama or LM Studio
- streamed assistant responses
- minimal session handling
- explicit invocation of the RAG subsystem when requested
- basic configuration support
- basic error handling

In the MVP, advanced routing and classification can be simplified. Requests may default to direct
chat unless the user explicitly chooses a document-based or tool-based path.

## Architecture Role

The Rust engine is the **central controller** of AEGIS.

It does **not** replace:
- the inference backend
- the RAG engine
- the UI
- the CLI

Instead, it coordinates them.

## High-Level Flow

1. Receive a request from the CLI or Web UI.
2. Load the relevant session state.
3. Determine the execution path.
4. If required, invoke the RAG subsystem or MCP integration.
5. Assemble the final context for inference.
6. Send the request to the configured inference provider.
7. Stream the response back to the caller.
8. Handle errors, logging, and state updates.

## Supported Integrations

The engine is designed to work with:

- **Ollama**
    - local inference backend
- **LM Studio**
    - local OpenAI-compatible inference backend
- **Python RAG subsystem**
    - document ingestion and retrieval
- **MCP integrations**
    - optional local tools or data sources
- **CLI**
    - local control and interaction
- **Web UI**
    - browser-based chat interface

## Inference Configuration

The engine defaults to Ollama:

```bash
AEGIS_INFERENCE_PROVIDER=ollama
AEGIS_OLLAMA_URL=http://127.0.0.1:11434
AEGIS_MODEL=mistral:7b
```

LM Studio uses the shared OpenAI-compatible backend and defaults to LM Studio's local server URL:

```bash
AEGIS_INFERENCE_PROVIDER=lmstudio
AEGIS_LM_STUDIO_URL=http://127.0.0.1:1234
AEGIS_MODEL=<lm-studio-model-id>
```

Other OpenAI-compatible local servers can use:

```bash
AEGIS_INFERENCE_PROVIDER=openai-compatible
AEGIS_OPENAI_COMPAT_URL=http://127.0.0.1:1234
AEGIS_OPENAI_COMPAT_API_KEY=<optional-api-key>
AEGIS_MODEL=<model-id>
```

## Future Possibilities

The engine is intended to allow future extension as the project evolves. Possible future additions
include:

- dynamic model selection based on local hardware
- more advanced request routing or classification
- richer session memory and context handling
- multiple inference providers beyond Ollama
- more MCP-based tool integrations
- stronger observability and benchmarking support
- more advanced fallback logic and policy control

These are future possibilities and are not required for the initial MVP.

## Design Principles

The engine should follow these principles:

- keep orchestration logic centralized
- avoid placing core logic in the UI
- avoid making the model runtime the controller of the system
- treat RAG and MCP as supporting subsystems
- keep interfaces explicit and controlled
- start with a minimal working core and extend gradually

## Summary

The Rust engine is the core backend of AEGIS. Its value lies in providing a structured, local-only
orchestration layer that unifies inference, retrieval, tool access, and user interfaces into a single
controlled execution flow
