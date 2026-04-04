use crate::context::{ConversationHistory, RequestContext};

pub struct MemoryStore;

impl MemoryStore {
    pub fn new() -> Self { Self }

    pub fn load_or_create(&self, session_id: &str) -> Session {
        // TODO: load from disk, create new if not found
        Session {
            session_id: session_id.to_string(),
            history:    ConversationHistory::empty(),
        }
    }

    pub fn append_turn(&self, session_id: &str, ctx: &RequestContext) {
        // TODO: persist the turn to disk
    }
}

pub struct Session {
    pub session_id: String,
    pub history:    ConversationHistory,
}
