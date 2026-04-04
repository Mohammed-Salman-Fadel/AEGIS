// Network layer — entry point for all incoming requests
//
// TODO: define IncomingRequest { session_id, message, attachments }
// TODO: define Attachment { filename, path }
// TODO: define ResponseSender (streaming channel to client)
// TODO: serve() — start HTTP server
// TODO: POST /chat handler — deserialize request, open stream, hand off to orchestrator
// TODO: POST /ingest handler — trigger RAG ingestion for a file/directory
// TODO: GET  /health handler — check engine + RAG process status

pub mod handlers;
mod router;
mod state;