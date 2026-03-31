# AEGIS Python RAG Subsystem

The `rag-python` module is the document retrieval subsystem of AEGIS. Its purpose is to enable
document-grounded responses by ingesting local files, preparing them for retrieval, and returning
relevant document content when requested by the central orchestration layer.

This subsystem is designed as a supporting local process rather than the main backend of the
application. It is responsible only for document processing and retrieval. The Rust engine remains
the central controller of the overall system flow.

## Responsibilities

The Python RAG subsystem is responsible for:

- ingesting local documents from user-specified folders
- preprocessing document content
- splitting documents into searchable chunks
- building and maintaining a local persistent index
- retrieving relevant chunks for document-based queries
- returning source references or citation metadata where available

## Role in the System

The RAG subsystem does not communicate directly with the user interface and does not control the
inference workflow. Its role is to act as a dedicated retrieval component that responds to requests
from the Rust orchestration layer.

In the current design, the Rust engine may launch the Python RAG process during startup and keep it
running in memory as a persistent local helper process. Once initialized, the RAG subsystem remains
available to handle indexing and retrieval requests without repeated startup overhead.

## How It Is Used

The RAG subsystem is used only when the Rust engine determines that document retrieval is required.

A typical usage flow is:

1. A user sends a request through the CLI or Web UI.
2. The request is received by the Rust engine.
3. If the request requires document-grounded processing, the Rust engine invokes the Python RAG
   subsystem.
4. The RAG subsystem performs the required operation, such as indexing documents or retrieving
   relevant chunks.
5. The RAG subsystem returns structured results to the Rust engine.
6. The Rust engine integrates the retrieved context into the final model request.
7. The Rust engine sends the prepared request to Ollama for response generation.

In this architecture, the Python RAG subsystem is a specialized retrieval worker, while the Rust
engine remains responsible for orchestration, inference coordination, and user-facing flow.

## Initialization and Runtime Behavior

The Python RAG subsystem is expected to support an initialization phase during startup. During this
phase, it may:

- load runtime configuration
- prepare or restore local indexes
- initialize retrieval-related resources
- confirm readiness to the Rust engine

After initialization, the process remains alive in memory and handles repeated indexing and retrieval
requests until the system shuts down.

## Intended Provided Functions

The Python RAG subsystem is intended to provide a small set of core operations that can be invoked by
the Rust engine. At a minimum, these include:

- **Initialization**
    - prepare runtime state and load required resources
- **Document Indexing**
    - ingest local files, preprocess them, and build/update the retrieval index
- **Document Retrieval**
    - accept a query and return the most relevant document chunks
- **Citation Support**
    - return source metadata such as file names and page references alongside retrieved content
- **Health Check**
    - report whether the subsystem is running and ready
- **Shutdown**
    - terminate cleanly when the main system exits

These functions are intended to form the minimal contract between the Rust engine and the RAG
subsystem.

## Supported Operations

At a minimum, the RAG subsystem is expected to support:

- initialization
- document indexing
- query-time retrieval
- health/status reporting
- clean shutdown

The exact communication method between the Rust engine and the Python RAG subsystem may vary
depending on implementation choices, but the subsystem should always operate as a controlled
dependency of the Rust engine rather than an independent application.

## Future Possibilities

Possible future extensions include:

- support for additional document formats
- improved retrieval quality and ranking
- more advanced citation support
- persistent index management improvements
- richer document preprocessing strategies

These are future possibilities and are not all required for the initial MVP.

## Summary

The Python RAG subsystem is the document-grounding component of AEGIS. It is used by the Rust
orchestration layer to process and retrieve local document content so that model responses can be
grounded in user-provided data rather than relying only on the base model’s general knowledge.

