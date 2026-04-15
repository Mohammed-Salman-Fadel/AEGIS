# AEGIS Python RAG Subsystem

The `rag-python` module is the document retrieval subsystem of AEGIS. Its purpose is to enable document-grounded responses by ingesting local files, preparing them for semantic retrieval, and returning relevant content when requested by the central Rust orchestration engine.

This subsystem operates as a local, persistent service and is responsible exclusively for retrieval-related tasks. It does not manage user interaction or model inference.

## Overview

AEGIS follows a modular architecture where responsibilities are clearly separated:

- Rust Engine → orchestration, routing, inference control
- Python RAG Subsystem → document indexing & retrieval
- Local LLM (e.g., Ollama) → response generation

The RAG subsystem acts as a specialized retrieval worker, enabling the system to ground responses in user-provided data rather than relying solely on model knowledge.

## Responsibilities

The Python RAG subsystem is responsible for:

- Ingesting local documents from user-specified folders
- Preprocessing and cleaning document content
- Splitting documents into semantically meaningful chunks
- Generating embeddings using a local model
- Storing embeddings and metadata in a persistent vector store
- Retrieving relevant document chunks for a given query
- Returning structured results with source metadata (citations)

## Architecture
### Role in the System

The subsystem does not:

- communicate directly with the UI
- control inference
- manage user sessions

Instead, it is invoked by the Rust engine when retrieval is required.

### System Flow

1. User sends request (CLI or Web UI)
2. Rust engine receives and analyzes request
3. If retrieval is needed → calls RAG subsystem
4. RAG subsystem:
   - processes request (index or query)
   - returns relevant chunks + metadata
5. Rust engine:
   - injects retrieved context into prompt
   - sends request to LLM
6. Response is returned to the user

## Runtime Behavior

### Initialization

The subsystem must be initialized before use:

- Load embedding model (local, e.g., SentenceTransformers)
- Initialize or restore vector store (Chroma persistent storage)
- Prepare internal state
- Mark system as ready

### Persistent Process

Once initialized:

- Runs as a long-lived local service
- Handles repeated requests without reloading models
- Maintains in-memory + on-disk state

## API Contract

The subsystem exposes a REST API for integration with the Rust engine.

### Endpoints
- `GET /health` → service status
- `POST /init` → initialize system
- `POST /index` → index documents from a folder
- `POST /query` → retrieve relevant chunks
- `POST /store` → store user memory
- `POST /shutdown` → clean shutdown

### Key Design Rules
- All responses are JSON
- Strict initialization requirement (must call `/init` first)
- Deterministic and predictable behavior
- No direct UI interaction

## Data & Metadata Model

Each stored entry (document or memory) includes:
- `text` → chunk content
- `source` → file name or `"user"`
- `page` → page number (if applicable)
- `type` → `"document"` or `"memory"`

This enables:

- citation support
- filtering
- future extensibility

## Memory Support

The subsystem also supports long-term user memory:

- Stored via `/store` endpoint
- Treated as lightweight semantic entries
- Retrieved alongside documents

Memory is:

- persistent
- queryable
- integrated into retrieval results

## Vector Store Design

The subsystem uses Chroma in persistent mode.

However, the implementation abstracts the vector store layer, allowing future replacement with alternatives such as:

- FAISS
- LanceDB

This ensures:

- modularity
- flexibility
- reduced vendor lock-in

## Reliability & Design Considerations

The system is designed with the following guarantees:

#### ✔ Concurrency Safety
- Thread-safe access to vector store and state using locks
#### ✔ Path Normalization
- All file paths handled via `pathlib` for cross-platform compatibility
#### ✔ Duplicate Protection
- Prevents re-indexing the same document
#### ✔ Resource Awareness
- Designed for low-resource machines
- No GPU required
#### ✔ Error Handling
- Clean JSON error responses
- Validation of inputs and file paths

## Integration with Rust

The subsystem is designed for seamless integration with the Rust engine:

- Communication via REST (JSON)
- Stateless request handling (except stored index)
- Fast response times
- Clear and minimal contract

## Future Extensions

Possible enhancements:

- Support for additional file formats (DOCX, HTML, etc.)
- Improved ranking and retrieval strategies
- Advanced metadata filtering
- Hybrid search (keyword + vector)
- Smarter memory management
- Streaming responses


## Summary

The Python RAG subsystem is the data grounding layer of AEGIS.

It enables the system to:

- understand user documents
- retrieve relevant knowledge
- provide context-aware responses

while remaining:

- local-first
- modular
- efficient
- and easy to integrate


> This subsystem serves as a foundational component in building a **privacy-preserving, local AI assistant platform**.
