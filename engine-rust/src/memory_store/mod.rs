// Memory Store — persists sessions and conversation history across requests
//
// TODO: Session {
//     session_id: String,
//     history:    ConversationHistory,
//     created_at: DateTime,
//     updated_at: DateTime,
// }
//
// TODO: ConversationHistory {
//     turns: Vec<Turn>,
// }
//
// TODO: Turn {
//     query:     String,
//     response:  String,
//     timestamp: DateTime,
// }
//
// TODO: impl ConversationHistory
//   → recent_turns(n: usize) -> &[Turn]   — last N turns for prompt
//   → compress_oldest()                    — called by compactor
//   → token_estimate() -> usize
//
// TODO: MemoryStore struct (SQLite or flat JSON files for MVP)
//
// TODO: impl MemoryStore
//   → load_or_create(session_id: &str) -> Session
//   → save(session: &Session) -> Result<()>
//   → append_turn(session_id: &str, turn: Turn) -> Result<()>
//   → list_sessions() -> Result<Vec<String>>
//   → delete_session(session_id: &str) -> Result<()>
