# AEGIS

AEGIS is a local-only AI assistant platform designed to run entirely on the user’s machine without
relying on cloud-based inference or external online services. The project combines a Rust-based
orchestration engine, a web interface, a command-line interface, a Python-based RAG subsystem, and a
local inference backend such as Ollama to provide private and modular AI interaction.

## Project Overview

AEGIS was initially framed as a local AI chat application with document querying and MCP-based tool
access. The project is now being developed as a more complete software system centered around a
**central orchestration layer**. This orchestration layer manages the communication between user
interfaces, the local inference backend, the document retrieval subsystem, and selected local tool
integrations.

The system is designed around the following principles:

- **Local-only execution**
- **Privacy-preserving interaction**
- **Modular subsystem boundaries**
- **Clear orchestration of inference, retrieval, and tools**
- **Support for both CLI and web-based usage**

## High-Level Architecture

The main components of AEGIS are:

- **Web UI**
    - Browser-based interface for interactive chat and user-facing controls
- **CLI**
    - Local control interface for installation, startup, diagnostics, and direct interaction
- **Rust Engine**
    - Central orchestration layer and main backend of the system
- **Python RAG Subsystem**
    - Document ingestion, chunking, indexing, retrieval, and citation support
- **Ollama**
    - Local inference backend used for model execution
- **MCP Integrations**
    - Optional local tool/data integrations used when required

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

## Repository Structure

  ```text
  aegis/
  ├── engine-rust/
  ├── rag-python/
  ├── web-ui/
  ├── cli/
  ├── installer/
  ├── docs/
  ├── scripts/
  └── data/

  ### Directory Roles

  - engine-rust/
      - Central orchestration engine
      - API layer
      - Session handling
      - Ollama integration
      - RAG/MCP client communication
  - rag-python/
      - RAG worker process
      - Document indexing and retrieval
      - Citation support
  - web-ui/
      - Frontend application
  - cli/
      - Command-line interface
  - installer/
      - Setup and environment bootstrap logic
  - docs/
      - Architecture notes, diagrams, and project documentation
  - scripts/
      - Development and benchmark helper scripts
  - data/
      - Local runtime data, indexes, and samples

  ## Current Project Direction

  The project is intentionally being developed as a structured local AI platform rather than a simple
  frontend wrapper around a local model. The main engineering contribution is the orchestration layer,
  which ensures that:

  - all interfaces rely on the same backend logic
  - inference is controlled centrally
  - document retrieval is integrated cleanly
  - tool usage remains bounded and modular
  - the system remains understandable and extensible

  ## MVP Scope

  The minimal viable version of AEGIS focuses on:

  - basic local chat through Ollama
  - a working Rust orchestration engine
  - web interface integration
  - CLI support
  - document retrieval through the RAG subsystem
  - local-only execution
  - response streaming
  - basic session handling

  ## Status

  AEGIS is currently under active development as a graduation project. The architecture and backlog are
  being refined to ensure a realistic, modular, and defensible implementation within the project
  timeline.

  ## Notes

  This project prioritizes:

  - correctness over hype
  - architecture over superficial feature count
  - local execution over cloud dependency
  - practical engineering over toy demonstrations


  If you want, I can also give you:
  - a **shorter GitHub-style README**
  - or a **more polished one with setup placeholders and contribution sections**.


› this is the currents structure // jasserlabiadh@Jassers-MacBook-Air AEGIS % ls
  README.md       cli             engine-rust     installer       rag-python      scripts         web-
  ui
  jasserlabiadh@Jassers-MacBook-Air AEGIS %


• That structure is fine. Use this adjusted README so it matches the repo exactly.

  # AEGIS

  AEGIS is a local-only AI assistant platform designed to run entirely on the user’s machine without
  depending on cloud-based inference or external online services. The project combines a Rust-based
  orchestration engine, a command-line interface, a web interface, a Python-based RAG subsystem, and a
  local inference backend such as Ollama to provide private and modular AI interaction.

  ## Project Overview

  AEGIS was initially framed as a local AI chat application with document querying and MCP-based tool
  access. It is now being developed as a more complete software system centered around a **central
  orchestration layer**.

  This orchestration layer is the core backend of the system. It coordinates communication between the
  user interfaces, the local inference backend, the document retrieval subsystem, and selected local
  tool integrations. The goal is to build a structured local-first AI platform rather than a thin
  frontend wrapper around an LLM.

  ## Core Principles

  - **Local-only execution**
  - **Privacy-preserving design**
  - **Modular subsystem boundaries**
  - **Centralized orchestration**
  - **Support for both CLI and web usage**

  ## High-Level Architecture

  The main components of AEGIS are:

  - **Rust Engine**
    - central orchestration layer and backend
  - **CLI**
    - local control and terminal interaction interface
  - **Web UI**
    - browser-based chat and user interaction layer
  - **Python RAG Subsystem**
    - document ingestion, indexing, retrieval, and citation support
  - **Ollama**
    - local inference backend for running the language model
  - **MCP Integrations**
    - optional local tool/data integrations used when required

  ## System Flow

  1. A user sends a request through the CLI or Web UI.
  2. The Rust engine receives the request.
  3. The Rust engine determines the correct execution path.
  4. If the request is a direct chat request, it is sent to Ollama.
  5. If the request is document-related, the Rust engine invokes the Python RAG subsystem.
  6. If the request requires a local tool, the Rust engine uses the appropriate MCP integration.
  7. The Rust engine assembles the final context and sends it to Ollama.
  8. Ollama generates the response.
  9. The Rust engine streams the response back to the user interface.

  ## Repository Structure

  ```text
  AEGIS/
  ├── README.md
  ├── cli/
  ├── engine-rust/
  ├── installer/
  ├── rag-python/
  ├── scripts/
  └── web-ui/

  ## Directory Roles

  ### engine-rust/

  Contains the central orchestration engine.
  This is the main backend of AEGIS and is responsible for:

  - request handling
  - session flow
  - Ollama integration
  - RAG invocation
  - MCP integration points
  - response streaming

  ### cli/

  Contains the command-line interface.
  This part is responsible for:

  - starting the system
  - installation/setup commands
  - diagnostics and validation
  - configuration commands
  - optional direct terminal interaction

  ### installer/

  Contains setup and bootstrap logic.
  This part is responsible for:

  - preparing the local environment
  - validating dependencies
  - obtaining the correct local setup
  - supporting first-time system initialization

  ### rag-python/

  Contains the RAG subsystem.
  This part is responsible for:

  - document ingestion
  - preprocessing and chunking
  - indexing
  - retrieval
  - citation metadata
  - communication with the Rust engine

  ### web-ui/

  Contains the browser-based user interface.
  This part is responsible for:

  - chat interaction
  - streamed output display
  - status indicators
  - user-facing controls and settings

  ### scripts/

  Contains helper scripts for:

  - development
  - setup
  - benchmarking
  - local automation

  ## MVP Scope

  The minimal viable version of AEGIS focuses on:

  - local chat through Ollama
  - a working Rust orchestration engine
  - web interface integration
  - CLI support
  - document retrieval through the RAG subsystem
  - local-only execution
  - response streaming
  - basic session handling

  ## Current Status

  AEGIS is under active development as a graduation project. The system architecture, backlog, and
  module ownership are being refined to keep the implementation realistic, modular, and defensible
  within the project timeline.

  ## Engineering Direction

  The project is intentionally being developed as a structured local AI system rather than a simple
  local chatbot wrapper. The main engineering contribution is the orchestration layer, which ensures
  that inference, retrieval, interfaces, and local tool integrations are coordinated through a clear
  and controlled backend flow.