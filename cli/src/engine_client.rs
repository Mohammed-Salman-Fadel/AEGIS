//! Role: placeholder localhost HTTP boundary between the CLI and the Rust orchestrator.
//! Called by: `commands.rs` for chat, session, provider, model, and status flows.
//! Calls into: future engine endpoints such as `/chat`, `/health`, `/sessions`, `/providers`, and `/models`.
//! Owns: CLI-side request intent and placeholder response shapes for the future HTTP client.
//! Does not own: orchestration logic, session history, provider state, or model state.
//! Next TODOs: replace placeholder returns with real HTTP requests and source endpoint paths from shared engine config.

use std::env;
use std::io::{BufRead, BufReader};
use std::time::Duration;

use crate::AppResult;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct EngineClient {
    base_url: String,
    ollama_url: String,
    lm_studio_url: String,
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
pub struct CreatedSession {
    pub id: String,
    pub created_at: String,
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
        let ollama_url =
            env::var("AEGIS_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
        let lm_studio_url = env::var("AEGIS_LM_STUDIO_URL")
            .or_else(|_| env::var("AEGIS_LMSTUDIO_URL"))
            .unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());

        Self {
            base_url,
            ollama_url,
            lm_studio_url,
        }
    }

    pub fn health(&self) -> EngineHealth {
        let request_path = format!("{}/health", self.base_url);
        match health_probe_client().get(&request_path).send() {
            Ok(response) if response.status().is_success() => EngineHealth {
                base_url: self.base_url.clone(),
                request_path,
                reachable: true,
                note: "Engine /health responded successfully.".to_string(),
            },
            Ok(response) => EngineHealth {
                base_url: self.base_url.clone(),
                request_path,
                reachable: false,
                note: format!("Engine /health returned HTTP {}.", response.status()),
            },
            Err(error) => EngineHealth {
                base_url: self.base_url.clone(),
                request_path,
                reachable: false,
                note: format!("Could not reach engine: {error}"),
            },
        }
    }

    pub fn ollama_health(&self) -> EngineHealth {
        let request_path = format!("{}/api/tags", self.ollama_url.trim_end_matches('/'));
        match health_probe_client().get(&request_path).send() {
            Ok(response) if response.status().is_success() => EngineHealth {
                base_url: self.ollama_url.clone(),
                request_path,
                reachable: true,
                note: "Ollama serve responded successfully.".to_string(),
            },
            Ok(response) => EngineHealth {
                base_url: self.ollama_url.clone(),
                request_path,
                reachable: false,
                note: format!("Ollama serve returned HTTP {}.", response.status()),
            },
            Err(error) => EngineHealth {
                base_url: self.ollama_url.clone(),
                request_path,
                reachable: false,
                note: format!("Could not reach Ollama serve: {error}"),
            },
        }
    }

    pub fn chat<F>(
        &self,
        prompt: &str,
        session_id: Option<&str>,
        on_token: F,
    ) -> AppResult<ChatReply>
    where
        F: FnMut(&str) -> AppResult<()>,
    {
        let request_path = format!("{}/chat", self.base_url);
        let response = reqwest::blocking::Client::new()
            .post(&request_path)
            .json(&ChatRequestBody {
                session_id: session_id.map(str::to_string),
                message: prompt.to_string(),
            })
            .send()
            .map_err(|error| format!("Could not send chat request to engine: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Engine chat request failed with HTTP {}.",
                response.status()
            ));
        }

        self.consume_chat_stream(response, request_path, on_token)
    }

    pub fn chat_from_stdin<F>(
        &self,
        prompt: &str,
        session_id: Option<&str>,
        on_token: F,
    ) -> AppResult<ChatReply>
    where
        F: FnMut(&str) -> AppResult<()>,
    {
        // TODO: reuse the same /chat request body as `chat`, but feed the prompt text from stdin.
        self.chat(prompt, session_id, on_token)
    }

    pub fn repl_turn<F>(
        &self,
        prompt: &str,
        session_id: Option<&str>,
        on_token: F,
    ) -> AppResult<ChatReply>
    where
        F: FnMut(&str) -> AppResult<()>,
    {
        // TODO: keep the REPL thin by forwarding each turn through the same /chat API boundary.
        self.chat(prompt, session_id, on_token)
    }

    pub fn create_session(&self) -> AppResult<CreatedSession> {
        let request_path = format!("{}/sessions", self.base_url);
        let response = reqwest::blocking::Client::new()
            .post(&request_path)
            .send()
            .map_err(|error| format!("Could not create a new session in the engine: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!(
                "Engine session creation failed with HTTP {}. {}",
                status,
                body.trim()
            ));
        }

        let response = response.json::<EngineSession>().map_err(|error| {
            format!("Could not parse engine session creation response: {error}")
        })?;

        Ok(CreatedSession {
            id: response.session_id,
            created_at: response.created_at,
        })
    }

    pub fn list_sessions(&self) -> AppResult<Vec<SessionSummary>> {
        let request_path = format!("{}/sessions", self.base_url);
        let response = reqwest::blocking::get(&request_path)
            .map_err(|error| format!("Could not fetch sessions from engine: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!(
                "Engine sessions request failed with HTTP {}. {}",
                status,
                body.trim()
            ));
        }

        let response = response
            .json::<SessionsResponse>()
            .map_err(|error| format!("Could not parse engine sessions response: {error}"))?;

        Ok(response
            .sessions
            .into_iter()
            .map(|session| SessionSummary {
                id: session.session_id,
                title: session.title,
                description: format!(
                    "{} turn(s), updated {}",
                    session.turn_count, session.updated_at
                ),
            })
            .collect())
    }

    pub fn show_session(&self, session_id: &str) -> AppResult<SessionDetail> {
        let request_path = format!("{}/sessions/{session_id}", self.base_url);
        let response = reqwest::blocking::get(&request_path)
            .map_err(|error| format!("Could not fetch session from engine: {error}"))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(format!("Session `{session_id}` was not found."));
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!(
                "Engine session request failed with HTTP {}. {}",
                status,
                body.trim()
            ));
        }

        let response = response
            .json::<EngineSession>()
            .map_err(|error| format!("Could not parse engine session response: {error}"))?;

        Ok(SessionDetail {
            id: response.session_id,
            title: response.title,
            note: format!(
                "Created {}, updated {}",
                response.created_at, response.updated_at
            ),
            recent_turns: response
                .history
                .turns
                .into_iter()
                .flat_map(|turn| {
                    [
                        format!("user> {}", turn.query),
                        format!("assistant> {}", turn.response),
                    ]
                })
                .collect(),
        })
    }

    pub fn delete_session(&self, session_id: &str) -> AppResult<ActionStatus> {
        let request_path = format!("{}/sessions/{session_id}", self.base_url);
        let response = reqwest::blocking::Client::new()
            .delete(&request_path)
            .send()
            .map_err(|error| format!("Could not delete the session in the engine: {error}"))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(format!("Session `{session_id}` was not found."));
        }

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!(
                "Engine session delete failed with HTTP {}. {}",
                status,
                body.trim()
            ));
        }

        let response = response
            .json::<DeleteSessionResponse>()
            .map_err(|error| format!("Could not parse engine session delete response: {error}"))?;

        Ok(ActionStatus {
            request_path,
            target: response.session_id,
            persisted: response.persisted,
            message: response.message,
        })
    }

    pub fn list_providers(&self) -> AppResult<Vec<ProviderSummary>> {
        let request_path = format!("{}/providers", self.base_url);
        let response = reqwest::blocking::get(&request_path)
            .map_err(|error| format!("Could not fetch providers from engine: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!(
                "Engine providers request failed with HTTP {}. {}",
                status,
                body.trim()
            ));
        }

        let response = response
            .json::<ProvidersResponse>()
            .map_err(|error| format!("Could not parse engine providers response: {error}"))?;

        Ok(response
            .providers
            .into_iter()
            .map(|provider| ProviderSummary {
                name: provider.name,
                description: provider.description,
            })
            .collect())
    }

    pub fn select_provider(&self, name: &str) -> AppResult<ActionStatus> {
        let request_path = format!("{}/providers/select", self.base_url);
        let response = reqwest::blocking::Client::new()
            .post(&request_path)
            .json(&SelectProviderRequest {
                name: name.to_string(),
            })
            .send()
            .map_err(|error| format!("Could not switch provider: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!(
                "Engine provider switch request failed with HTTP {}. {}",
                status,
                body.trim()
            ));
        }

        let response = response
            .json::<SwitchProviderResponse>()
            .map_err(|error| format!("Could not parse provider switch response: {error}"))?;

        Ok(ActionStatus {
            request_path,
            target: name.to_string(),
            persisted: response.persisted,
            message: response.message,
        })
    }

    pub fn list_models(&self) -> AppResult<Vec<ModelSummary>> {
        let current_model = self.current_model().ok();
        let request_path = format!("{}/models", self.base_url);
        let response = reqwest::blocking::get(&request_path)
            .map_err(|error| format!("Could not fetch models from engine: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            if status == reqwest::StatusCode::NOT_FOUND {
                return self.list_models_fallback(current_model);
            }
            return Err(format!(
                "Engine models request failed with HTTP {}. {}",
                status,
                body.trim()
            ));
        }

        let response = response
            .json::<ModelsResponse>()
            .map_err(|error| format!("Could not parse engine models response: {error}"))?;

        Ok(response
            .models
            .into_iter()
            .map(|model| {
                let is_active = current_model
                    .as_deref()
                    .map(|current| current.eq_ignore_ascii_case(&model.name))
                    .unwrap_or(false);

                ModelSummary {
                    description: if is_active {
                        "Currently active in the engine.".to_string()
                    } else {
                        String::new()
                    },
                    name: model.name,
                    provider: response.provider.clone(),
                }
            })
            .collect())
    }

    fn list_models_fallback(&self, current_model: Option<String>) -> AppResult<Vec<ModelSummary>> {
        match self.current_provider()?.as_str() {
            "lmstudio" => self.list_lm_studio_models(current_model),
            "ollama" => self.list_ollama_models(current_model),
            _ => self.list_lm_studio_models(current_model),
        }
    }

    fn list_lm_studio_models(&self, current_model: Option<String>) -> AppResult<Vec<ModelSummary>> {
        let request_path = format!("{}/v1/models", self.lm_studio_url.trim_end_matches('/'));
        let response = reqwest::blocking::get(&request_path)
            .map_err(|error| format!("Could not fetch LM Studio models: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!("LM Studio models request failed with HTTP {}. {}", status, body.trim()));
        }

        let response = response
            .json::<LmStudioModelsResponse>()
            .map_err(|error| format!("Could not parse LM Studio models response: {error}"))?;

        Ok(response
            .data
            .into_iter()
            .map(|model| {
                let is_active = current_model
                    .as_deref()
                    .map(|current| current.eq_ignore_ascii_case(&model.id))
                    .unwrap_or(false);

                ModelSummary {
                    name: model.id,
                    provider: "lmstudio".to_string(),
                    description: if is_active {
                        "Currently active in the engine.".to_string()
                    } else {
                        String::new()
                    },
                }
            })
            .collect())
    }

    fn list_ollama_models(&self, current_model: Option<String>) -> AppResult<Vec<ModelSummary>> {
        let request_path = format!("{}/api/tags", self.ollama_url.trim_end_matches('/'));
        let response = reqwest::blocking::get(&request_path)
            .map_err(|error| format!("Could not fetch Ollama models: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!("Ollama model list request failed with HTTP {}. {}", status, body.trim()));
        }

        let response = response
            .json::<OllamaTagsResponse>()
            .map_err(|error| format!("Could not parse Ollama model list: {error}"))?;

        Ok(response
            .models
            .into_iter()
            .map(|model| {
                let is_active = current_model
                    .as_deref()
                    .map(|current| current.eq_ignore_ascii_case(&model.name))
                    .unwrap_or(false);

                ModelSummary {
                    description: if is_active {
                        "Currently active in the engine.".to_string()
                    } else {
                        String::new()
                    },
                    name: model.name,
                    provider: "ollama".to_string(),
                }
            })
            .collect())
    }

    pub fn current_provider(&self) -> AppResult<String> {
        let request_path = format!("{}/providers/current", self.base_url);
        let response = reqwest::blocking::get(&request_path)
            .map_err(|error| format!("Could not fetch the active provider from engine: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Engine current provider request failed with HTTP {}.",
                response.status()
            ));
        }

        let response = response
            .json::<CurrentProviderResponse>()
            .map_err(|error| format!("Could not parse current provider response: {error}"))?;

        Ok(response.provider)
    }

    pub fn current_model(&self) -> AppResult<String> {
        let request_path = format!("{}/models/current", self.base_url);
        let response = reqwest::blocking::get(&request_path)
            .map_err(|error| format!("Could not fetch the active model from engine: {error}"))?;

        if !response.status().is_success() {
            return Err(format!(
                "Engine current model request failed with HTTP {}.",
                response.status()
            ));
        }

        let response = response
            .json::<CurrentModelResponse>()
            .map_err(|error| format!("Could not parse current model response: {error}"))?;

        Ok(response.model)
    }

    pub fn select_model(&self, name: &str) -> AppResult<ActionStatus> {
        let request_path = format!("{}/models/select", self.base_url);
        let response = reqwest::blocking::Client::new()
            .post(&request_path)
            .json(&SwitchModelRequest {
                name: name.to_string(),
            })
            .send()
            .map_err(|error| format!("Could not switch the active model: {error}"))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_default();
            return Err(format!(
                "Engine model switch request failed with HTTP {}. {}",
                status,
                body.trim()
            ));
        }

        let response = response
            .json::<SwitchModelResponse>()
            .map_err(|error| format!("Could not parse model switch response: {error}"))?;

        Ok(ActionStatus {
            request_path,
            target: response.current,
            persisted: response.persisted,
            message: response.message,
        })
    }

    fn consume_chat_stream<F>(
        &self,
        response: reqwest::blocking::Response,
        request_path: String,
        mut on_token: F,
    ) -> AppResult<ChatReply>
    where
        F: FnMut(&str) -> AppResult<()>,
    {
        let reader = BufReader::new(response);
        let mut message = String::new();
        let mut event_lines = Vec::new();

        for line in reader.lines() {
            let line =
                line.map_err(|error| format!("Could not read engine chat stream: {error}"))?;
            if let Some(data) = line.strip_prefix("data: ") {
                event_lines.push(data.to_string());
                continue;
            }

            if line.is_empty() {
                if flush_sse_event(&mut event_lines, &mut message, &mut on_token)? {
                    break;
                }
            }
        }

        let _ = flush_sse_event(&mut event_lines, &mut message, &mut on_token)?;

        Ok(ChatReply {
            request_path,
            message,
        })
    }
}

#[derive(Serialize)]
struct ChatRequestBody {
    session_id: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct SwitchModelRequest {
    name: String,
}

#[derive(Deserialize)]
struct SessionsResponse {
    sessions: Vec<EngineSessionSummary>,
}

#[derive(Deserialize)]
struct EngineSessionSummary {
    session_id: String,
    title: String,
    turn_count: usize,
    updated_at: String,
}

#[derive(Deserialize)]
struct EngineSession {
    session_id: String,
    title: String,
    history: EngineHistory,
    created_at: String,
    updated_at: String,
}

#[derive(Deserialize)]
struct DeleteSessionResponse {
    session_id: String,
    persisted: bool,
    message: String,
}

#[derive(Deserialize)]
struct EngineHistory {
    turns: Vec<EngineTurn>,
}

#[derive(Deserialize)]
struct EngineTurn {
    query: String,
    response: String,
}

#[derive(Deserialize)]
struct CurrentModelResponse {
    model: String,
}

#[derive(Deserialize)]
struct CurrentProviderResponse {
    provider: String,
}

#[derive(Deserialize)]
struct SwitchModelResponse {
    current: String,
    persisted: bool,
    message: String,
}

#[derive(Deserialize)]
struct ProvidersResponse {
    providers: Vec<ProviderRecord>,
}

#[derive(Deserialize)]
struct ProviderRecord {
    name: String,
    description: String,
}

#[derive(Serialize)]
struct SelectProviderRequest {
    name: String,
}

#[derive(Deserialize)]
struct SwitchProviderResponse {
    current: String,
    persisted: bool,
    message: String,
}

#[derive(Deserialize)]
struct ModelsResponse {
    provider: String,
    models: Vec<ModelRecord>,
}

#[derive(Deserialize)]
struct ModelRecord {
    name: String,
}

#[derive(Deserialize)]
struct LmStudioModelsResponse {
    data: Vec<LmStudioModelRecord>,
}

#[derive(Deserialize)]
struct LmStudioModelRecord {
    id: String,
}

#[derive(Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Deserialize)]
struct OllamaModel {
    name: String,
}

fn flush_sse_event<F>(
    event_lines: &mut Vec<String>,
    message: &mut String,
    on_token: &mut F,
) -> AppResult<bool>
where
    F: FnMut(&str) -> AppResult<()>,
{
    if event_lines.is_empty() {
        return Ok(false);
    }

    let data = event_lines.join("\n");
    event_lines.clear();

    if data == "[DONE]" {
        return Ok(true);
    }

    if data.starts_with("[ERROR]") {
        return Err(data);
    }

    message.push_str(&data);
    on_token(&data)?;
    Ok(false)
}

fn health_probe_client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .expect("health probe client should build")
}
