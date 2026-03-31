# AEGIS Web UI

The `web-ui` module provides the browser-based user interface for AEGIS. Its purpose is to allow
users to interact with the system through a local web application running on `localhost`.

The Web UI is one of the main user-facing interfaces of AEGIS. It is responsible for presenting chat
interactions, displaying system status, and exposing user controls, while relying on the Rust engine
as the central backend and orchestration layer.

## Responsibilities

The Web UI is responsible for:

- providing a local browser-based chat interface
- sending user requests to the Rust engine
- displaying streamed assistant responses
- showing runtime and status information to the user
- exposing user-facing controls such as session reset and interface options
- presenting feedback related to indexing, system state, and local-only operation

## Role in the System

The Web UI acts as a local client of the Rust engine. It does not directly communicate with the
inference backend, the RAG subsystem, or MCP-based tools. Instead, it forwards user actions to the
Rust engine and displays the results returned through the orchestrated backend flow.

This separation keeps the UI focused on user interaction rather than system control logic.

## How It Is Used

A typical Web UI-based flow is:

1. The user opens the local AEGIS web application in the browser.
2. The user submits a request through the chat interface.
3. The Web UI sends the request to the Rust engine.
4. The Rust engine processes the request and determines the correct execution path.
5. If needed, the Rust engine invokes the RAG subsystem or MCP-based integrations.
6. The Rust engine sends the prepared request to Ollama.
7. Ollama generates the response.
8. The Rust engine streams the response back to the Web UI.
9. The Web UI displays the result progressively to the user.

## Intended Provided Functions

The Web UI is intended to provide the following user-facing capabilities:

- **Chat Interface**
    - allow users to submit natural language requests and view assistant responses
- **Streamed Response Display**
    - display assistant output progressively in real time
- **Status Indicators**
    - show assistant state such as loading, generating, or idle
- **Local-Only Confirmation**
    - provide a visible indication that processing remains on the local machine
- **Session Controls**
    - support actions such as resetting or starting a new conversation
- **Document Processing Feedback**
    - display indexing progress or retrieval-related feedback when relevant
- **User Convenience Features**
    - support features such as message editing, export, or interface preferences where applicable

These functions define the Web UI as the primary graphical interaction layer of AEGIS.

## Relationship with Other Components

The Web UI operates together with the following components:

- **Rust Engine**
    - main backend and orchestration layer
- **CLI**
    - alternative local interaction and control interface
- **Ollama**
    - inference backend used indirectly through the Rust engine
- **Python RAG subsystem**
    - document retrieval subsystem used indirectly through the Rust engine
- **MCP integrations**
    - local tools accessed indirectly through the Rust engine when required

## Future Possibilities

Possible future extensions include:

- richer visual feedback for subsystem activity
- more advanced settings and configuration screens
- improved usability and accessibility features
- optional voice interaction support
- richer document interaction workflows

These are future possibilities and are not all required for the initial MVP.

## Summary

The Web UI is the browser-based interaction layer of AEGIS. It allows users to communicate with the
system through a local chat interface while relying on the Rust engine to coordinate inference,
retrieval, and other backend operations.