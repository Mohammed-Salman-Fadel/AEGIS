# AEGIS weak-points remediation plan

Date: 2026-07-20

This plan reconciles `aegis-weakpoints-analysis.md` with the current tree. The
analysis is a useful backlog, but several findings describe an older revision.
The classifications below therefore distinguish already-patched behavior from
remaining production hardening.

## Architecture decisions

- Preserve the current orchestrator/ReAct split. The deterministic pipeline is
  useful for context assembly; iterative tool use belongs in `react_loop`.
- Keep JSON session compatibility for now, but serialize mutations per session
  and replace files through same-directory temporary files. SQLite remains the
  long-term answer for multi-process transactions, indexing, and migrations.
- Every outbound HTTP client must have explicit connect and total timeouts.
  Streaming inference receives a longer total timeout than metadata and RAG
  operations.
- Loopback remains zero-configuration. A non-loopback bind must fail closed
  unless an API key is configured; API routes then require that key. Bound
  concurrency limits protect both loopback and remote deployments.
- Session transfer uses a versioned envelope rather than exposing the internal
  on-disk representation as an undocumented permanent contract.
- First-message titles are deterministic and local. Imported-document title
  generation can remain model-assisted because it is an explicit import action,
  but should eventually become configurable.
- Large redesigns are delivered in vertical slices with tests and telemetry,
  not as partially connected scaffolding.

## Point-by-point disposition

1. **Agentic loop — patched, hardening remains.** `react_loop` implements
   bounded think/act/observe rounds and tool execution. Follow up with
   model-native structured tool calling, cancellation propagation, durable
   traces, and policy controls.
2. **Classifier stub — patched.** The classifier now has deterministic signals,
   scores, attachment awareness, and tests. Keep keyword routing as a fast path;
   add an optional small-model fallback only after an evaluation corpus exists.
3. **Compactor stub — patched, quality work remains.** The compactor now applies
   token budgets and history/slot reduction. Add semantic summarization,
   summary provenance, and regression fixtures for long multilingual sessions.
4. **MemoryStore concurrency/atomicity — unresolved (P0).** Add per-session
   mutation serialization and same-directory replacement writes now. Add
   concurrency and recovery tests. Phase 2 migrates to SQLite/WAL for true
   cross-process transactions.
5. **Hardcoded context window — patched.** Ollama runtime/model metadata feeds
   the active model profile with a conservative fallback. Extend provider
   adapters where metadata is available and cache with bounded staleness.
6. **Environment-only configuration — partial (P2).** Provider selection is
   persisted, and `toml` is available, but there is no unified typed file.
   Phase 1: optional `aegis.toml`; precedence is environment > file > defaults.
   Phase 2: schema validation and redacted effective-config diagnostics. Do not
   persist secrets by default.
7. **Mostly-dead WorkflowRegistry — partial.** Classification is active, but
   workflow behavior remains uneven. Define executable workflow contracts and
   delete variants that have no distinct behavior after measuring real usage.
8. **Observability — partial.** Structured tracing and user-visible inference/
   retrieval metrics exist. Add request IDs, histograms, error counters, tool
   latency, readiness details, and an OpenTelemetry/Prometheus opt-in exporter.
9. **Hardcoded tools/no discovery — partial.** ReAct can select tools and MCP
   support exists, but capabilities are still assembled statically. Introduce a
   normalized capability schema, per-request allowlists, and MCP discovery
   caching before exposing arbitrary remote tool servers.
10. **HTTP auth/security — unresolved for remote bind (P0).** Require
    `AEGIS_API_KEY` for non-loopback hosts, authenticate `/api/*`, keep health
    probes available, cap concurrent requests, retain body-size limits, and
    avoid logging credentials. Later add per-client token-bucket limits and TLS
    termination guidance.
11. **RAG process readiness — partial (P1).** Startup work is backgrounded and
    bounded, so the UI is not blocked. Add live/degraded readiness reporting and
    retry state; later let the process manager supervise a configured local RAG
    child process.
12. **Session export/import — unresolved (P1).** Add versioned JSON envelopes,
    strict size/count validation, collision-safe default import IDs, and explicit
    overwrite semantics. Add UI actions only after API round-trip tests.
13. **LLM-generated titles — unresolved for first turns (P1).** Replace the
    automatic first-turn inference call with deterministic normalization.
    Preserve manual rename and cap title length.
14. **Brittle scope heuristics — partial.** The classifier reduces duplication,
    but several orchestrator guards remain. Consolidate routing into one scored
    decision object and validate it against a labeled prompt corpus before
    removing conservative guards.
15. **Missing HTTP timeouts — unresolved in several clients (P0).** Build
    reusable clients with explicit connect and total timeouts for Ollama,
    OpenAI-compatible, RAG, and model-management calls. Keep streaming budgets
    configurable in a later pass.
16. **Frontend state duplication — unresolved (P2/architectural).** Extract API,
    session, settings, voice, and import domains incrementally behind hooks and
    reducers. Add component tests before moving state; avoid a one-shot Zustand
    rewrite while `App.tsx` has unrelated active development.

## Immediate implementation scope

This remediation pass targets #4, #10, #15, #13, and, if the API surface remains
coherent, #11/#12. It adds focused unit/integration tests and does not overwrite
the substantial unrelated CLI, engine, and frontend work already present.

## Constraints and acceptance criteria

- Existing session files remain readable.
- Concurrent appends to one session do not lose turns inside one engine process.
- Interrupted writes never expose a partially serialized primary session file.
- A non-loopback bind without an API key is rejected at startup.
- With a key configured, protected API calls without a valid key receive 401.
- Network calls terminate within documented bounds.
- First-turn title creation causes no extra inference call.
- Exported data declares a schema version; unsupported versions and oversized
  imports fail safely.
- Focused tests and `cargo check` pass. Failures caused by unrelated dirty work
  are reported rather than hidden.

## Follow-up phases

1. Add benchmark/evaluation corpora for routing, compaction, and tool selection.
2. Migrate sessions to SQLite/WAL with an automatic, reversible JSON importer.
3. Add request-scoped cancellation, IDs, metrics, and per-client rate limiting.
4. Unify typed configuration and expose redacted diagnostics/readiness.
5. Normalize dynamic tool capabilities and enforce explicit execution policy.
6. Decompose frontend state domain-by-domain with behavior-preserving tests.
