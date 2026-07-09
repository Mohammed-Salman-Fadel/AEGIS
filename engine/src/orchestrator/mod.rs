use anyhow::Context;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::warn;
use uuid::Uuid;

use crate::classifier::Classifier;
use crate::compactor::Compactor;
use crate::config::InferenceProvider;
use crate::context::RequestContext;
use crate::inference::InferenceBackend;
use crate::memory_store::{MemoryStore, Session, SessionSummary};
use crate::model_registry::{ModelRegistry, DEFAULT_CONTEXT_WINDOW};
use crate::network::handlers::chat::ChatRequest;
use crate::plan_parser::{PlanParser, StepResult};

/// Resolve the project filesystem path from a project ID.
/// Mirrors the logic in `projects.rs::dirs_project_dir`.
pub fn resolve_project_path(project_id: &str) -> Option<PathBuf> {
    let sanitized: String = project_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let base = std::env::var("AEGIS_DATA_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                std::env::var("APPDATA")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("AEGIS")
            } else {
                std::env::var("XDG_DATA_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| {
                        std::env::var("HOME")
                            .map(|h| PathBuf::from(h).join(".local/share"))
                            .unwrap_or_else(|_| PathBuf::from("."))
                    })
                    .join("AEGIS")
            }
        });
    Some(base.join("projects").join(sanitized))
}
use crate::prompt_builder::{format_history, PromptBuilder};
use crate::provider_registry::ProviderRegistry;
use crate::rag_client::RagClient;
use crate::react_loop::ReactLoop;
use crate::mcp::McpManager;
use crate::response_style;
use crate::tool_registry::ToolRegistry;
use crate::user_profile;
use crate::workflow::registry::WorkflowRegistry;

const MAX_CODE_PROJECT_CONTEXT_CHARS: usize = 500_000;

/// The central orchestrator — coordinates every subsystem.
/// This is the primary entry point for all incoming chat requests.
pub struct Orchestrator {
    classifier: Classifier,
    workflow_registry: WorkflowRegistry,
    compactor: Compactor,
    prompt_builder: PromptBuilder,
    inference: RwLock<Box<dyn InferenceBackend + Send + Sync>>,
    plan_parser: PlanParser,
    pub rag_client: Arc<RagClient>, // Public access for the network layer to ingest files
    tool_registry: ToolRegistry,
    model_registry: ModelRegistry,
    provider_registry: ProviderRegistry,
    memory_store: MemoryStore,
    pub mcp_manager: McpManager,
}

pub struct ModelSwitchOutcome {
    pub previous_model: String,
    pub current_model: String,
    pub changed: bool,
    pub unload_warning: Option<String>,
}

pub struct ProviderSwitchOutcome {
    pub previous_provider: String,
    pub current_provider: String,
    pub changed: bool,
}

pub struct ContextUsageSnapshot {
    pub provider: String,
    pub model: String,
    pub used_tokens: usize,
    pub context_window: usize,
    pub usage_source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatMode {
    General,
    Coder,
    Academic,
}

impl From<Option<String>> for ChatMode {
    fn from(mode: Option<String>) -> Self {
        match mode.as_deref() {
            Some("coder") => ChatMode::Coder,
            Some("academic") => ChatMode::Academic,
            _ => ChatMode::General,
        }
    }
}

fn format_active_documents(attachments: &[String]) -> String {
    if attachments.is_empty() {
        return "No explicit document attachments were provided for this turn.".to_string();
    }

    attachments
        .iter()
        .map(|attachment| format!("- {attachment}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_code_project_context(context: &str) -> String {
    let trimmed = context.trim();
    if trimmed.len() <= MAX_CODE_PROJECT_CONTEXT_CHARS {
        return trimmed.to_string();
    }

    let mut truncated = trimmed
        .chars()
        .take(MAX_CODE_PROJECT_CONTEXT_CHARS)
        .collect::<String>();
    truncated
        .push_str("\n\n[AEGIS truncated the selected project context to fit the prompt budget.]");
    truncated
}

// Request classification has moved to engine/src/classifier/mod.rs.
// The orchestrator calls `self.classifier.classify()` at the start of
// handle_fallback and uses the returned WorkflowId for persona selection
// and context-source routing.

fn session_title_prompt(first_prompt: &str) -> String {
    let first_prompt: String = first_prompt.trim().chars().take(1_200).collect();

    format!(
        "Create a short title for a chat session using only the user's first message.\n\
        Rules:\n\
        - 3 to 7 words\n\
        - capture the main topic\n\
        - do not use later conversation context\n\
        - return only the title\n\
        - no quotes, markdown, explanations, or ending punctuation\n\n\
        First user message:\n{first_prompt}\n\n\
        Title:"
    )
}

fn imported_document_title_prompt(
    first_prompt: Option<&str>,
    document_names: &[String],
    document_excerpts: &[String],
) -> String {
    let first_prompt = first_prompt
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("<no first prompt yet>");
    let first_prompt: String = first_prompt.chars().take(1_200).collect();
    let documents = if document_names.is_empty() {
        "<no document names provided>".to_string()
    } else {
        document_names
            .iter()
            .map(|name| format!("- {name}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let excerpts = if document_excerpts.is_empty() {
        "<no document excerpts were retrieved; infer only from the file names and first prompt>"
            .to_string()
    } else {
        document_excerpts
            .iter()
            .map(|excerpt| excerpt.trim())
            .filter(|excerpt| !excerpt.is_empty())
            .take(4)
            .collect::<Vec<_>>()
            .join("\n---\n")
            .chars()
            .take(2_400)
            .collect()
    };

    format!(
        "Create a short title for an AEGIS chat session.\n\
        Use the user's first prompt if it exists, and the imported document context.\n\
        Rules:\n\
        - 3 to 7 words\n\
        - capture the main topic of the conversation or document\n\
        - return only the title\n\
        - no quotes, markdown, explanations, or ending punctuation\n\n\
        First user prompt:\n{first_prompt}\n\n\
        Imported documents:\n{documents}\n\n\
        Document excerpts:\n{excerpts}\n\n\
        Title:"
    )
}

fn strip_thinking_sections(raw: &str) -> String {
    let mut output = String::new();
    let mut remaining = raw;

    loop {
        let lower = remaining.to_lowercase();
        let Some(start) = lower.find("<think>") else {
            output.push_str(remaining);
            break;
        };

        output.push_str(&remaining[..start]);
        let after_start = &remaining[start + "<think>".len()..];
        let lower_after_start = after_start.to_lowercase();

        let Some(end) = lower_after_start.find("</think>") else {
            break;
        };

        remaining = &after_start[end + "</think>".len()..];
    }

    output
}

fn clean_generated_session_title(raw: &str) -> Option<String> {
    let without_thinking = strip_thinking_sections(raw);
    let mut candidate = without_thinking
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?
        .to_string();

    for prefix in [
        "session title:",
        "chat title:",
        "title:",
        "the title is:",
        "the title:",
        "here is a short title:",
        "here's a short title:",
    ] {
        if candidate.to_lowercase().starts_with(prefix) {
            candidate = candidate[prefix.len()..].trim().to_string();
        }
    }

    candidate = candidate
        .trim_matches(|ch: char| {
            ch.is_whitespace() || matches!(ch, '"' | '\'' | '`' | '*' | '#' | '-' | ':' | '.')
        })
        .to_string();
    candidate = candidate
        .trim_end_matches(|ch: char| matches!(ch, '.' | ':' | ';' | ',' | '!' | '?'))
        .to_string();

    let title = candidate
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    let title: String = title.chars().take(60).collect();

    if title.trim().is_empty() {
        None
    } else {
        Some(title.trim().to_string())
    }
}

impl Orchestrator {
    pub fn new(
        inference: Box<dyn InferenceBackend + Send + Sync>,
        rag_client: Arc<RagClient>,
        memory_store: MemoryStore,
        provider: InferenceProvider,
        _active_base_url: String,
        _api_key: Option<String>,
        semble_path: String,
        python_path: String,
        mcp_manager: McpManager,
    ) -> Self {
        let provider_registry = ProviderRegistry::new();
        provider_registry.set_active_provider(provider);
        Self {
            classifier: Classifier::new(),
            workflow_registry: WorkflowRegistry::new(),
            compactor: Compactor::new(),
            prompt_builder: PromptBuilder::new(),
            inference: RwLock::new(inference),
            plan_parser: PlanParser::new(),
            rag_client,
            tool_registry: ToolRegistry::new(&python_path, &semble_path),
            model_registry: ModelRegistry::new(),
            provider_registry,
            memory_store,
            mcp_manager,
        }
    }

    /// Handles incoming requests, manages session IDs, and streams responses to the network layer.
    pub async fn handle(&self, req: ChatRequest, tx: mpsc::Sender<String>) {
        let persisted_session_id = req
            .session_id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .map(|value| value.trim().to_string());

        let working_session_id = persisted_session_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        match self
            .handle_fallback(req, working_session_id, persisted_session_id, tx.clone())
            .await
        {
            Ok(_) => {
                let _ = tx.send("[DONE]".to_string()).await;
            }
            Err(error) => {
                let _ = tx.send(format!("[ERROR] {error}")).await;
            }
        }
    }

    /// Creates a new persisted chat session.
    pub async fn create_session(&self, title: Option<String>) -> anyhow::Result<Session> {
        self.memory_store.create_session(title).await
    }

    /// Returns a list of all persisted chat sessions.
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        self.memory_store.list_sessions().await
    }

    /// Retrieves a specific stored session by its unique ID.
    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<Session>> {
        self.memory_store.get_session(session_id).await
    }

    pub async fn rename_session(&self, session_id: &str, title: &str) -> anyhow::Result<Session> {
        self.memory_store.rename_session(session_id, title).await
    }

    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<bool> {
        // First delete the documents from the RAG subsystem
        if let Err(e) = self.rag_client.delete_session(session_id).await {
            warn!(
                "Failed to delete RAG documents for session {}: {}",
                session_id, e
            );
            // We continue anyway so the session file is still removed
        }

        self.memory_store.delete_session(session_id).await
    }

    pub fn current_model_name(&self) -> String {
        self.model_registry.current_model_name()
    }

    pub fn current_provider_name(&self) -> String {
        self.provider_registry.current_provider_name()
    }

    pub async fn context_usage(
        &self,
        session_id: Option<&str>,
    ) -> anyhow::Result<ContextUsageSnapshot> {
        let model = self.model_registry.current_model_name();
        let provider = self.current_provider_name();

        // Use the cached context window if we already discovered it
        // (e.g. during warm_active_model or a prior poll).  Never
        // re-query the backend on every poll — the context window
        // does not change at runtime, and re-querying can cause the
        // display to flicker if a transient backend error falls back
        // to the default value.
        let context_window = {
            let cached = self.model_registry.current_context_window(&model);
            if cached != DEFAULT_CONTEXT_WINDOW {
                cached
            } else {
                // First time seeing this model — discover the real window.
                match self.inference.read().await.context_window(&model).await {
                    Ok(Some(real_window)) => {
                        self.model_registry.set_context_window(&model, real_window);
                        real_window
                    }
                    _ => cached,
                }
            }
        };

        let used_tokens = match session_id {
            Some(session_id) if !session_id.trim().is_empty() => self
                .memory_store
                .latest_prompt_token_usage(session_id)
                .await?
                .unwrap_or(0),
            _ => 0,
        };

        Ok(ContextUsageSnapshot {
            provider,
            model,
            used_tokens,
            context_window,
            usage_source: if used_tokens > 0 {
                "ollama-prompt-eval-count".to_string()
            } else {
                "no-session-usage-yet".to_string()
            },
        })
    }

    pub async fn list_available_models(&self) -> anyhow::Result<(String, Vec<String>)> {
        let provider = self.current_provider_name();
        let models = self
            .inference
            .read()
            .await
            .list_models()
            .await
            .unwrap_or_default();
        Ok((provider, models))
    }

    pub async fn call_inference(&self, prompt: &str, model: &str) -> anyhow::Result<String> {
        self.inference.read().await.call(prompt, model).await
    }

    /// Query the inference backend for the model's real context window.
    /// Returns `None` if the backend doesn't report a value (e.g. OpenAI-compatible).
    pub async fn call_inference_context_window(&self, model: &str) -> Option<usize> {
        self.inference
            .read()
            .await
            .context_window(model)
            .await
            .ok()
            .flatten()
    }

    /// Persist a real context window for a specific model into the model registry.
    /// Subsequent lookups for this model return the correct value immediately.
    pub fn set_model_context_window(&self, model: &str, window: usize) {
        self.model_registry.set_context_window(model, window);
    }

    async fn generate_session_title(
        &self,
        first_prompt: &str,
        model: &str,
    ) -> anyhow::Result<String> {
        let prompt = session_title_prompt(first_prompt);
        let raw_title = self.inference.read().await.call(&prompt, model).await?;

        clean_generated_session_title(&raw_title).ok_or_else(|| {
            anyhow::anyhow!("The model returned an empty or unusable session title.")
        })
    }

    async fn generate_imported_document_session_title(
        &self,
        first_prompt: Option<&str>,
        document_names: &[String],
        document_excerpts: &[String],
        model: &str,
    ) -> anyhow::Result<String> {
        let prompt =
            imported_document_title_prompt(first_prompt, document_names, document_excerpts);
        let raw_title = self.inference.read().await.call(&prompt, model).await?;

        clean_generated_session_title(&raw_title).ok_or_else(|| {
            anyhow::anyhow!("The model returned an empty or unusable imported-document title.")
        })
    }

    async fn title_first_turn_session(&self, session_id: &str, first_prompt: &str, model: &str) {
        match self.generate_session_title(first_prompt, model).await {
            Ok(title) => {
                if let Err(error) = self.memory_store.rename_session(session_id, &title).await {
                    warn!(
                        session_id,
                        "Could not save generated session title `{title}`: {error}"
                    );
                }
            }
            Err(error) => {
                warn!(
                    session_id,
                    "Could not generate a title for the first session prompt: {error}"
                );
            }
        }
    }

    pub async fn title_session_from_import(
        &self,
        session_id: &str,
        document_names: &[String],
    ) -> anyhow::Result<Session> {
        let session = self
            .memory_store
            .get_session(session_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Session `{session_id}` was not found."))?;

        let first_prompt = session
            .history
            .turns
            .first()
            .map(|turn| turn.query.as_str());
        let query = match first_prompt {
            Some(prompt) if !prompt.trim().is_empty() => {
                format!("{prompt}\n{}", document_names.join("\n"))
            }
            _ => format!("main topic summary title {}", document_names.join(" ")),
        };
        let document_excerpts = match self.rag_client.retrieve(&query, 4, 0.0, session_id).await {
            Ok(outcome) => outcome.chunks.into_iter().map(|c| c.text).collect(),
            Err(error) => {
                warn!(
                    session_id,
                    "Could not retrieve document excerpts for imported-document title: {error}"
                );
                Vec::new()
            }
        };
        let model = self.model_registry.get_active();
        let title = self
            .generate_imported_document_session_title(
                first_prompt,
                document_names,
                &document_excerpts,
                &model.name,
            )
            .await?;

        self.memory_store.rename_session(session_id, &title).await
    }

    pub fn list_providers(&self) -> Vec<(String, String, bool)> {
        let current = self.current_provider_name();
        vec![
            (
                "ollama".to_string(),
                "Local Ollama provider".to_string(),
                current == "ollama",
            ),
            (
                "lmstudio".to_string(),
                "LM Studio OpenAI-compatible provider".to_string(),
                current == "lmstudio",
            ),
        ]
    }

    pub async fn switch_provider(&self, name: &str) -> anyhow::Result<ProviderSwitchOutcome> {
        let provider = match name.trim().to_lowercase().as_str() {
            "ollama" => InferenceProvider::Ollama,
            "lmstudio" | "lm-studio" | "lm_studio" => InferenceProvider::LmStudio,
            "openai-compatible" | "openai_compatible" | "openai-compatible-api" => {
                InferenceProvider::OpenAiCompatible
            }
            _ => anyhow::bail!(
                "unsupported inference provider `{name}`; expected ollama, lmstudio, or openai-compatible"
            ),
        };

        let current = self.provider_registry.current_provider();
        if current == provider {
            return Ok(ProviderSwitchOutcome {
                previous_provider: current.as_str().to_string(),
                current_provider: current.as_str().to_string(),
                changed: false,
            });
        }

        let _ = self.provider_registry.set_active_provider(provider.clone());

        let new_backend: Box<dyn InferenceBackend + Send + Sync> = match &provider {
            InferenceProvider::Ollama => {
                let url = std::env::var("AEGIS_OLLAMA_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
                Box::new(crate::inference::backends::ollama::OllamaBackend::new(url))
            }
            InferenceProvider::LmStudio | InferenceProvider::OpenAiCompatible => {
                let url = std::env::var("AEGIS_LM_STUDIO_URL")
                    .or_else(|_| std::env::var("AEGIS_LMSTUDIO_URL"))
                    .or_else(|_| std::env::var("AEGIS_OPENAI_COMPAT_URL"))
                    .unwrap_or_else(|_| "http://127.0.0.1:1234".to_string());
                let api_key = std::env::var("AEGIS_OPENAI_COMPAT_API_KEY").ok();
                Box::new(
                    crate::inference::backends::openai_compat::OpenAiCompatBackend::new(
                        url, api_key,
                    ),
                )
            }
        };

        let mut backend = self.inference.write().await;
        *backend = new_backend;

        // Refresh the context window for the new provider's active model
        let active_model = self.model_registry.current_model_name();
        if let Ok(window) = backend.context_window(&active_model).await {
            if let Some(real_window) = window {
                self.model_registry.set_context_window(&active_model, real_window);
            }
        }
        drop(backend);

        Ok(ProviderSwitchOutcome {
            previous_provider: current.as_str().to_string(),
            current_provider: provider.as_str().to_string(),
            changed: true,
        })
    }

    pub fn set_active_model(&self, name: &str) -> String {
        self.model_registry.set_active_model(name)
    }

    pub async fn warm_active_model(&self) -> anyhow::Result<()> {
        let model_name = self.model_registry.current_model_name();
        self.inference
            .read()
            .await
            .warm_model(&model_name)
            .await
            .with_context(|| format!("Could not warm the active model `{model_name}`."))?;

        // Eagerly discover the real context window so the compactor
        // and token meter are accurate from the very first turn,
        // instead of relying on the fallback default until the
        // frontend polls context_usage().
        if let Ok(Some(window)) = self
            .inference
            .read()
            .await
            .context_window(&model_name)
            .await
        {
            self.model_registry
                .set_context_window(&model_name, window);
            tracing::info!(
                "Discovered context window for `{model_name}`: {} tokens",
                window
            );
        } else {
            tracing::info!(
                "Using default context window for `{model_name}`: {} tokens (backend did not report one)",
                self.model_registry.current_context_window(&model_name),
            );
        }

        Ok(())
    }

    pub async fn switch_active_model(&self, name: &str) -> anyhow::Result<ModelSwitchOutcome> {
        let next_model = name.trim();
        if next_model.is_empty() {
            anyhow::bail!("The requested model name cannot be empty.");
        }

        let current_model = self.model_registry.current_model_name();
        if current_model.eq_ignore_ascii_case(next_model) {
            return Ok(ModelSwitchOutcome {
                previous_model: current_model.clone(),
                current_model,
                changed: false,
                unload_warning: None,
            });
        }

        self.inference
            .read()
            .await
            .warm_model(next_model)
            .await
            .with_context(|| format!("Could not warm the requested model `{next_model}`."))?;

        let previous_model = self.model_registry.set_active_model(next_model);
        // Refresh the cached context window for the newly activated model.
        // Check the cache first — avoid a backend round-trip if we already know this model.
        if self.model_registry.current_context_window(next_model) == DEFAULT_CONTEXT_WINDOW {
            if let Ok(window) = self.inference.read().await.context_window(next_model).await {
                if let Some(real_window) = window {
                    self.model_registry.set_context_window(next_model, real_window);
                }
            }
        }

        let unload_warning = match self
            .inference
            .read()
            .await
            .unload_model(&previous_model)
            .await
        {
            Ok(()) => None,
            Err(error) => Some(format!(
                "Switched successfully, but could not unload `{previous_model}`: {error}"
            )),
        };

        Ok(ModelSwitchOutcome {
            previous_model,
            current_model: next_model.to_string(),
            changed: true,
            unload_warning,
        })
    }

    /// The core logic for handling queries using a fallback-to-synthesis approach.
    async fn handle_fallback(
        &self,
        mut req: ChatRequest,
        working_session_id: String,
        persisted_session_id: Option<String>,
        tx: mpsc::Sender<String>,
    ) -> anyhow::Result<String> {
        // If code_project_path is not provided but code_project_id is,
        // resolve the project directory from the ID.
        if req.code_project_path.is_none() {
            if let Some(ref project_id) = req.code_project_id {
                let resolved = resolve_project_path(project_id);
                if let Some(p) = resolved {
                    req.code_project_path = Some(p.to_string_lossy().to_string());
                }
            }
        }
        let mut session = if let Some(session_id) = persisted_session_id.as_deref() {
            self.memory_store
                .get_session(session_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Session `{session_id}` was not found."))?
        } else {
            Session {
                session_id: working_session_id.clone(),
                title: "Temporary chat".to_string(),
                history: crate::context::ConversationHistory::empty(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            }
        };

        if let Some(turn_index) = req.edit_from_turn_index {
            if turn_index > session.history.turns.len() {
                anyhow::bail!(
                    "Cannot edit turn {turn_index}; session `{}` only has {} turns.",
                    session.session_id,
                    session.history.turns.len()
                );
            }

            session.history.turns.truncate(turn_index);
        }

        let should_title_first_turn_session = persisted_session_id.is_some()
            && req.edit_from_turn_index.is_none()
            && session.history.turns.is_empty();

        let model = self.model_registry.get_active();
        let mut ctx = RequestContext::new(
            working_session_id.clone(),
            req.message.clone(),
            session.history.clone(),
            model,
        );

        // Compact conversation history to fit within the model's context window.
        // Drops oldest turns first so the prompt never overshoots the budget.
        let context_window = ctx.model.context_window;
        self.compactor.compact(&mut ctx, context_window);

        // Classify the request into a workflow so the orchestrator can
        // select the right system persona and decide which context sources
        // to query.
        let classification = self.classifier.classify(
            &req.message,
            !req.attachments.is_empty(),
            req.mode.as_deref().unwrap_or("general"),
        );
        ctx.trace_summary("classify", &format!("{:?}", classification));

        if req.attachments.is_empty()
            && crate::classifier::message_refers_to_document(&req.message)
        {
            let final_answer = "No document is attached to this conversation. Please import a document into this conversation first, then ask me to summarize it.".to_string();
            let _ = tx.send(final_answer.clone()).await;

            if let Some(session_id) = persisted_session_id.as_deref() {
                self.memory_store
                    .append_turn_with_edit(
                        session_id,
                        &req.message,
                        &final_answer,
                        &ctx.model.name,
                        &ctx.trace,
                        req.edit_from_turn_index,
                        req.edit_from_turn_index.is_some(),
                        None,
                        None,
                    )
                    .await?;

                if should_title_first_turn_session {
                    self.title_first_turn_session(session_id, &req.message, &ctx.model.name)
                        .await;
                }
            }

            return Ok(final_answer);
        }

        // ── ReAct loop for all workflows ──────────────────────────────
        // Every query goes through the agentic loop. The model sees a list
        // of available tools (search, read, calculate, rag, etc.) and can
        // choose to call them iteratively, or produce a final answer directly
        // if no tools are needed.  This makes the system truly agentic for
        // all chat modes, not just code workflows.
        // The loop is bounded by MAX_ROUNDS so it always terminates.
        let react_project_path = if matches!(
            classification,
            crate::workflow::WorkflowId::CodeExplain
                | crate::workflow::WorkflowId::CodeGenerate
                | crate::workflow::WorkflowId::CodeDebug
        ) {
            req.code_project_path.as_deref()
        } else {
            None
        };

        let final_answer = ReactLoop::execute(
            &self.inference,
            &self.tool_registry,
            &self.rag_client,
            &req.message,
            &working_session_id,
            &ctx.model.name,
            react_project_path,
            tx.clone(),
        )
        .await?;

        // Send the final answer as a single chunk.
        let _ = tx.send(final_answer.clone()).await;

        if let Some(session_id) = persisted_session_id.as_deref() {
            self.memory_store
                .append_turn_with_edit(
                    session_id,
                    &req.message,
                    &final_answer,
                    &ctx.model.name,
                    &ctx.trace,
                    req.edit_from_turn_index,
                    req.edit_from_turn_index.is_some(),
                    None,
                    None,
                )
                .await?;

            if should_title_first_turn_session {
                self.title_first_turn_session(
                    session_id,
                    &req.message,
                    &ctx.model.name,
                )
                .await;
            }
        }

        return Ok(final_answer);

        let rag_enabled = req.rag_enabled.unwrap_or(true);
        let rag_top_k = req.rag_top_k.unwrap_or(5);
        let rag_threshold = req.rag_similarity_threshold.unwrap_or(0.0);

        // 1. RAG is strictly session-scoped. If this turn has no current-session
        // attachments, or RAG is disabled, do not query the global RAG store at all.
        let (mut relevant_chunks, rag_metrics) = if req.attachments.is_empty() || !rag_enabled {
            ctx.trace_summary("rag", "no current-session document attachments or RAG disabled");
            (Vec::new(), None)
        } else {
            match self
                .rag_client
                .retrieve(&ctx.original_query, rag_top_k, rag_threshold, &working_session_id)
                .await
            {
                Ok(outcome) => (outcome.chunks, Some(outcome.metrics)),
                Err(error) => {
                    anyhow::bail!(
                        "Could not retrieve context from the current session's imported documents: {error}"
                    );
                }
            }
        };

        if let Some(metrics) = &rag_metrics {
            if let Ok(json) = serde_json::to_string(metrics) {
                let _ = tx.send(format!("[RAG_METRICS] {json}")).await;
            }
        }

        let mut unique_sources = Vec::new();
        let mut context_parts = Vec::new();

        for chunk in &relevant_chunks {
            let filename = std::path::Path::new(&chunk.source)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&chunk.source)
                .to_string();

            let citation = if let Some(page) = chunk.page {
                format!("{} (Page {})", filename, page)
            } else {
                filename.clone()
            };

            if !unique_sources.contains(&citation) {
                unique_sources.push(citation.clone());
            }

            let meta = if let Some(page) = chunk.page {
                format!("[Source: {}, Page: {}]", filename, page)
            } else {
                format!("[Source: {}]", filename)
            };

            context_parts.push(format!("{}\n\"\"\"\n{}\n\"\"\"", meta, chunk.text));
        }

        if !relevant_chunks.is_empty() {
            if let Ok(json) = serde_json::to_string(&relevant_chunks) {
                let _ = tx.send(format!("[RAG_SOURCES] {json}")).await;
            }
        }

        let context_from_docs = context_parts.join("\n\n---\n\n");

        let mode = ChatMode::from(req.mode.clone());

        // 1.5 Code Search: If the query is code-related, use Semble MCP.
        // The classifier determines code intent; the frontend's Coder mode
        // also forces code search regardless of classification.
        let context_from_code = if mode == ChatMode::Coder
            || matches!(
                classification,
                crate::workflow::WorkflowId::CodeExplain
                    | crate::workflow::WorkflowId::CodeGenerate
                    | crate::workflow::WorkflowId::CodeDebug
            ) {
            tracing::info!(
                "Query classified as code-related or in Coder mode: invoking Semble MCP tool"
            );
            ctx.trace_summary("code_search", "invoking Semble");
            match self
                .tool_registry
                .execute_code_search(&req.message, req.code_project_path.as_deref())
                .await
            {
                Ok(context) => {
                    tracing::info!(
                        "Code search successful, retrieved context length: {}",
                        context.len()
                    );
                    context
                }
                Err(e) => {
                    tracing::warn!("Code search tool execution failed: {}", e);
                    String::new()
                }
            }
        } else {
            String::new()
        };

        let context_from_project = req
            .code_project_context
            .as_deref()
            .map(truncate_code_project_context)
            .filter(|context| !context.is_empty())
            .unwrap_or_default();

        // Query RAG for project files if a project is active  
        let mut project_rag_chunks = Vec::new();
        if let Some(ref project_id) = req.code_project_id {
            let project_session = format!("__project__{project_id}");
            match self.rag_client.retrieve(&ctx.original_query, 8, 0.0, &project_session).await {
                Ok(outcome) => {
                    project_rag_chunks = outcome.chunks;
                    if let Ok(json) = serde_json::to_string(&outcome.metrics) {
                        let _ = tx.send(format!("[RAG_METRICS] {json}")).await;
                    }
                }
                Err(e) => tracing::warn!("Project RAG retrieval failed: {e}"),
            }
        }

        // Build project RAG citations — add to both context_parts and relevant_chunks
        for chunk in &project_rag_chunks {
            let filename = std::path::Path::new(&chunk.source)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&chunk.source)
                .to_string();
            let meta = format!("[Project: {}]", filename);
            context_parts.push(format!("{}\n\"\"\"\n{}\n\"\"\"", meta, chunk.text));
            if !unique_sources.contains(&filename) {
                unique_sources.push(filename);
            }
        }
        // Add project chunks to relevant_chunks so [RAG_SOURCES] sends them to frontend
        relevant_chunks.extend(project_rag_chunks);

        if !context_from_project.is_empty() {
            ctx.trace_summary(
                "project_context",
                "injecting selected local code project snapshot",
            );
        }

        // 1.6 Zotero Search: If in Academic mode or the query mentions
        // research terms, search the Zotero library.
        let context_from_zotero = if mode == ChatMode::Academic
            || req.message.to_lowercase().contains("zotero")
            || req.message.to_lowercase().contains("citation")
            || req.message.to_lowercase().contains("cite")
            || req.message.to_lowercase().contains("paper")
            || req.message.to_lowercase().contains("article")
            || req.message.to_lowercase().contains("journal")
            || req.message.to_lowercase().contains("research")
            || req.message.to_lowercase().contains("bibliography")
            || req.message.to_lowercase().contains("reference")
        {
            tracing::info!(
                "Query detected as research-related or in Academic mode: invoking Zotero MCP tool"
            );
            ctx.trace_summary("zotero_search", "invoking Zotero");
            match self.tool_registry.execute("zotero", &req.message).await {
                Ok(context) => {
                    tracing::info!(
                        "Zotero search successful, retrieved context length: {}",
                        context.len()
                    );
                    context
                }
                Err(e) => {
                    tracing::warn!("Zotero search tool execution failed: {}", e);
                    String::new()
                }
            }
        } else {
            String::new()
        };

        // 2. Prompt Synthesis: Perform "Context Injection" if data is available
        let synthesis_prompt = if !context_from_docs.is_empty()
            || !context_from_code.is_empty()
            || !context_from_project.is_empty()
            || !context_from_zotero.is_empty()
        {
            ctx.trace_summary("synthesis", "context found and injected into prompt");
            let active_documents = format_active_documents(&req.attachments);
            let system_persona = match classification {
                crate::workflow::WorkflowId::CodeExplain
                | crate::workflow::WorkflowId::CodeGenerate
                | crate::workflow::WorkflowId::CodeDebug => {
                    "You are the AEGIS AI Coder. You specialize in analyzing local codebases, explaining logic, and providing high-quality implementation suggestions."
                }
                crate::workflow::WorkflowId::DocumentQA
                | crate::workflow::WorkflowId::Summarize => {
                    "You are the AEGIS AI Document Analyst. You specialize in analyzing imported documents, answering questions based on their content, and providing clear, well-structured summaries."
                }
                crate::workflow::WorkflowId::Writing => {
                    "You are the AEGIS AI Writing Assistant. You specialize in creative and professional writing — essays, stories, emails, articles, and editing."
                }
                _ => match mode {
                    ChatMode::Coder => {
                        "You are the AEGIS AI Coder. You specialize in analyzing local codebases, explaining logic, and providing high-quality implementation suggestions."
                    }
                    ChatMode::Academic => {
                        "You are the AEGIS AI Researcher. You specialize in analyzing scientific papers, providing precise citations from the Zotero library, and maintaining a formal, evidence-based tone."
                    }
                    ChatMode::General => {
                        "You are the AEGIS AI Assistant. You provide balanced and helpful assistance across various tasks."
                    }
                },
            };

            let mut prompt = format!(
                "{}\nBelow is the retrieved context to help you answer the user's question.\n\nLOCAL RUNTIME CONTEXT:\n{}\n\n",
                system_persona,
                self.prompt_builder.runtime_context()
            );

            if !context_from_docs.is_empty() {
                prompt.push_str(&format!(
                    "ACTIVE IMPORTED DOCUMENTS:\n{}\n\nDOCUMENT CONTENT:\n{}\n\n",
                    active_documents, context_from_docs
                ));
            }

            if !context_from_code.is_empty() {
                prompt.push_str(&format!(
                    "CODEBASE EXCERPTS (from Semble):\n{}\n\n",
                    context_from_code
                ));
            }

            if !context_from_project.is_empty() {
                let project_name = req
                    .code_project_name
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                    .unwrap_or("selected local project");
                prompt.push_str(&format!(
                    "SELECTED LOCAL CODE PROJECT: {project_name}\n\
                    You have read-only context of this project. When the user asks for code changes, provide a unified diff.\n\
                    Always use the correct file extension in diff headers (e.g. +++ b/src/main.rs not +++ b/src/main).\n\
                    After every code change response, include a clear summary section listing:\n\
                    - Files created (with full content)\n\
                    - Files modified (with the exact changes made)\n\
                    - Files deleted (if any)\n\
                    Do NOT claim files were modified unless the user applied your patch.\n\n\
                    PROJECT SNAPSHOT:\n{}\n\n",
                    context_from_project
                ));
            }

            if !context_from_zotero.is_empty() {
                prompt.push_str(&format!(
                    "ZOTERO LIBRARY EXCERPTS:\n{}\n\n",
                    context_from_zotero
                ));
            }

            // Inject conversation history so the model has multi-turn context
            let history_text = format_history(&ctx.history);
            if history_text != "<empty>" {
                prompt.push_str(&format!(
                    "CONVERSATION HISTORY:\n{}\n\n",
                    history_text
                ));
            }

            prompt.push_str(&format!("USER QUERY: {}", ctx.original_query));
            prompt
        } else {
            if req.attachments.is_empty() {
                ctx.trace_summary("rag", "no relevant document context found");
                self.prompt_builder
                    .build_synthesis_prompt(&ctx.history, &ctx.original_query, &[])
            } else {
                ctx.trace_summary(
                    "rag",
                    "active documents present but no matching chunks found",
                );
                format!(
                    "{}\n\nActive imported documents:\n{}\n\nNote: retrieval found no matching chunks for this turn, so answer carefully and say when the imported documents do not contain enough context.",
                    self.prompt_builder.build_synthesis_prompt(
                        &ctx.history,
                        &ctx.original_query,
                        &[]
                    ),
                    format_active_documents(&req.attachments)
                )
            }
        };
        let synthesis_prompt = user_profile::personalize_prompt(&synthesis_prompt);
        let synthesis_prompt =
            response_style::apply_response_style(&synthesis_prompt, req.response_style.as_deref());

        // 3. Inference: Call the local LLM and stream tokens back to the client
        let final_answer = self
            .inference
            .read()
            .await
            .stream_with_usage(&synthesis_prompt, &ctx.model.name, tx)
            .await?;

        if let Some(session_id) = persisted_session_id.as_deref() {
            self.memory_store
                .append_turn_with_edit(
                    session_id,
                    &req.message,
                    &final_answer.text,
                    &ctx.model.name,
                    &ctx.trace,
                    req.edit_from_turn_index,
                    req.edit_from_turn_index.is_some(),
                    final_answer.usage.prompt_tokens,
                    final_answer.usage.completion_tokens,
                )
                .await?;

            if should_title_first_turn_session {
                self.title_first_turn_session(session_id, &req.message, &ctx.model.name)
                    .await;
            }
        }

        Ok(final_answer.text)
    }

    /// Executes specific atomic steps planned by the reasoning engine.
    async fn execute_steps(
        &self,
        ctx: &RequestContext,
        steps: Vec<crate::plan_parser::PlanStep>,
    ) -> anyhow::Result<Vec<StepResult>> {
        let mut results = Vec::new();

        for step in steps.into_iter().take(3) {
            let output = match step.tool.as_str() {
                "think" => {
                    let prompt = self.prompt_builder.build_step_prompt(
                        &ctx.history,
                        &ctx.original_query,
                        &step.input,
                    );
                    self.inference
                        .read()
                        .await
                        .call(&prompt, &ctx.model.name)
                        .await?
                }
                "rag" | "search" | "document" => {
                    let outcome = self
                        .rag_client
                        .retrieve(&step.input, 5, 0.0, &ctx.session_id)
                        .await;

                    match outcome {
                        Ok(o) if !o.chunks.is_empty() => o.chunks.into_iter().map(|c| c.text).collect::<Vec<_>>().join("\n---\n"),
                        _ => "No relevant information was found in the document.".to_string(),
                    }
                }
                unsupported => {
                    format!("Unsupported tool usage detected: `{unsupported}`.")
                }
            };

            results.push(StepResult {
                step_id: step.id,
                output,
            });
        }

        Ok(results)
    }
}
