use std::collections::HashMap;
use uuid::Uuid;

use crate::model_registry::ModelProfile;

/// One turn in the conversation — a user message and the assistant's response.
pub struct Turn {
    pub query:    String,
    pub response: String,
}

/// The conversation history for a session.
pub struct ConversationHistory {
    pub turns: Vec<Turn>,
}

impl ConversationHistory {
    pub fn empty() -> Self {
        Self { turns: Vec::new() }
    }
}

/// A single entry recording what a phase did and how many tokens it used.
pub struct TraceEntry {
    pub phase:       String,
    pub tokens_used: usize,
    pub summary:     Option<String>,
}

/// Values stored in context slots — what nodes read from and write to.
pub enum SlotValue {
    Text(String),
    Json(serde_json::Value),
}

/// The central state object — lives for the entire duration of one request.
/// Every phase reads from and writes to this.
pub struct RequestContext {
    pub request_id:     Uuid,
    pub session_id:     String,
    pub original_query: String,
    pub history:        ConversationHistory,
    pub model:          ModelProfile,
    pub slots:          HashMap<String, SlotValue>,
    pub trace:          Vec<TraceEntry>,
}

impl RequestContext {
    pub fn new(
        session_id:     String,
        original_query: String,
        history:        ConversationHistory,
        model:          ModelProfile,
    ) -> Self {
        Self {
            request_id: Uuid::new_v4(),
            session_id,
            original_query,
            history,
            model,
            slots: HashMap::new(),
            trace: Vec::new(),
        }
    }

    /// Insert a value into the context slots.
    pub fn insert(&mut self, key: &str, value: SlotValue) {
        self.slots.insert(key.to_string(), value);
    }

    /// Get a value from the context slots.
    pub fn get(&self, key: &str) -> Option<&SlotValue> {
        self.slots.get(key)
    }

    /// Append a trace entry.
    pub fn trace(&mut self, phase: &str, tokens_used: usize) {
        self.trace.push(TraceEntry {
            phase:   phase.to_string(),
            tokens_used,
            summary: None,
        });
    }

    /// Total tokens used across all phases so far.
    pub fn total_tokens_used(&self) -> usize {
        self.trace.iter().map(|e| e.tokens_used).sum()
    }
}
