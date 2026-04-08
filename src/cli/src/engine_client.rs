//! Role: placeholder localhost HTTP boundary between the CLI and the Rust orchestrator.
//! Called by: `commands.rs` for chat, session, provider, model, and status flows.
//! Calls into: future engine endpoints such as `/chat`, `/health`, `/sessions`, `/providers`, and `/models`.
//! Owns: CLI-side request intent and placeholder response shapes for the future HTTP client.
//! Does not own: orchestration logic, session history, provider state, or model state.
//! Next TODOs: replace placeholder returns with real HTTP requests and source endpoint paths from shared engine config.

use std::env;

use crate::AppResult;

#[derive(Debug, Clone)]
pub struct EngineClient {
    base_url: String,
}

#[derive(Debug, Clone)]
pub struct EngineHealth {
    pub base_url: String,
    pub request_path: String,
    pub reachable: bool,
    pub note: String,
}

#[derive(Debug, Clone)]
pub struct ChatReply {
    pub request_path: String,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct ActionStatus {
    pub request_path: String,
    pub target: String,
    pub persisted: bool,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct SessionSummary {
    pub id: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct SessionDetail {
    pub id: String,
    pub title: String,
    pub note: String,
    pub recent_turns: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderSummary {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ModelSummary {
    pub name: String,
    pub provider: String,
    pub description: String,
}

impl EngineClient {
    pub fn from_env() -> Self {
        let base_url =
            env::var("AEGIS_ENGINE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        Self { base_url }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn health(&self) -> EngineHealth {
        // TODO: replace this placeholder shape with a real HTTP GET so both the CLI and
        // future tests can verify the engine is reachable before running chat commands.
        EngineHealth {
            base_url: self.base_url.clone(),
            request_path: format!("{}/health", self.base_url),
            reachable: false,
            note: "TODO: implement a real GET /health request against the engine network layer."
                .to_string(),
        }
    }

    pub fn chat(&self, prompt: &str, session_id: Option<&str>) -> AppResult<ChatReply> {
        // TODO: keep `/chat` as the shared runtime seam instead of adding special CLI-only
        // orchestration branches here.
        Ok(ChatReply {
            request_path: format!("{}/chat", self.base_url),
            message: format!(
                "TODO: send chat payload to the orchestrator /chat endpoint over localhost HTTP.\nSession: {}\nPrompt: {}",
                session_id.unwrap_or("<engine should create or choose a session>"),
                prompt
            ),
        })
    }

    pub fn chat_from_stdin(&self, prompt: &str, session_id: Option<&str>) -> AppResult<ChatReply> {
        // TODO: reuse the same /chat request body as `chat`, but feed the prompt text from stdin.
        self.chat(prompt, session_id)
    }

    pub fn repl_turn(&self, prompt: &str, session_id: Option<&str>) -> AppResult<ChatReply> {
        // TODO: keep the REPL thin by forwarding each turn through the same /chat API boundary.
        self.chat(prompt, session_id)
    }

    pub fn create_session(&self) -> AppResult<ActionStatus> {
        // TODO: POST `/sessions` so the engine remains the source of truth for session lifecycle.
        Ok(ActionStatus {
            request_path: format!("{}/sessions", self.base_url),
            target: "<new-session-id>".to_string(),
            persisted: false,
            message: "TODO: POST /sessions to let the engine create and own session state."
                .to_string(),
        })
    }

    pub fn list_sessions(&self) -> AppResult<Vec<SessionSummary>> {
        // Placeholder records exist only so the scaffold menus and output paths are easy to test.
        Ok(vec![
            SessionSummary {
                id: "todo-session-001".to_string(),
                title: "Placeholder chat session".to_string(),
                description: "TODO: replace this list with engine-owned session summaries."
                    .to_string(),
            },
            SessionSummary {
                id: "todo-session-002".to_string(),
                title: "Second placeholder session".to_string(),
                description: "Used only so interactive menu scaffolding has something to render."
                    .to_string(),
            },
        ])
    }

    pub fn show_session(&self, session_id: &str) -> AppResult<SessionDetail> {
        Ok(SessionDetail {
            id: session_id.to_string(),
            title: "Placeholder session detail".to_string(),
            note: "TODO: GET /sessions/<id> from the engine and render real conversation history."
                .to_string(),
            recent_turns: vec![
                "user> TODO session request".to_string(),
                "assistant> TODO session response".to_string(),
            ],
        })
    }

    pub fn use_session(&self, session_id: &str) -> AppResult<ActionStatus> {
        Ok(ActionStatus {
            request_path: format!("{}/sessions/{session_id}/use", self.base_url),
            target: session_id.to_string(),
            persisted: false,
            message:
                "TODO: tell the engine which session should become active for future chat flows."
                    .to_string(),
        })
    }

    pub fn reset_session(&self, session_id: &str) -> AppResult<ActionStatus> {
        Ok(ActionStatus {
            request_path: format!("{}/sessions/{session_id}/reset", self.base_url),
            target: session_id.to_string(),
            persisted: false,
            message:
                "TODO: ask the engine to clear session history without making the CLI the source of truth."
                    .to_string(),
        })
    }

    pub fn list_providers(&self) -> AppResult<Vec<ProviderSummary>> {
        // TODO: source providers from `/providers` so selection and discovery stay consistent
        // across every AEGIS client surface.
        Ok(vec![
            ProviderSummary {
                name: "ollama".to_string(),
                description: "Local default provider placeholder.".to_string(),
            },
            ProviderSummary {
                name: "openai-compat".to_string(),
                description: "Placeholder for engines exposing an OpenAI-compatible local API."
                    .to_string(),
            },
        ])
    }

    pub fn select_provider(&self, name: &str) -> AppResult<ActionStatus> {
        Ok(ActionStatus {
            request_path: format!("{}/providers/{name}/select", self.base_url),
            target: name.to_string(),
            persisted: false,
            message:
                "TODO: persist provider selection through the engine, not inside CLI-only state."
                    .to_string(),
        })
    }

    pub fn list_models(&self) -> AppResult<Vec<ModelSummary>> {
        // TODO: source models from `/models` so menus and commands render backend-owned choices.
        Ok(vec![
            ModelSummary {
                name: "mistral:7b".to_string(),
                provider: "ollama".to_string(),
                description: "can add random predefined description".to_string(),
            },
            ModelSummary {
                name: "llama3.1".to_string(),
                provider: "ollama".to_string(),
                description: "".to_string(),
            },
        ])
    }

    pub fn select_model(&self, name: &str) -> AppResult<ActionStatus> {
        Ok(ActionStatus {
            request_path: format!("{}/models/{name}/select", self.base_url),
            target: name.to_string(),
            persisted: false,
            message: "TODO: ask the engine to make this model active in shared config.".to_string(),
        })
    }
}
