//! Role: file-backed session persistence layer for stored conversations.
//! Called by: `main.rs` during engine startup and `orchestrator/mod.rs` for session lifecycle and turn persistence.
//! Calls into: the local filesystem under the shared AEGIS data directory.
//! Owns: creating, loading, listing, deleting, and appending stored session history.
//! Does not own: inference execution, HTTP routing, or CLI rendering.
//! Next TODOs: add lightweight file locking, session export/import helpers, and richer audit summaries when the UI needs them.

use std::env;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

use crate::context::{ConversationHistory, TraceEntry, Turn};

#[derive(Clone)]
pub struct MemoryStore {
    backend: SessionBackend,
}

#[derive(Clone)]
enum SessionBackend {
    Files(Arc<FileSessionStore>),
    Unavailable { reason: Arc<String> },
}

struct FileSessionStore {
    sessions_dir: PathBuf,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredSessionFile {
    session_id: String,
    title: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    turns: Vec<StoredTurnRecord>,
}

#[derive(Clone, Serialize, Deserialize)]
struct StoredTurnRecord {
    query: String,
    response: String,
    model_name: String,
    created_at: DateTime<Utc>,
    trace: Vec<TraceEntry>,
}

impl MemoryStore {
    pub async fn new() -> Self {
        match FileSessionStore::initialize().await {
            Ok(store) => {
                info!(
                    sessions_dir = %store.sessions_dir.display(),
                    "Session persistence enabled with local files."
                );
                Self {
                    backend: SessionBackend::Files(Arc::new(store)),
                }
            }
            Err(error) => {
                let reason = format!("Session persistence is unavailable: {error}");
                warn!("{reason}");
                Self::unavailable(reason)
            }
        }
    }

    pub async fn create_session(&self) -> anyhow::Result<Session> {
        match &self.backend {
            SessionBackend::Files(store) => store.create_session().await,
            SessionBackend::Unavailable { reason } => Err(unavailable_error(reason)),
        }
    }

    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        match &self.backend {
            SessionBackend::Files(store) => store.list_sessions().await,
            SessionBackend::Unavailable { reason } => Err(unavailable_error(reason)),
        }
    }

    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<Session>> {
        match &self.backend {
            SessionBackend::Files(store) => store.get_session(session_id).await,
            SessionBackend::Unavailable { reason } => Err(unavailable_error(reason)),
        }
    }

    pub async fn append_turn(
        &self,
        session_id: &str,
        query: &str,
        response: &str,
        model_name: &str,
        trace: &[TraceEntry],
    ) -> anyhow::Result<()> {
        match &self.backend {
            SessionBackend::Files(store) => {
                store
                    .append_turn(session_id, query, response, model_name, trace)
                    .await
            }
            SessionBackend::Unavailable { reason } => Err(unavailable_error(reason)),
        }
    }

    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<bool> {
        match &self.backend {
            SessionBackend::Files(store) => store.delete_session(session_id).await,
            SessionBackend::Unavailable { reason } => Err(unavailable_error(reason)),
        }
    }

    fn unavailable(reason: impl Into<String>) -> Self {
        Self {
            backend: SessionBackend::Unavailable {
                reason: Arc::new(reason.into()),
            },
        }
    }
}

impl FileSessionStore {
    async fn initialize() -> anyhow::Result<Self> {
        let sessions_dir = session_storage_dir();
        fs::create_dir_all(&sessions_dir).await.with_context(|| {
            format!(
                "Could not create the AEGIS session storage directory `{}`.",
                sessions_dir.display()
            )
        })?;

        Ok(Self { sessions_dir })
    }

    async fn create_session(&self) -> anyhow::Result<Session> {
        let now = Utc::now();
        let stored = StoredSessionFile {
            session_id: Uuid::new_v4().to_string(),
            title: "New chat".to_string(),
            created_at: now,
            updated_at: now,
            turns: Vec::new(),
        };

        self.write_session_file(&stored).await?;
        Ok(stored.into_session())
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        let mut sessions = Vec::new();
        let mut directory = fs::read_dir(&self.sessions_dir).await.with_context(|| {
            format!(
                "Could not read the AEGIS session storage directory `{}`.",
                self.sessions_dir.display()
            )
        })?;

        while let Some(entry) = directory.next_entry().await.with_context(|| {
            format!(
                "Could not iterate through the AEGIS session storage directory `{}`.",
                self.sessions_dir.display()
            )
        })? {
            if !entry
                .file_type()
                .await
                .with_context(|| format!("Could not inspect `{}`.", entry.path().display()))?
                .is_file()
            {
                continue;
            }

            match self.read_session_path(&entry.path()).await {
                Ok(session) => sessions.push(session.summary()),
                Err(error) => warn!(
                    path = %entry.path().display(),
                    "Skipping unreadable stored session file: {error}"
                ),
            }
        }

        sessions.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(sessions)
    }

    async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<Session>> {
        Ok(self
            .read_session_file(session_id)
            .await?
            .map(StoredSessionFile::into_session))
    }

    async fn append_turn(
        &self,
        session_id: &str,
        query: &str,
        response: &str,
        model_name: &str,
        trace: &[TraceEntry],
    ) -> anyhow::Result<()> {
        let mut stored = self
            .read_session_file(session_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Session `{session_id}` was not found."))?;

        let now = Utc::now();
        if stored.title == "New chat" {
            stored.title = trimmed_title(query);
        }
        stored.updated_at = now;
        stored.turns.push(StoredTurnRecord {
            query: query.to_string(),
            response: response.to_string(),
            model_name: model_name.to_string(),
            created_at: now,
            trace: trace.to_vec(),
        });

        self.write_session_file(&stored).await
    }

    async fn delete_session(&self, session_id: &str) -> anyhow::Result<bool> {
        let path = self.session_path(session_id)?;

        match fs::remove_file(&path).await {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
            Err(error) => Err(anyhow::Error::new(error)
                .context(format!("Could not delete stored session `{session_id}`."))),
        }
    }

    async fn read_session_file(
        &self,
        session_id: &str,
    ) -> anyhow::Result<Option<StoredSessionFile>> {
        let path = self.session_path(session_id)?;

        match fs::read_to_string(&path).await {
            Ok(contents) => self.parse_session_file(&path, &contents).map(Some),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(anyhow::Error::new(error)
                .context(format!("Could not load stored session `{session_id}`."))),
        }
    }

    async fn read_session_path(&self, path: &Path) -> anyhow::Result<StoredSessionFile> {
        let contents = fs::read_to_string(path)
            .await
            .with_context(|| format!("Could not read stored session file `{}`.", path.display()))?;

        self.parse_session_file(path, &contents)
    }

    fn parse_session_file(&self, path: &Path, contents: &str) -> anyhow::Result<StoredSessionFile> {
        serde_json::from_str(contents).with_context(|| {
            format!(
                "Stored session file `{}` is not valid JSON text.",
                path.display()
            )
        })
    }

    async fn write_session_file(&self, session: &StoredSessionFile) -> anyhow::Result<()> {
        let path = self.session_path(&session.session_id)?;
        let payload = serde_json::to_string_pretty(session).with_context(|| {
            format!(
                "Could not serialize stored session `{}` into JSON text.",
                session.session_id
            )
        })?;

        fs::write(&path, payload)
            .await
            .with_context(|| format!("Could not write stored session `{}`.", path.display()))
    }

    fn session_path(&self, session_id: &str) -> anyhow::Result<PathBuf> {
        let session_id = normalize_session_id(session_id)?;
        Ok(self.sessions_dir.join(session_id))
    }
}

impl StoredSessionFile {
    fn into_session(self) -> Session {
        Session {
            session_id: self.session_id,
            title: self.title,
            history: ConversationHistory {
                turns: self
                    .turns
                    .into_iter()
                    .map(|turn| Turn {
                        query: turn.query,
                        response: turn.response,
                        created_at: turn.created_at,
                    })
                    .collect(),
            },
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    fn summary(&self) -> SessionSummary {
        SessionSummary {
            session_id: self.session_id.clone(),
            title: self.title.clone(),
            turn_count: self.turns.len(),
            updated_at: self.updated_at.clone(),
        }
    }
}

fn unavailable_error(reason: &str) -> anyhow::Error {
    anyhow::anyhow!("Session persistence is unavailable: {reason}")
}

fn trimmed_title(query: &str) -> String {
    let title: String = query.trim().chars().take(60).collect();
    if title.is_empty() {
        "New chat".to_string()
    } else {
        title
    }
}

fn normalize_session_id(session_id: &str) -> anyhow::Result<&str> {
    let session_id = session_id.trim();
    if session_id.is_empty() {
        anyhow::bail!("Session ids cannot be empty.");
    }

    if !session_id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        anyhow::bail!(
            "Session id `{session_id}` contains unsupported characters for local file storage."
        );
    }

    Ok(session_id)
}

fn session_storage_dir() -> PathBuf {
    env::var("AEGIS_SESSIONS_DIR")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_session_storage_dir)
}

fn default_session_storage_dir() -> PathBuf {
    resolve_home_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(".aegis")
        .join("sessions")
}

fn resolve_home_dir() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub session_id: String,
    pub title: String,
    pub history: ConversationHistory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub title: String,
    pub turn_count: usize,
    pub updated_at: DateTime<Utc>,
}
