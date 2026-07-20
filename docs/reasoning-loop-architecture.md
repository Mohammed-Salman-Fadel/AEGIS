# Reasoning Loop Architecture

AEGIS supports an optional reasoning mode for chat requests. The goal is to improve answer reliability by letting the engine plan, choose tools, inspect results, repair malformed tool-routing output, and only then produce the final response.

## User Experience

- The chat composer exposes a `Reason` toggle.
- The toggle is persisted in local storage so users can keep their preferred latency/correctness tradeoff.
- When enabled, general chat requests use the ReAct loop instead of the direct RAG path.
- Code workflows continue to use a read-only ReAct loop even when the toggle is off because file inspection is part of the expected code-assistant behavior.
- While the loop is running, the UI renders compact progress events such as planning, calling a tool, receiving a result, repairing invalid JSON, and composing the final answer.
- After the answer is produced, the reasoning trace stays collapsed by default and can be expanded by the user.

## Safety Boundary

The UI should not display hidden chain-of-thought or private model scratchpad text. Instead, the engine emits structured reasoning events:

```text
[REASONING_EVENT] {"phase":"tool_call","title":"Calling tool","round":2,"tool":"rag"}
```

These events are status summaries, not raw internal reasoning. This gives users useful transparency without exposing unreliable or sensitive internal tokens.

## Engine Flow

1. `POST /chat` accepts `reasoning_enabled`.
2. The orchestrator routes requests through the ReAct loop when either:
   - `reasoning_enabled` is true, or
   - the workflow is code explain, code generate, or code debug.
3. When active documents are attached, the orchestrator performs deterministic retrieval first and seeds the ReAct prompt with those excerpts.
4. The ReAct loop prompts the model to return JSON only:
   - a final answer, or
   - a tool call with arguments.
5. The engine validates and executes the selected tool.
6. Automatic reasoning runs are read-only. File writes are rejected, and terminal commands that create, edit, copy, move, or delete files are blocked.
7. Tool output is truncated before being returned to the loop.
8. Repeated identical tool calls are guarded against.
9. Malformed JSON gets one repair attempt.
10. If repair fails, AEGIS asks for a safe final answer rather than showing raw model output.
11. The loop stops after a bounded number of rounds and forces a sanitized final answer.

## UI Flow

1. The frontend sends `reasoning_enabled` with the chat request.
2. SSE chunks prefixed with `[REASONING_EVENT]` are parsed separately from answer tokens.
3. Parsed events are attached to the in-progress assistant message.
4. The message bubble shows a compact trace row.
5. The trace expands into a readable event list on demand.

## Performance Tradeoffs

Reasoning mode can improve correctness when the model needs to gather evidence, inspect files, or recover from uncertainty. It adds latency because each tool call can require another model round. For simple conversational prompts, users can leave it off and use the faster direct path.

## Future Improvements

- Add explicit per-tool allow/deny controls for users who want stricter autonomy or approved mutating actions.
- Persist reasoning events with chat history so previous traces remain inspectable after reload.
- Add elapsed time per reasoning event to make slow tool calls easier to diagnose.
- Add a small "why this tool" field generated from safe summaries, not chain-of-thought.
