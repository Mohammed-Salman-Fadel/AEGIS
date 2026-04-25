//! Role: session persistence layer for stored conversations.
//! Called by: `main.rs` during engine startup and `orchestrator/mod.rs` for session lifecycle and turn persistence.
//! Calls into: PostgreSQL through `tokio-postgres`.
//! Owns: creating, loading, listing, deleting, and appending stored session history.
//! Does not own: inference execution, HTTP routing, or CLI rendering.
//! Next TODOs: add session metadata updates, richer audit events, and connection pooling when traffic grows.

use std::env;
use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use tokio_postgres::{Client, NoTls};
use tracing::{info, warn};
use uuid::Uuid;

use crate::context::{ConversationHistory, TraceEntry, Turn};

#[derive(Clone)]
pub struct MemoryStore {
    backend: SessionBackend,
}

#[derive(Clone)]
enum SessionBackend {
    Postgres(Arc<PostgresSessionStore>),
    Unavailable { reason: Arc<String> },
}

struct PostgresSessionStore {
    client: Arc<Client>,
}

impl MemoryStore {
    pub async fn new() -> Self {
        let Some(database_url) = env::var("AEGIS_DATABASE_URL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
        else {
            let reason =
                "Set AEGIS_DATABASE_URL to enable Postgres-backed session persistence.".to_string();
            warn!("{reason}");
            return Self::unavailable(reason);
        };

        match PostgresSessionStore::connect(&database_url).await {
            Ok(store) => {
                info!("Session persistence enabled with PostgreSQL.");
                Self {
                    backend: SessionBackend::Postgres(Arc::new(store)),
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
            SessionBackend::Postgres(store) => store.create_session().await,
            SessionBackend::Unavailable { reason } => Err(unavailable_error(reason)),
        }
    }

    pub async fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        match &self.backend {
            SessionBackend::Postgres(store) => store.list_sessions().await,
            SessionBackend::Unavailable { reason } => Err(unavailable_error(reason)),
        }
    }

    pub async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<Session>> {
        match &self.backend {
            SessionBackend::Postgres(store) => store.get_session(session_id).await,
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
            SessionBackend::Postgres(store) => {
                store
                    .append_turn(session_id, query, response, model_name, trace)
                    .await
            }
            SessionBackend::Unavailable { reason } => Err(unavailable_error(reason)),
        }
    }

    pub async fn delete_session(&self, session_id: &str) -> anyhow::Result<bool> {
        match &self.backend {
            SessionBackend::Postgres(store) => store.delete_session(session_id).await,
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

impl PostgresSessionStore {
    async fn connect(database_url: &str) -> anyhow::Result<Self> {
        let (client, connection) = tokio_postgres::connect(database_url, NoTls)
            .await
            .with_context(|| "Could not connect to PostgreSQL.")?;

        tokio::spawn(async move {
            if let Err(error) = connection.await {
                warn!("PostgreSQL connection task ended: {error}");
            }
        });

        let store = Self {
            client: Arc::new(client),
        };
        store.initialize_schema().await?;
        Ok(store)
    }

    async fn initialize_schema(&self) -> anyhow::Result<()> {
        self.client
            .batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS aegis_sessions (
                    session_id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
                );

                CREATE TABLE IF NOT EXISTS aegis_session_turns (
                    id BIGSERIAL PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES aegis_sessions(session_id) ON DELETE CASCADE,
                    user_message TEXT NOT NULL,
                    assistant_message TEXT NOT NULL,
                    model_name TEXT NOT NULL,
                    trace JSONB NOT NULL DEFAULT '[]'::jsonb,
                    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
                );

                CREATE INDEX IF NOT EXISTS aegis_session_turns_session_id_created_idx
                    ON aegis_session_turns (session_id, created_at, id);
                "#,
            )
            .await
            .with_context(|| "Could not initialize the PostgreSQL schema for sessions.")?;

        Ok(())
    }

    async fn create_session(&self) -> anyhow::Result<Session> {
        let now = Utc::now();
        let session_id = Uuid::new_v4().to_string();
        let metadata = json!({
            "storage": "postgres",
            "created_by": "aegis-engine"
        });

        self.client
            .execute(
                "INSERT INTO aegis_sessions (session_id, title, created_at, updated_at, metadata)
                 VALUES ($1, $2, $3, $4, $5)",
                &[&session_id, &"New chat", &now, &now, &metadata],
            )
            .await
            .with_context(|| "Could not create a new stored session.")?;

        Ok(Session {
            session_id,
            title: "New chat".to_string(),
            history: ConversationHistory::empty(),
            created_at: now,
            updated_at: now,
        })
    }

    async fn list_sessions(&self) -> anyhow::Result<Vec<SessionSummary>> {
        let rows = self
            .client
            .query(
                r#"
                SELECT
                    s.session_id,
                    s.title,
                    s.updated_at,
                    COUNT(t.id) AS turn_count
                FROM aegis_sessions s
                LEFT JOIN aegis_session_turns t ON t.session_id = s.session_id
                GROUP BY s.session_id, s.title, s.updated_at
                ORDER BY s.updated_at DESC
                "#,
                &[],
            )
            .await
            .with_context(|| "Could not list stored sessions.")?;

        Ok(rows
            .into_iter()
            .map(|row| SessionSummary {
                session_id: row.get("session_id"),
                title: row.get("title"),
                turn_count: row.get::<_, i64>("turn_count") as usize,
                updated_at: row.get("updated_at"),
            })
            .collect())
    }

    async fn get_session(&self, session_id: &str) -> anyhow::Result<Option<Session>> {
        let session_row = self
            .client
            .query_opt(
                "SELECT session_id, title, created_at, updated_at
                 FROM aegis_sessions
                 WHERE session_id = $1",
                &[&session_id],
            )
            .await
            .with_context(|| format!("Could not load stored session `{session_id}`."))?;

        let Some(session_row) = session_row else {
            return Ok(None);
        };

        let turn_rows = self
            .client
            .query(
                "SELECT user_message, assistant_message, created_at
                 FROM aegis_session_turns
                 WHERE session_id = $1
                 ORDER BY created_at ASC, id ASC",
                &[&session_id],
            )
            .await
            .with_context(|| format!("Could not load turn history for session `{session_id}`."))?;

        let turns = turn_rows
            .into_iter()
            .map(|row| Turn {
                query: row.get("user_message"),
                response: row.get("assistant_message"),
                created_at: row.get("created_at"),
            })
            .collect();

        Ok(Some(Session {
            session_id: session_row.get("session_id"),
            title: session_row.get("title"),
            history: ConversationHistory { turns },
            created_at: session_row.get("created_at"),
            updated_at: session_row.get("updated_at"),
        }))
    }

    async fn append_turn(
        &self,
        session_id: &str,
        query: &str,
        response: &str,
        model_name: &str,
        trace: &[TraceEntry],
    ) -> anyhow::Result<()> {
        let next_title = trimmed_title(query);
        let trace_value = trace_json(trace);

        let updated_rows = self
            .client
            .execute(
                "UPDATE aegis_sessions
                 SET title = CASE WHEN title = 'New chat' THEN $2 ELSE title END,
                     updated_at = NOW()
                 WHERE session_id = $1",
                &[&session_id, &next_title],
            )
            .await
            .with_context(|| format!("Could not update stored session `{session_id}`."))?;

        if updated_rows == 0 {
            anyhow::bail!("Session `{session_id}` was not found.");
        }

        self.client
            .execute(
                "INSERT INTO aegis_session_turns
                    (session_id, user_message, assistant_message, model_name, trace, created_at)
                 VALUES ($1, $2, $3, $4, $5, NOW())",
                &[&session_id, &query, &response, &model_name, &trace_value],
            )
            .await
            .with_context(|| format!("Could not append a turn to session `{session_id}`."))?;

        Ok(())
    }

    async fn delete_session(&self, session_id: &str) -> anyhow::Result<bool> {
        let deleted_rows = self
            .client
            .execute(
                "DELETE FROM aegis_sessions WHERE session_id = $1",
                &[&session_id],
            )
            .await
            .with_context(|| format!("Could not delete stored session `{session_id}`."))?;

        if deleted_rows == 0 {
            return Ok(false);
        }

        Ok(true)
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

fn trace_json(trace: &[TraceEntry]) -> Value {
    serde_json::to_value(trace).unwrap_or_else(|_| json!([]))
}

#[derive(Clone, Serialize)]
pub struct Session {
    pub session_id: String,
    pub title: String,
    pub history: ConversationHistory,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub title: String,
    pub turn_count: usize,
    pub updated_at: DateTime<Utc>,
}
