# AEGIS Persistent Memory Architecture

## Goal

AEGIS should remember useful user facts and preferences without turning every answer into a replay of old memory. The memory system is intentionally local-first, transparent, editable, and retrieval-gated.

## Memory Layers

1. Short-term session memory

Session history is stored by `engine/src/memory_store/mod.rs` and compacted per request before inference. This memory answers: "What has happened in this conversation?"

2. Stable user profile

The markdown profile at `.aegis/user_profile.md` stores small facts and preferences the user explicitly saves. This memory answers: "What stable personalization should AEGIS know?"

3. Long-term semantic memory

The next best-fit implementation is a structured memory store backed by embeddings. Each record should include:

```json
{
  "id": "uuid",
  "text": "User prefers concise implementation summaries.",
  "category": "preference",
  "source": "explicit-save",
  "created_at": "timestamp",
  "updated_at": "timestamp",
  "last_used_at": "timestamp",
  "confidence": 0.95,
  "expires_at": null,
  "pinned": false
}
```

## Best-Fit Model

Use an embedding model for memory retrieval, not the chat model.

The best immediate fit is the existing local embedding stack already used by the Python services: `all-MiniLM-L6-v2`. It is small, fast, local, and good enough for retrieving short memories by semantic similarity. This avoids paying a full chat-model call on every user message and prevents the assistant from over-defaulting to memory.

Recommended retrieval flow:

1. Extract keywords from the current user query.
2. Detect explicit memory intent, such as "what do you know about me?" or "remember my preference".
3. Retrieve top semantic memories only when intent or similarity is strong enough.
4. Inject a bounded memory block into the prompt.
5. Trace which memory records were injected.

## Prompt Contract

Persistent memory must be injected as a separate section with explicit priority rules:

```text
Use selected persistent memory only as user-provided background.
Current user instructions, attached documents, and explicit task context override memory.
If memory is unrelated, ignore it.
If memory conflicts with the current request, follow the current request.
```

This keeps memory helpful but non-dominant.

## Current Implementation

The current implementation keeps markdown profile parsing but now builds a `MemoryInjection` from the current user query instead of scoring against the fully synthesized prompt. That prevents retrieved documents, project context, or session history from accidentally pulling unrelated memories into the model.

Unrelated preferences and instructions are no longer injected without keyword overlap or explicit profile-query intent.

## Future Work

- Add structured memory CRUD endpoints.
- Add memory deletion and "why was this memory used?" UI.
- Store memory provenance and confidence.
- Add semantic embedding retrieval through the existing Python service.
- Include memory IDs in trace metadata.
- Add memory budget accounting to the same compaction path as session history.
