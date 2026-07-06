# AEGIS — Weakpoints & High-Impact Improvements

> **Codebase:** ~14K LOC Rust engine + ~1K LOC Python RAG + ~6.3K LOC TSX frontend + ~6.4K LOC Rust CLI
> **Status:** Early-stage (v0.1.0) — solid skeleton architecture, many stubs and TODOs

---

## 🔴 Critical Weakpoints (Blocking Progress)

### 1. No True Agentic Loop — Single-Pass Pipeline
**Where:** `engine/src/orchestrator/mod.rs` — `handle_fallback()`

The core "loop" is a **linear pipeline** with no iteration:
```
Classify → RAG → Code Search → Zotero → Prompt Synthesis → LLM → Done
```

The LLM never calls tools. It only sees pre-injected context. There's no:
- Tool selection loop (LLM picks what tool to call)
- Multi-turn reasoning (think → act → observe → think again)
- Feedback loop from LLM output back into context gathering
- The `execute_steps()` method exists but is never called from `handle_fallback()`

**Fix impact: ★★★★★** — This is the single biggest architectural gap. Without it, AEGIS is a glorified RAG chatbot, not an agent.

### 2. Classifier Is a Stub
**Where:** `engine/src/classifier/mod.rs` (15 lines)

```rust
pub fn classify(&self, _ctx: &RequestContext) -> WorkflowId {
    // TODO: heuristics + small model fallback
    WorkflowId::Default
}
```

**Impact:** Every query defaults to the same workflow. The rich `WorkflowRegistry` (DocumentQA, CodeExplain, CodeDebug, Summarize, etc.) is **entirely dead code** — nothing ever selects a different workflow.

**Fix impact: ★★★★★** — 20 lines of keyword matching would unlock the entire workflow system. This is a weekend-level change with massive architectural payoff.

### 3. Compactor Is a Stub
**Where:** `engine/src/compactor/mod.rs` (13 lines)

```rust
pub fn compact(&self, _ctx: &mut RequestContext) {
    // TODO: measure tokens, compress history, summarize large slots
}
```

**Impact:** Context management is non-existent. Sessions grow unbounded. After 5-10 turns with large documents, the prompt exceeds context windows and behavior degrades. This is the #1 user-facing reliability issue.

**Fix impact: ★★★★★** — Implement context budget tracking and history summarization. Medium effort, critical for any real-world usage.

### 4. No File Locking in MemoryStore
**Where:** `engine/src/memory_store/mod.rs`

Session files are written with `tokio::fs::write()` — a read-modify-write pattern with no file locking. Two concurrent requests for the same session cause:
- Lost writes (last writer wins, overwriting earlier writes)
- Corrupted session files (partial writes from concurrent modifications)
- Race conditions on turn append (→ edit conflicts)

**Fix impact: ★★★★☆** — Add file-level advisory locks or switch to SQLite. Medium effort, essential for production readiness.

---

## 🟠 Significant Weakpoints

### 5. Hardcoded Context Window (8192)
**Where:** `engine/src/model_registry/mod.rs:53`

```rust
pub fn get_active(&self) -> ModelProfile {
    ModelProfile {
        name: self.current_model_name(),
        context_window: 8192,        // Hardcoded!
        output_reserve: 512,
    }
}
```

**Impact:** Llama 3.2 supports 128K context. Mistral supports 32K+. By hardcoding 8192, you're wasting 90%+ of available context on capable models. The Ollama backend *can* query the real context window via `/api/show` and `/api/ps` — but nothing uses that data here.

**Fix impact: ★★★★☆** — 5 lines in `ModelRegistry` to pass through the backend's real context window. Trivial effort, immediate performance gains.

### 6. Config Is 100% Env-Var Driven — No Config File
**Where:** `engine/src/config.rs`

Every setting comes from environment variables (`AEGIS_INFERENCE_PROVIDER`, `AEGIS_MODEL`, `AEGIS_OLLAMA_URL`, etc.). There is:
- No config file (TOML/YAML)
- No config validation at startup
- No type safety (strings parsed ad-hoc)
- No way to save persistent configuration

The `AppConfig::from_env()` returns an `anyhow::Result` — but nothing validates that URLs are reachable or providers exist.

**Fix impact: ★★★★☆** — Add a `aegis.toml` reader alongside env vars. Low effort, huge UX improvement. Users shouldn't need `.bashrc` changes to configure AEGIS.

### 7. WorkflowRegistry Is Mostly Dead Code
**Where:** `engine/src/workflow/`

The system defines: Default, DocumentQA, Summarize, CodeExplain, CodeGenerate, CodeDebug, Writing

**Implemented:** Default (3 phases) + DocumentQA (3 phases) **only**. The rest have the TODO comment:
```rust
// TODO: remaining workflows
```

Combined with the stub Classifier, this is ~200 lines of scaffolding that does nothing.

**Fix impact: ★★★★☆** — Either implement workflows or simplify to remove the abstraction overhead. The abstraction adds complexity without delivering value in its current state.

### 8. No Observability / Metrics
**Where:** Everywhere

- No structured metrics (tokens used, latency, error rates)
- No health endpoint beyond bare "is it running"
- No tracing of RAG query times, inference latency, tool call durations
- Token accounting is best-effort (optional fields, default to 0)

**Impact:** You can't answer "how is the system performing" or "what's slow" without instrumenting it yourself. Debugging production issues is guesswork.

**Fix impact: ★★★☆☆** — Add metrics counters wrapped around the `handle_fallback` pipeline. Medium effort, pays for itself on the first production issue.

### 9. Hardcoded Tools — No LLM Tool Discovery
**Where:** `engine/src/tool_registry/mod.rs`

The `ToolRegistry` only supports two tools: Semble (code search) and Zotero (research). The `execute()` method is a `match` on hardcoded strings. The LLM **never discovers or calls tools** — context is gathered by the engine, not by the model.

This is the architecture's most fundamental design tension: it calls itself an "agent" but has no tool calling loop.

**Fix impact: ★★★★★** — This requires the agentic loop redesign (#1). Not a standalone fix.

### 10. No Auth/Security on the HTTP Layer
**Where:** `engine/src/network/`

The Axum server binds to `127.0.0.1:{port}` by default — no authentication, no rate limiting, no request validation beyond deserialization. Any process or script on the machine can send arbitrary chat requests.

**Fix impact: ★★★☆☆** — Add a simple API key header check. Easy win when running non-local.

---

## 🟡 Lower-Impact but Easy Wins

### 11. RAG Service Is a Separate Python Process
The Python FastAPI service adds a deployment dependency (Python venv, Sentence-Transformers, FAISS). It's another process to monitor, another startup step, and another point of failure. The engine starts up even if the RAG service is down (it just logs a warning).

**Easy fix:** Add a health-check retry loop in the engine with proper degradation. Document the dependency clearly.

### 12. No Session Export/Import
The `MemoryStore` has TODOs for this. Users have no way to back up or transfer conversations.

**Easy fix:** Add JSON export/import endpoints. ~50 lines of Rust.

### 13. Session Title Generation Is a Full LLM Call
**Where:** `orchestrator/mod.rs` — `generate_session_title()`

Each new session pays a full inference call just to generate a 3-7 word title. For local models, this adds 5-15 seconds to session creation.

**Easy fix:** Generate titles client-side from the first message, or use a tiny model, or allow manual naming as the default.

### 14. `is_code_scoped_request` and Friends Are Brittle Keyword Matching
The routing heuristics (`is_code_scoped_request`, `is_document_scoped_request`, `is_research_scoped_request`) are keyword lists. They'll miss obvious cases or misfire.

**Fix:** Replace with the classifier (fix #2) or a tiny on-device classifier model.

### 15. No HTTP Request Timeouts Configured
**Where:** `engine/src/inference/backends/ollama.rs`

The `reqwest::Client::new()` has default timeouts (none). A hung Ollama process hangs the engine indefinitely.

**Easy fix:** `Client::builder().timeout(Duration::from_secs(120)).build()`

### 16. Frontend Has State Duplication
**Where:** `frontend/src/App.tsx`

The frontend manages: session list, messages, settings, RAG state, voice, calendar — all in one monolithic `App` component with `useState`. This makes the component ~2000+ lines and hard to reason about.

**Fix:** Split state into dedicated contexts or use Zustand.

---

## 🎯 The Pyramid of Impact

Here's how to prioritize — start at the top, work down:

```
▲ HIGHEST IMPACT (smallest change, biggest result)
│
│  1. Fix Classifier stub → unlock WorkflowRegistry       [1-2 days]
│  2. Dynamic context_window from backend                  [2 hours]
│  3. Implement Compactor → prevent context overflow       [2-3 days]
│  4. Add config file (aegis.toml)                         [1 day]
│  5. Add HTTP timeouts to inference clients               [30 min]
│  6. Add file locking to MemoryStore                      [1-2 days]
│  7. Session export/import                                 [half day]
│  8. Add request timeout middleware                        [30 min]
│
│  ──── Mid-Term ────
│
│  9. True agentic loop (tool-calling LLM)                 [1-2 weeks]
│  10. Auth/API key for HTTP layer                         [1 day]
│  11. Observability framework                             [2-3 days]
│  12. Config validation at startup                        [1 day]
│
│  ──── Long-Term ────
│
│  13. Replace JSON session storage with SQLite            [1 week]
│  14. Multi-model routing (small model for classify)      [1-2 weeks]
│  15. Replace keyword routing with lightweight classifier [1 week]
│  16. Split monolithic frontend App component             [2-3 days]
▼ LEAST URGENT
```

The **#1 biggest unlock** for the smallest effort is: **fix the Classifier** (20 lines of code) + **dynamic context window** (5 lines). These two trivial changes immediately make AEGIS feel like it's using its full design instead of pretending.

**#2 biggest unlock** for user-facing reliability: **implement the Compactor**. Without it, every long session degrades into garbage-in/garbage-out as context overflows.

**#3** — the **agentic loop redesign** is the largest payoff but also the largest effort. It's not a "small change" — it's a fundamental architecture shift from pipeline to loop. But it's what transforms AEGIS from a RAG chatbot into a true AI agent.
