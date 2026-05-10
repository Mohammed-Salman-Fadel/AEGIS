use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::context::{ConversationHistory, Turn};

#[derive(Clone)]
pub struct MemoryStore {
    sessions: Arc<Mutex<HashMap<String, Session>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn load_or_create(&self, session_id: &str) -> Session {
        let mut sessions = self.sessions.lock().expect("memory store lock poisoned");
        sessions
            .entry(session_id.to_string())
            .or_insert_with(|| Session::new(session_id.to_string()))
            .clone()
    }

    pub fn append_turn(&self, session_id: &str, query: String, response: String) {
        let mut sessions = self.sessions.lock().expect("memory store lock poisoned");
        let session = sessions
            .entry(session_id.to_string())
            .or_insert_with(|| Session::new(session_id.to_string()));

        if session.title == "New chat" {
            session.title = query.chars().take(60).collect();
        }

        session.history.turns.push(Turn {
            query,
            response,
            created_at: Utc::now(),
        });
        session.updated_at = Utc::now();
    }

    pub fn list_sessions(&self) -> Vec<SessionSummary> {
        let sessions = self.sessions.lock().expect("memory store lock poisoned");
        let mut summaries: Vec<_> = sessions
            .values()
            .map(|session| SessionSummary {
                session_id: session.session_id.clone(),
                title: session.title.clone(),
                turn_count: session.history.turns.len(),
                updated_at: session.updated_at,
            })
            .collect();
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        summaries
    }

    pub fn get_session(&self, session_id: &str) -> Option<Session> {
        let sessions = self.sessions.lock().expect("memory store lock poisoned");
        sessions.get(session_id).cloned()
    }
}

#[derive(Clone, Serialize)]
pub struct Session {
    pub session_id:  String,
    pub title:       String,
    pub history:     ConversationHistory,
    pub created_at:  DateTime<Utc>,
    pub updated_at:  DateTime<Utc>,
}

impl Session {
    fn new(session_id: String) -> Self {
        let now = Utc::now();
        Self {
            session_id,
            title: "New chat".to_string(),
            history: ConversationHistory::empty(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Serialize)]
pub struct SessionSummary {
    pub session_id:  String,
    pub title:       String,
    pub turn_count:  usize,
    pub updated_at:  DateTime<Utc>,
}
