//! ReAct (Reasoning + Acting) tool-use loop.
//!
//! The model iteratively decides what to do: call a tool to gather
//! information, or produce the final answer.  Each tool result is
//! appended as context for the next round, building up a chain of
//! reasoning until the model has enough to answer definitively.
//!
//! Locking strategy
//! ─────────────────
//! `execute()` takes `&RwLock<Box<dyn InferenceBackend>>` and
//! acquires the read lock *inside each round*, releasing it between
//! rounds.  This prevents the ReAct loop from blocking writes
//! (e.g. `switch_provider()`) for the loop's entire duration.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::sync::{RwLock, mpsc};

use crate::inference::InferenceBackend;
use crate::rag_client::RagClient;
use crate::tool_registry::ToolRegistry;
use crate::workflow::WorkflowId;

// ── constants ─────────────────────────────────────────────────────────

const MAX_ROUNDS: usize = 6;
const MAX_FILE_CHARS: usize = 10_000;
const MAX_TOOL_OUTPUT_CHARS: usize = 20_000;
const MAX_TERMINAL_OUTPUT_CHARS: usize = 10_000;
const TERMINAL_TIMEOUT_SECS: u64 = 30;
const MAX_SEARCH_RESULTS: usize = 50;
const LIST_DIR_MAX_ENTRIES: usize = 100;
const GIT_TIMEOUT_SECS: u64 = 15;
const MAX_IMAGE_BYTES: u64 = 10_000_000; // 10 MB

/// Shared HTTP client used by all tool implementations that make
/// external HTTP requests (OCR, vision model).  Reuses connections
/// and avoids the overhead of creating a new client per call.
static SHARED_HTTP_CLIENT: once_cell::sync::Lazy<reqwest::Client> =
    once_cell::sync::Lazy::new(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .expect("Failed to create HTTP client")
    });

// ── public types ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum ToolCall {
    Search(String),
    ReadFile(String),
    WriteFile(String, String),
    SearchFiles(String, Option<String>),
    RunTerminal(String),
    GitStatus(String),
    ListDirectory(String),
    Calculate(String),
    KnowledgeSearch(String),
    OcrImage(String),
    DescribeImage(String),
    Rag(String),
    Zotero(String),
}

impl ToolCall {
    pub fn tool_name(&self) -> &'static str {
        match self {
            ToolCall::Search(_) => "search",
            ToolCall::ReadFile(_) => "read_file",
            ToolCall::WriteFile(..) => "write_file",
            ToolCall::SearchFiles(..) => "search_files",
            ToolCall::RunTerminal(_) => "run_terminal",
            ToolCall::GitStatus(_) => "git_status",
            ToolCall::ListDirectory(_) => "list_directory",
            ToolCall::Calculate(_) => "calculate",
            ToolCall::KnowledgeSearch(_) => "search_knowledge",
            ToolCall::OcrImage(_) => "ocr_image",
            ToolCall::DescribeImage(_) => "describe_image",
            ToolCall::Rag(_) => "rag",
            ToolCall::Zotero(_) => "zotero",
        }
    }
}

#[derive(Debug, Clone)]
struct ToolPlan {
    allowed: Vec<&'static str>,
    max_calls: usize,
    rationale: &'static str,
}

impl ToolPlan {
    fn allows(&self, tool: &ToolCall) -> bool {
        self.allowed.contains(&tool.tool_name())
    }

    fn is_direct(&self) -> bool {
        self.allowed.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub tool_name: String,
    pub input: String,
    pub output: String,
}

/// Result from a ReAct loop execution, including token usage.
#[derive(Debug, Clone)]
pub struct ReActResult {
    pub text: String,
    pub prompt_tokens: Option<usize>,
    pub completion_tokens: Option<usize>,
}

// ── react loop ────────────────────────────────────────────────────────

pub struct ReactLoop;

#[derive(Serialize)]
struct ReasoningEvent<'a> {
    phase: &'a str,
    title: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    round: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool: Option<&'a str>,
}

impl ReactLoop {
    /// Run the ReAct loop for a single user query.
    ///
    /// * `inference_lock` — read-locked per round so writes (provider/model
    ///   switches) can acquire the write lock between rounds.
    /// * `project_path` — optional code project path for scoped searches.
    pub async fn execute(
        inference_lock: &RwLock<Box<dyn InferenceBackend + Send + Sync>>,
        tool_registry: &ToolRegistry,
        rag_client: &RagClient,
        query: &str,
        routing_query: &str,
        conversation_context: &str,
        session_id: &str,
        model_name: &str,
        project_path: Option<&str>,
        workflow: WorkflowId,
        mode: &str,
        has_attachments: bool,
        tx: mpsc::Sender<String>,
    ) -> Result<String> {
        let tool_plan = select_tool_plan(
            routing_query,
            workflow,
            mode,
            has_attachments,
            project_path.is_some(),
        );

        if tool_plan.is_direct() {
            send_reasoning_event(
                &tx,
                ReasoningEvent {
                    phase: "route_direct",
                    title: "Answering without tools",
                    detail: Some(tool_plan.rationale),
                    round: None,
                    tool: None,
                },
            )
            .await;
            let answer =
                direct_reasoned_answer(inference_lock, query, conversation_context, model_name)
                    .await?;
            send_reasoning_event(
                &tx,
                ReasoningEvent {
                    phase: "final",
                    title: "Final answer ready",
                    detail: Some("No external tools were needed for this request."),
                    round: Some(1),
                    tool: None,
                },
            )
            .await;
            return Ok(answer);
        }

        let system_prompt = build_system_prompt(&tool_plan);
        let mut conversation =
            build_reasoning_conversation(&system_prompt, conversation_context, query);
        let mut last_call: Option<(String, String)> = None;
        let mut tool_counts = HashMap::<&'static str, usize>::new();

        send_reasoning_event(
            &tx,
            ReasoningEvent {
                phase: "route_tools",
                title: "Using selected tools",
                detail: Some(tool_plan.rationale),
                round: None,
                tool: None,
            },
        )
        .await;

        let max_rounds = (tool_plan.max_calls + 1).min(MAX_ROUNDS);
        for round in 1..=max_rounds {
            send_reasoning_event(
                &tx,
                ReasoningEvent {
                    phase: "thinking",
                    title: "Thinking through next step",
                    detail: Some("Deciding whether to answer directly or gather more context."),
                    round: Some(round),
                    tool: None,
                },
            )
            .await;

            // ── call the model ──────────────────────────────────────
            // Acquire the read lock, call inference, release before
            // tool execution so writes can proceed between rounds.
            let raw = {
                let inference = inference_lock.read().await;
                let prompt = format!("{}\n\n[Assistant — respond with JSON only]", conversation);
                call_with_retry(&**inference, &prompt, model_name, round).await?
            };

            // ── parse the response ──────────────────────────────────
            let decision = match parse_react_response(&raw) {
                Ok(d) => d,
                Err(e) => {
                    send_reasoning_event(
                        &tx,
                        ReasoningEvent {
                            phase: "repair",
                            title: "Repairing reasoning output",
                            detail: Some("The model returned malformed tool JSON, so AEGIS is asking for a valid action."),
                            round: Some(round),
                            tool: None,
                        },
                    )
                    .await;
                    // Re-prompt once: tell the model its JSON was invalid.
                    let correction_prompt = format!(
                        "{}\n\nYour previous response contained invalid JSON: {e}\n\
                         Please respond with valid JSON only. Either a tool call or a final answer.\n\n\
                         [Assistant — valid JSON only]",
                        conversation
                    );
                    let inference = inference_lock.read().await;
                    let raw2 = call_with_retry(&**inference, &correction_prompt, model_name, round)
                        .await?;
                    drop(inference);
                    match parse_react_response(&raw2) {
                        Ok(d) => d,
                        Err(_e2) => {
                            // Second failure: do not expose raw malformed JSON,
                            // scratchpad fields, or hidden thinking to the user.
                            send_reasoning_event(
                                &tx,
                                ReasoningEvent {
                                    phase: "fallback",
                                    title: "Falling back to direct response",
                                    detail: Some("Tool routing could not be parsed reliably, so the model response is returned directly."),
                                    round: Some(round),
                                    tool: None,
                                },
                            )
                            .await;
                            return safe_final_answer(
                                inference_lock,
                                &conversation,
                                model_name,
                                round,
                            )
                            .await;
                        }
                    }
                }
            };

            let decision = if matches!(decision, ModelDecision::FinalAnswer(_))
                && tool_counts.values().sum::<usize>() == 0
            {
                send_reasoning_event(
                    &tx,
                    ReasoningEvent {
                        phase: "guard",
                        title: "Verifying with the selected tool",
                        detail: Some(
                            "This route requires real tool evidence before AEGIS can answer.",
                        ),
                        round: Some(round),
                        tool: None,
                    },
                )
                .await;
                match required_tool_fallback(&tool_plan, routing_query) {
                    Some(tool) => ModelDecision::Tool(tool),
                    None => {
                        conversation.push_str(
                            "\n[System guard]\nA tool result is required for this request. Call one allowed tool now; do not answer yet.",
                        );
                        continue;
                    }
                }
            } else {
                decision
            };

            match decision {
                ModelDecision::FinalAnswer(answer) => {
                    send_reasoning_event(
                        &tx,
                        ReasoningEvent {
                            phase: "final",
                            title: "Final answer ready",
                            detail: Some(
                                "The model has enough context and is composing the response.",
                            ),
                            round: Some(round),
                            tool: None,
                        },
                    )
                    .await;
                    return Ok(answer);
                }
                ModelDecision::Tool(tool_call) => {
                    if !tool_plan.allows(&tool_call) {
                        send_reasoning_event(
                            &tx,
                            ReasoningEvent {
                                phase: "guard",
                                title: "Skipping an unnecessary tool",
                                detail: Some("The requested tool was outside the route selected for this question."),
                                round: Some(round),
                                tool: Some(tool_call.tool_name()),
                            },
                        )
                        .await;
                        return safe_final_answer(inference_lock, &conversation, model_name, round)
                            .await;
                    }

                    let count = tool_counts.entry(tool_call.tool_name()).or_default();
                    let per_tool_limit = if tool_call.tool_name() == "read_file" {
                        2
                    } else {
                        1
                    };
                    if *count >= per_tool_limit {
                        send_reasoning_event(
                            &tx,
                            ReasoningEvent {
                                phase: "guard",
                                title: "Tool budget reached",
                                detail: Some("AEGIS already gathered enough signal from this tool and will answer with the available context."),
                                round: Some(round),
                                tool: Some(tool_call.tool_name()),
                            },
                        )
                        .await;
                        return safe_final_answer(inference_lock, &conversation, model_name, round)
                            .await;
                    }
                    *count += 1;
                    // Dedup guard: detect repeated identical tool calls.
                    let call_sig = (
                        tool_call.tool_name().to_string(),
                        match &tool_call {
                            ToolCall::Search(q)
                            | ToolCall::ReadFile(q)
                            | ToolCall::RunTerminal(q)
                            | ToolCall::Rag(q)
                            | ToolCall::Zotero(q) => q.clone(),
                            ToolCall::WriteFile(p, _c) => p.clone(),
                            ToolCall::SearchFiles(p, _) => p.clone(),
                            ToolCall::GitStatus(p)
                            | ToolCall::ListDirectory(p)
                            | ToolCall::Calculate(p)
                            | ToolCall::KnowledgeSearch(p)
                            | ToolCall::OcrImage(p)
                            | ToolCall::DescribeImage(p) => p.clone(),
                        },
                    );
                    if let Some(last) = &last_call {
                        if *last == call_sig {
                            send_reasoning_event(
                                &tx,
                                ReasoningEvent {
                                    phase: "guard",
                                    title: "Avoiding repeated tool call",
                                    detail: Some("The same tool call was requested twice, so AEGIS is nudging the model to use a different approach."),
                                    round: Some(round),
                                    tool: Some(tool_call.tool_name()),
                                },
                            )
                            .await;
                            conversation.push_str(&format!(
                                "\n[Note: You just called `{}` with the same \
                                 arguments. Try a different approach or provide \
                                 your final answer.]",
                                tool_call.tool_name()
                            ));
                            continue;
                        }
                    }
                    last_call = Some(call_sig);
                    let call_detail = tool_call_detail(&tool_call);
                    send_reasoning_event(
                        &tx,
                        ReasoningEvent {
                            phase: "tool_call",
                            title: "Calling tool",
                            detail: Some(&call_detail),
                            round: Some(round),
                            tool: Some(tool_call.tool_name()),
                        },
                    )
                    .await;

                    // Execute the tool (no lock is held during this).
                    let result = execute_tool(
                        &tool_call,
                        tool_registry,
                        rag_client,
                        session_id,
                        project_path,
                        &tx,
                    )
                    .await;

                    let result_str = match result {
                        Ok(r) => {
                            let truncated = truncate(&r.output, MAX_TOOL_OUTPUT_CHARS);
                            let result_detail = tool_result_detail(&r);
                            send_reasoning_event(
                                &tx,
                                ReasoningEvent {
                                    phase: "tool_result",
                                    title: "Tool result received",
                                    detail: Some(&result_detail),
                                    round: Some(round),
                                    tool: Some(&r.tool_name),
                                },
                            )
                            .await;
                            format!("\n[Tool Result: {}]\n```\n{}\n```", r.tool_name, truncated)
                        }
                        Err(e) => {
                            send_reasoning_event(
                                &tx,
                                ReasoningEvent {
                                    phase: "tool_error",
                                    title: "Tool returned an error",
                                    detail: Some("The model will try another route or answer with available context."),
                                    round: Some(round),
                                    tool: Some(tool_call.tool_name()),
                                },
                            )
                            .await;
                            tracing::warn!(tool = tool_call.tool_name(), error = %e, "optional reasoning tool failed");
                            conversation.push_str(&format!(
                                "\n[Tool unavailable: {}]\nThis optional tool could not be used. Do not infer an engine, model, or pipeline failure from this. Answer with reliable existing context and mention the unavailable tool only if it prevents answering.",
                                tool_call.tool_name()
                            ));
                            return safe_final_answer(
                                inference_lock,
                                &conversation,
                                model_name,
                                round,
                            )
                            .await;
                        }
                    };

                    conversation.push_str(&result_str);
                }
            }
        }

        // ── round limit reached — force final answer ────────────────
        send_reasoning_event(
            &tx,
            ReasoningEvent {
                phase: "limit",
                title: "Reasoning limit reached",
                detail: Some("AEGIS is forcing a final answer from the gathered context."),
                round: Some(max_rounds),
                tool: None,
            },
        )
        .await;

        let force_prompt = format!(
            "{}\n\nYou have used the available reasoning rounds. \
             Provide your final answer now. Do not call any more tools.\n\n\
             [Assistant — final answer only, no JSON]",
            conversation
        );

        // Best-effort: never return the internal loop transcript directly.
        let inference = inference_lock.read().await;
        match inference.call(&force_prompt, model_name).await {
            Ok(answer) => Ok(sanitize_final_answer(&answer)),
            Err(e) => Ok(format!(
                "I reached the reasoning limit and could not safely generate a final answer: {e}"
            )),
        }
    }
}

async fn send_reasoning_event(tx: &mpsc::Sender<String>, event: ReasoningEvent<'_>) {
    if let Ok(json) = serde_json::to_string(&event) {
        let _ = tx.send(format!("[REASONING_EVENT] {json}")).await;
    }
}

fn tool_call_detail(tool: &ToolCall) -> String {
    match tool {
        ToolCall::Calculate(expression) => {
            format!("Evaluating `{}` exactly.", truncate_inline(expression, 80))
        }
        ToolCall::Search(query) => format!(
            "Searching the active project for `{}`.",
            truncate_inline(query, 100)
        ),
        ToolCall::ReadFile(path) => format!(
            "Reading `{}` from the active project.",
            truncate_inline(path, 100)
        ),
        ToolCall::SearchFiles(pattern, _) => format!(
            "Finding project files matching `{}`.",
            truncate_inline(pattern, 100)
        ),
        ToolCall::RunTerminal(command) => format!(
            "Running the read-only check `{}`.",
            truncate_inline(command, 100)
        ),
        ToolCall::GitStatus(_) => {
            "Inspecting the active repository state and changed files.".to_string()
        }
        ToolCall::ListDirectory(path) => format!(
            "Inspecting the project directory `{}`.",
            truncate_inline(path, 100)
        ),
        ToolCall::KnowledgeSearch(query) => format!(
            "Searching indexed local knowledge for `{}`.",
            truncate_inline(query, 100)
        ),
        ToolCall::OcrImage(path) => {
            format!("Extracting text from `{}`.", truncate_inline(path, 100))
        }
        ToolCall::DescribeImage(path) => format!(
            "Inspecting visual content in `{}`.",
            truncate_inline(path, 100)
        ),
        ToolCall::Rag(query) => format!(
            "Retrieving document passages relevant to `{}`.",
            truncate_inline(query, 100)
        ),
        ToolCall::Zotero(query) => format!(
            "Searching the research library for `{}`.",
            truncate_inline(query, 100)
        ),
        ToolCall::WriteFile(path, _) => format!(
            "Preparing a safe patch proposal for `{}`.",
            truncate_inline(path, 100)
        ),
    }
}

fn tool_result_detail(result: &ToolResult) -> String {
    if result.tool_name == "calculate" {
        return format!(
            "Calculator result: `{}`.",
            truncate_inline(&result.output, 80)
        );
    }
    format!(
        "{} returned usable context for the next decision.",
        result.tool_name.replace('_', " ")
    )
}

fn truncate_inline(value: &str, max_chars: usize) -> String {
    let compact = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.chars().count() <= max_chars {
        return compact;
    }
    let mut truncated = compact
        .chars()
        .take(max_chars.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

async fn safe_final_answer(
    inference_lock: &RwLock<Box<dyn InferenceBackend + Send + Sync>>,
    conversation: &str,
    model_name: &str,
    round: usize,
) -> Result<String> {
    let prompt = format!(
        "{conversation}\n\n\
         The previous response could not be parsed as a safe tool-routing JSON object.\n\
         Provide a concise final answer for the user now. Do not include hidden reasoning, \
         chain-of-thought, JSON, XML tags, or tool calls. Use normal Markdown prose. Only place \
         executable code in a fenced block, and never wrap headings or ordinary prose in one. \
         Typeset mathematical notation as LaTeX using `$...$` inline or `$$...$$` for a standalone \
         equation. Never put mathematical notation in a code fence.\n\n\
         [Assistant - final answer only]"
    );
    let inference = inference_lock.read().await;
    let answer = call_with_retry(&**inference, &prompt, model_name, round).await?;
    Ok(sanitize_final_answer(&answer))
}

async fn direct_reasoned_answer(
    inference_lock: &RwLock<Box<dyn InferenceBackend + Send + Sync>>,
    query: &str,
    conversation_context: &str,
    model_name: &str,
) -> Result<String> {
    let prompt = build_direct_reasoned_prompt(query, conversation_context);
    let inference = inference_lock.read().await;
    let answer = call_with_retry(&**inference, &prompt, model_name, 1).await?;
    Ok(sanitize_final_answer(&answer))
}

fn build_direct_reasoned_prompt(query: &str, conversation_context: &str) -> String {
    let formatting = response_format_instruction(query);
    let context = render_reasoning_context(conversation_context);
    format!(
        "You are AEGIS, a careful local assistant. Answer the user directly using reliable knowledge. \
         Think internally, but return only the concise final answer. Do not claim to have searched files, \
         called tools, or verified external facts. Use the supplied conversation context to resolve references \
         and maintain continuity. Treat persistent memories as user-provided facts or preferences, while the \
         latest request always takes priority. {formatting} If necessary information is missing, say what is missing.\n\n\
         {context}\
         User request:\n{query}\n\n[Assistant - final answer only]"
    )
}

fn render_reasoning_context(conversation_context: &str) -> String {
    let context = conversation_context.trim();
    if context.is_empty() {
        String::new()
    } else {
        format!("Conversation context:\n{context}\n\n")
    }
}

fn build_reasoning_conversation(
    system_prompt: &str,
    conversation_context: &str,
    query: &str,
) -> String {
    format!(
        "[System]\n{system_prompt}\n\n{}[User]\n{query}",
        render_reasoning_context(conversation_context)
    )
}

fn response_format_instruction(query: &str) -> &'static str {
    let lower = query.to_ascii_lowercase();
    let requests_code = contains_any(
        &lower,
        &[
            "write code",
            "show me code",
            "code example",
            "create a function",
            "create me a function",
            "write a function",
            "implement a function",
            "python function",
            "javascript function",
            "typescript function",
            "rust function",
            "python script",
            "shell script",
        ],
    );
    let requests_math = contains_any(
        &lower,
        &[
            "calculate",
            "compute",
            "derivative",
            "differentiate",
            "integral",
            "integrate",
            "equation",
            "formula",
            "fraction",
            "square root",
            "matrix",
            "determinant",
            "probability",
            "simplify",
            "solve for",
        ],
    );
    if requests_code {
        "The user requested executable code, so put that code in a fenced Markdown block with the correct language tag. Keep any explanation outside the fence."
    } else if requests_math {
        "Typeset mathematical notation as valid LaTeX. Use `$...$` for inline expressions and `$$...$$` for standalone equations or derivations. Keep explanatory prose outside the delimiters, do not escape the delimiter dollar signs, and never put math in a code fence."
    } else {
        "Use normal Markdown prose. Do not use a code fence unless executable code is genuinely required; never wrap headings, greetings, quotations, or ordinary prose in a code block."
    }
}

fn sanitize_final_answer(raw: &str) -> String {
    let without_think = strip_tag_blocks(raw, "think");
    let trimmed = without_think.trim();
    if trimmed.is_empty() {
        return "I could not safely produce a final answer from the reasoning loop.".to_string();
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&extract_json(trimmed)) {
        if let Some(answer) = value.get("answer").and_then(|v| v.as_str()) {
            let answer = strip_tag_blocks(answer, "think").trim().to_string();
            if !answer.is_empty() {
                return answer;
            }
        }
    }

    trimmed.to_string()
}

fn strip_tag_blocks(input: &str, tag: &str) -> String {
    let mut remaining = input;
    let mut output = String::new();
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    loop {
        let lower = remaining.to_lowercase();
        let Some(start) = lower.find(&open) else {
            output.push_str(remaining);
            break;
        };
        output.push_str(&remaining[..start]);
        let after_open = start + open.len();
        let lower_after = remaining[after_open..].to_lowercase();
        let Some(end_rel) = lower_after.find(&close) else {
            break;
        };
        remaining = &remaining[after_open + end_rel + close.len()..];
    }
    output
}

/// Call inference with one automatic retry on transient failure.
async fn call_with_retry(
    inference: &dyn InferenceBackend,
    prompt: &str,
    model: &str,
    round: usize,
) -> Result<String> {
    let first = inference.call(prompt, model).await;
    match first {
        Ok(v) => return Ok(v),
        Err(e) => {
            tracing::warn!("ReAct round {round} inference failed (will retry once): {e}");
            // Short backoff, then retry.
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            inference
                .call(prompt, model)
                .await
                .with_context(|| format!("ReAct round {round} inference failed after retry"))
        }
    }
}

// ── tool execution ────────────────────────────────────────────────────

async fn execute_tool(
    tool: &ToolCall,
    tool_registry: &ToolRegistry,
    rag_client: &RagClient,
    session_id: &str,
    project_path: Option<&str>,
    tx: &mpsc::Sender<String>,
) -> Result<ToolResult> {
    match tool {
        ToolCall::Search(query) => {
            let output = match tool_registry.execute_code_search(query, project_path).await {
                Ok(output) if !output.trim().is_empty() => output,
                Ok(_) => search_project_locally(query, project_path).await?,
                Err(error) => {
                    tracing::warn!(error = %error, "Semble search unavailable; using native project search");
                    search_project_locally(query, project_path).await?
                }
            };
            Ok(ToolResult {
                tool_name: "search".into(),
                input: query.clone(),
                output,
            })
        }
        ToolCall::ReadFile(path) => {
            let safe_path = validate_read_path(path, project_path)?;
            let output = match tokio::fs::read_to_string(&safe_path).await {
                Ok(text) => truncate(&text, MAX_FILE_CHARS),
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    format!("`{path}` appears to be a binary file and cannot be read as text.")
                }
                Err(e) => format!("Could not read `{path}`: {e}"),
            };
            Ok(ToolResult {
                tool_name: "read_file".into(),
                input: path.clone(),
                output,
            })
        }
        ToolCall::WriteFile(path, content) => {
            let size = content.len();
            Ok(ToolResult {
                tool_name: "write_file".into(),
                input: format!("{path} ({size} bytes)"),
                output: "Write rejected: automatic reasoning runs are read-only. \
                         Describe the intended patch in the final answer so the user can review and apply it."
                    .to_string(),
            })
        }
        ToolCall::SearchFiles(pattern, dir) => {
            let output = search_files_locally(pattern, dir.as_deref(), project_path).await;
            Ok(ToolResult {
                tool_name: "search_files".into(),
                input: match dir {
                    Some(d) => format!("{pattern} in {d}"),
                    None => pattern.clone(),
                },
                output,
            })
        }
        ToolCall::Calculate(expr) => {
            let output = calculate_expression(expr)?;
            Ok(ToolResult {
                tool_name: "calculate".into(),
                input: expr.clone(),
                output,
            })
        }
        ToolCall::KnowledgeSearch(query) => {
            let output = search_knowledge_base(query, rag_client, session_id).await;
            Ok(ToolResult {
                tool_name: "search_knowledge".into(),
                input: query.clone(),
                output,
            })
        }
        ToolCall::Rag(query) => {
            let outcome = rag_client.retrieve(query, 5, 0.0, session_id).await;
            let output = match outcome {
                Ok(o) if !o.chunks.is_empty() => o
                    .chunks
                    .into_iter()
                    .map(|c| format!("---\n{}", c.text))
                    .collect::<Vec<_>>()
                    .join("\n"),
                Ok(_) => "No relevant documents found.".to_string(),
                Err(e) => format!("RAG retrieval failed: {e}"),
            };
            Ok(ToolResult {
                tool_name: "rag".into(),
                input: query.clone(),
                output,
            })
        }
        ToolCall::RunTerminal(command) => {
            let output = run_terminal_command(command, project_path).await;
            Ok(ToolResult {
                tool_name: "run_terminal".into(),
                input: command.clone(),
                output,
            })
        }
        ToolCall::GitStatus(path) => {
            let output = git_status_command(path, project_path).await;
            Ok(ToolResult {
                tool_name: "git_status".into(),
                input: path.clone(),
                output,
            })
        }
        ToolCall::ListDirectory(path) => {
            let output = list_directory_command(path, project_path).await;
            Ok(ToolResult {
                tool_name: "list_directory".into(),
                input: path.clone(),
                output,
            })
        }
        ToolCall::OcrImage(path) => {
            let output = ocr_image_command(path, &tx).await;
            Ok(ToolResult {
                tool_name: "ocr_image".into(),
                input: path.clone(),
                output,
            })
        }
        ToolCall::DescribeImage(path) => {
            let output = describe_image_command(path, &tx).await;
            Ok(ToolResult {
                tool_name: "describe_image".into(),
                input: path.clone(),
                output,
            })
        }
        ToolCall::Zotero(query) => {
            let output = tool_registry
                .execute("zotero", query)
                .await
                .unwrap_or_else(|e| format!("Zotero search failed: {e}"));
            Ok(ToolResult {
                tool_name: "zotero".into(),
                input: query.clone(),
                output,
            })
        }
    }
}

async fn search_project_locally(query: &str, project_path: Option<&str>) -> Result<String> {
    let root = project_path
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Code search requires an active project."))?
        .to_string();
    let query = query.to_string();
    tokio::task::spawn_blocking(move || {
        let root = PathBuf::from(&root).canonicalize().with_context(|| {
            format!("Could not access the active project `{root}` for local search")
        })?;
        let terms = query
            .split(|character: char| !character.is_alphanumeric() && character != '_')
            .map(str::to_ascii_lowercase)
            .filter(|term| term.len() >= 3)
            .filter(|term| {
                !matches!(
                    term.as_str(),
                    "the" | "and" | "for" | "with" | "this" | "that" | "from" | "where"
                )
            })
            .collect::<Vec<_>>();
        if terms.is_empty() {
            anyhow::bail!("Code search needs a more specific query.");
        }

        let mut stack = vec![root.clone()];
        let mut matches = Vec::<(usize, String)>::new();
        let mut scanned = 0usize;
        while let Some(directory) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .unwrap_or("");
                    if !matches!(
                        name,
                        ".git" | "node_modules" | "target" | "dist" | "build" | ".venv"
                    ) && !name.starts_with('.')
                    {
                        stack.push(path);
                    }
                    continue;
                }
                if scanned >= 20_000 || !is_local_search_file(&path) {
                    continue;
                }
                scanned += 1;
                let Ok(metadata) = path.metadata() else {
                    continue;
                };
                if metadata.len() > 1_000_000 {
                    continue;
                }
                let Ok(content) = std::fs::read_to_string(&path) else {
                    continue;
                };
                for (index, line) in content.lines().enumerate() {
                    let lower = line.to_ascii_lowercase();
                    let score = terms
                        .iter()
                        .filter(|term| lower.contains(term.as_str()))
                        .count();
                    if score > 0 {
                        let relative = path.strip_prefix(&root).unwrap_or(&path);
                        matches.push((
                            score,
                            format!("{}:{}  {}", relative.display(), index + 1, line.trim()),
                        ));
                    }
                }
            }
        }
        matches.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.cmp(&right.1)));
        matches.dedup_by(|left, right| left.1 == right.1);
        if matches.is_empty() {
            return Ok(format!(
                "No local project matches were found for `{query}`."
            ));
        }
        Ok(matches
            .into_iter()
            .take(30)
            .map(|(_, value)| value)
            .collect::<Vec<_>>()
            .join("\n"))
    })
    .await
    .map_err(|error| anyhow::anyhow!("Local code search task failed: {error}"))?
}

fn is_local_search_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            [
                "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "cs", "c", "h", "cpp",
                "hpp", "rb", "php", "swift", "vue", "svelte", "html", "css", "scss", "json",
                "toml", "yaml", "yml", "md", "txt", "sql", "sh", "ps1",
            ]
            .iter()
            .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

/// Path-sanitisation guard for `read_file`.
///
/// Resolves the path to its canonical form and rejects it if it does
/// not reside inside an allowed directory.  Currently allows reading
/// files under the current working directory.  Extend `ALLOWED_ROOTS`
/// if the tool should cover wider filesystem access.
fn validate_read_path(requested: &str, project_path: Option<&str>) -> Result<std::path::PathBuf> {
    let cwd = match project_path {
        Some(path) if !path.trim().is_empty() => PathBuf::from(path),
        _ => std::env::current_dir().context("Could not determine current working directory")?,
    };
    let cwd = cwd
        .canonicalize()
        .context("Could not canonicalize current working directory")?;

    let candidate = Path::new(requested);
    // If the path is relative, resolve it relative to cwd first.
    let candidate = if candidate.is_relative() {
        cwd.join(candidate)
    } else {
        candidate.to_path_buf()
    };

    if candidate.is_absolute() && !candidate.starts_with(&cwd) {
        anyhow::bail!(
            "Access denied: `{requested}` resolves outside the \
             allowed directory (`{}`)",
            cwd.display()
        );
    }

    let canonical = candidate
        .canonicalize()
        .with_context(|| format!("Path does not exist or is inaccessible: `{requested}`"))?;

    // Reject paths that escape the project root.
    if !canonical.starts_with(&cwd) {
        anyhow::bail!(
            "Access denied: `{requested}` resolves outside the \
             allowed directory (`{}`)",
            cwd.display()
        );
    }

    Ok(canonical)
}

/// Execute a shell command with a timeout.
///
/// Uses `sh -c` on Unix and `cmd /c` on Windows so pipes, redirects,
/// chaining (`&&`, `||`) work as expected.
///
/// Blocks for a maximum of `TERMINAL_TIMEOUT_SECS` — the ReAct loop
/// would deadlock if a command hangs forever.
async fn run_terminal_command(command: &str, project_path: Option<&str>) -> String {
    // Safety guard: block destructive operations with robust matching.
    // Normalise the command so simple obfuscations don't bypass the check.
    let normalised = command
        .to_lowercase()
        .replace('\n', " ")
        .replace('\t', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let normalised = normalised.trim();

    // Deny-list of known-dangerous binary names (first word of command).
    let first_word = normalised.split_whitespace().next().unwrap_or("");
    let dangerous_binaries = [
        "rm",
        "dd",
        "mkfs",
        "mkfs.ext4",
        "mkfs.ext3",
        "mkfs.ext2",
        "mkfs.xfs",
        "mkfs.btrfs",
        "mkfs.fat",
        "fdisk",
        "parted",
        "shred",
        "wipefs",
        "blockdev",
        "hdparm",
    ];
    if dangerous_binaries.contains(&first_word) {
        // Whitelist safe uses of `rm`: only rm with non-root paths.
        if first_word == "rm" {
            let args: Vec<&str> = normalised.split_whitespace().skip(1).collect();
            let has_glob_root = args.iter().any(|a| {
                *a == "/"
                    || *a == "/*"
                    || a.starts_with("/dev/")
                    || *a == "--no-preserve-root"
                    || a.starts_with("/sys/")
                    || a.starts_with("/proc/")
            });
            if !has_glob_root {
                // Allowed: rm on project-local files.
            } else {
                return "Command rejected: recursive deletion of system paths is \
                        not allowed through the terminal tool. \
                        Run it manually if needed."
                    .to_string();
            }
        } else {
            return format!(
                "Command rejected: `{first_word}` is a dangerous system utility. \
                 Run it manually if needed."
            )
            .to_string();
        }
    }

    // Check for destructive redirects to block devices.
    let lower = command.to_lowercase();
    let destructive_redirects = [
        "> /dev/sd",
        "> /dev/nvme",
        "> /dev/vd",
        "> /dev/mmcblk",
        "> /dev/sda",
        "> /dev/sdb",
        "> /dev/sdc",
        "of=/dev/sd",
        "of=/dev/nvme",
        "of=/dev/vd",
    ];
    if destructive_redirects.iter().any(|p| lower.contains(p)) {
        return "Command rejected: writing directly to block devices is not \
                allowed through the terminal tool. Run it manually if needed."
            .to_string();
    }

    if is_mutating_terminal_command(normalised, &lower) {
        return "Command rejected: automatic reasoning terminal commands are read-only. \
                Ask the user to approve file changes or provide a patch in the final answer."
            .to_string();
    }

    // Determine the shell command.
    let (shell, flag) = if cfg!(windows) {
        ("cmd.exe", "/C")
    } else {
        ("sh", "-c")
    };

    let working_directory = match project_path {
        Some(path) if !path.trim().is_empty() => match Path::new(path).canonicalize() {
            Ok(directory) if directory.is_dir() => Some(directory),
            Ok(_) => return format!("Command rejected: project path `{path}` is not a directory."),
            Err(error) => {
                return format!("Command rejected: project path `{path}` is unavailable: {error}");
            }
        },
        _ => None,
    };
    let mut process = tokio::process::Command::new(shell);
    process.arg(flag).arg(command);
    if let Some(directory) = working_directory {
        process.current_dir(directory);
    }
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(TERMINAL_TIMEOUT_SECS),
        process.output(),
    )
    .await;

    match result {
        Ok(Ok(output)) => {
            let mut text = String::new();

            if !output.stdout.is_empty() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                text.push_str(&stdout);
            }
            if !output.stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                let stderr = String::from_utf8_lossy(&output.stderr);
                text.push_str(&stderr);
            }

            if let Some(code) = output.status.code() {
                text.push_str(&format!("\n[Exit code: {code}]"));
            } else {
                text.push_str("\n[Process terminated by signal]");
            }

            truncate(&text, MAX_TERMINAL_OUTPUT_CHARS)
        }
        Ok(Err(e)) => {
            format!("Failed to run command: {e}")
        }
        Err(_) => {
            format!(
                "Command timed out after {TERMINAL_TIMEOUT_SECS} seconds. \
                 Try a simpler or faster command."
            )
        }
    }
}

fn is_mutating_terminal_command(normalised: &str, lower: &str) -> bool {
    let mutating_tokens = [
        ">",
        ">>",
        "del ",
        "erase ",
        "move ",
        "copy ",
        "xcopy ",
        "robocopy ",
        "ren ",
        "rename ",
        "rmdir ",
        "mkdir ",
        "md ",
        "touch ",
        "tee ",
        "mv ",
        "cp ",
        "install ",
        "set-content",
        "add-content",
        "out-file",
        "new-item",
        "remove-item",
        "move-item",
        "copy-item",
        "rename-item",
    ];
    let command_with_padding = format!(" {normalised} ");
    if mutating_tokens
        .iter()
        .any(|token| command_with_padding.contains(token))
    {
        return true;
    }

    lower.contains(" -out ")
        || lower.contains(" --out ")
        || lower.contains(" --output ")
        || lower.contains(" -output ")
}

/// Recursively search for files matching a glob-like pattern.
///
/// Uses simple substring matching (not full glob) so no extra
/// dependencies are needed.  Searches up to `MAX_SEARCH_RESULTS`
/// files and returns a tree-style listing.
async fn search_files_locally(
    pattern: &str,
    dir: Option<&str>,
    project_path: Option<&str>,
) -> String {
    if pattern.trim().is_empty() {
        return "Error: search pattern cannot be empty.".to_string();
    }

    let requested = dir.unwrap_or(".");
    let root = match validate_read_path(requested, project_path) {
        Ok(path) if path.is_dir() => path,
        Ok(_) => return format!("`{requested}` is not a directory."),
        Err(error) => return format!("Could not access directory `{requested}`: {error}"),
    };

    let lower_pattern = pattern.to_lowercase();
    let mut results = Vec::new();
    // Iterative stack-based walk — no recursion, safe for deep trees.
    let mut stack = vec![(root.clone(), 0usize)];

    while let Some((dir, depth)) = stack.pop() {
        if results.len() >= MAX_SEARCH_RESULTS {
            break;
        }
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
            if results.len() >= MAX_SEARCH_RESULTS {
                break;
            }
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                // Skip hidden dirs, generated dirs.
                if !name.starts_with('.')
                    && name != "node_modules"
                    && name != "target"
                    && name != ".git"
                {
                    stack.push((path, depth + 1));
                }
            } else if name.contains(&lower_pattern) {
                let rel = path.strip_prefix(&root).unwrap_or(&path);
                results.push((rel.to_path_buf(), depth));
            }
        }
    }

    if results.is_empty() {
        return format!(
            "No files matching `{pattern}` found under `{}`.",
            root.display()
        );
    }

    let mut output = String::new();
    for (path, depth) in &results {
        let indent = "  ".repeat(*depth);
        output.push_str(&format!("{indent}{}\n", path.display()));
    }
    output.push_str(&format!(
        "\n[Found {} file(s) matching `{pattern}`]",
        results.len()
    ));
    output
}

/// Run several git commands in a directory and aggregate the output.
async fn git_status_command(path: &str, project_path: Option<&str>) -> String {
    let dir = match validate_read_path(path, project_path) {
        Ok(p) => p,
        Err(e) => return format!("Could not access `{path}`: {e}"),
    };

    // Check if the directory is a git repository.
    let git_dir = dir.join(".git");
    if !git_dir.exists() {
        return format!("`{path}` is not a git repository (no .git directory found).");
    }

    let commands = [
        ("Branch", vec!["branch", "--show-current"]),
        (
            "Recent commits",
            vec!["log", "--oneline", "-10", "--no-decorate"],
        ),
        ("Status", vec!["status", "--short"]),
        ("Unstaged diff", vec!["diff", "--stat"]),
        ("Staged diff", vec!["diff", "--cached", "--stat"]),
    ];

    let mut output = String::new();
    for (label, args) in &commands {
        let cmd_output = tokio::time::timeout(
            std::time::Duration::from_secs(GIT_TIMEOUT_SECS),
            tokio::process::Command::new("git")
                .args(args)
                .current_dir(&dir)
                .output(),
        )
        .await;

        match cmd_output {
            Ok(Ok(result)) => {
                let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
                if !stdout.is_empty() || !stderr.is_empty() {
                    output.push_str(&format!("{label}:\n"));
                    if !stdout.is_empty() {
                        output.push_str(&stdout);
                        output.push('\n');
                    }
                    if !stderr.is_empty() {
                        output.push_str(&format!("  (stderr) {stderr}\n"));
                    }
                    output.push('\n');
                }
            }
            Ok(Err(e)) => {
                output.push_str(&format!("{label}: error — {e}\n\n"));
            }
            Err(_) => {
                output.push_str(&format!("{label}: timed out after {GIT_TIMEOUT_SECS}s\n\n"));
            }
        }
    }

    if output.is_empty() {
        output.push_str("Git status: clean — no changes detected.");
    }

    truncate(&output, MAX_TOOL_OUTPUT_CHARS)
}

/// List files and directories in a path, with size and type info.
async fn list_directory_command(path: &str, project_path: Option<&str>) -> String {
    let dir = match validate_read_path(path, project_path) {
        Ok(p) => p,
        Err(e) => return format!("Could not access `{path}`: {e}"),
    };

    let mut entries = match tokio::fs::read_dir(&dir).await {
        Ok(e) => e,
        Err(e) => return format!("Could not read directory `{path}`: {e}"),
    };

    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut count = 0usize;

    while let Some(entry) = entries.next_entry().await.unwrap_or(None) {
        if count >= LIST_DIR_MAX_ENTRIES {
            break;
        }
        count += 1;

        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden entries.
        if name.starts_with('.') || name == "node_modules" || name == "target" {
            continue;
        }

        let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
        let metadata = entry.metadata().await;

        if is_dir {
            dirs.push((name, None::<Vec<String>>));
        } else {
            let size = metadata
                .ok()
                .map(|m| m.len())
                .map(format_file_size)
                .unwrap_or_else(|| "?".to_string());
            files.push((name, size));
        }
    }

    dirs.sort_by(|(a, _), (b, _)| a.cmp(b));
    files.sort_by(|(a, _), (b, _)| a.cmp(b));

    let mut output = format!("Directory listing for `{}`:\n\n", dir.display());

    if !dirs.is_empty() {
        output.push_str("Directories:\n");
        for (name, _) in &dirs {
            output.push_str(&format!("  📁 {name}/\n"));
        }
        output.push('\n');
    }

    if !files.is_empty() {
        output.push_str("Files:\n");
        for (name, size) in &files {
            output.push_str(&format!("  📄 {name:<30} {size}\n"));
        }
        output.push('\n');
    }

    if count >= LIST_DIR_MAX_ENTRIES {
        output.push_str(&format!(
            "[Note: Directory listing limited to first {LIST_DIR_MAX_ENTRIES} entries]\n"
        ));
    }

    output.push_str(&format!(
        "[{} directories, {} files]\n",
        dirs.len(),
        files.len()
    ));

    output
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Extract text from an image using OCR (Tesseract via Python service).
/// Sends the image as base64 in a JSON payload so no filesystem path is
/// shared between the Rust engine and Python service.
async fn ocr_image_command(path: &str, tx: &mpsc::Sender<String>) -> String {
    let safe_path = match validate_read_path(path, None) {
        Ok(p) => p,
        Err(e) => return format!("Could not access `{path}`: {e}"),
    };

    let _ = tx.send("\n[REACT: reading image file...]\n".into()).await;

    let image_bytes = match tokio::fs::read(&safe_path).await {
        Ok(b) => {
            if b.len() as u64 > MAX_IMAGE_BYTES {
                return format!(
                    "Image too large ({} bytes). Maximum is {} MB. Compress or resize first.",
                    b.len(),
                    MAX_IMAGE_BYTES / 1_000_000
                );
            }
            b
        }
        Err(e) => return format!("Could not read `{path}`: {e}"),
    };

    let _ = tx
        .send("\n[REACT: sending to OCR service...]\n".into())
        .await;

    let b64 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&image_bytes)
    };

    let body = serde_json::json!({ "image": b64, "filename": path });

    let resp = SHARED_HTTP_CLIENT
        .post("http://127.0.0.1:8000/ocr")
        .json(&body)
        .timeout(std::time::Duration::from_secs(60))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
            Ok(json) => {
                let text = json["text"].as_str().unwrap_or("(no text returned)");
                if text.trim().is_empty() {
                    "OCR returned no text. The image may be blank or contain \
                         only non-text content (charts, diagrams, photos)."
                        .to_string()
                } else {
                    format!("OCR Result:\n{text}")
                }
            }
            Err(e) => format!("OCR service returned invalid response: {e}"),
        },
        Ok(r) => {
            format!("OCR service returned error (HTTP {})", r.status())
        }
        Err(e) => {
            format!(
                "Could not reach OCR service at localhost:8000. \
                 Make sure the Python OCR/RAG service is running: {e}"
            )
        }
    }
}

/// Generate a natural-language description of an image using a vision model.
///
/// Calls Ollama's `/api/chat` endpoint.  The vision model is read from the
/// `VISION_MODEL` environment variable (default: `llava`).  The image is
/// base64-encoded and sent inline so no file-system access is needed.
async fn describe_image_command(path: &str, tx: &mpsc::Sender<String>) -> String {
    let safe_path = match validate_read_path(path, None) {
        Ok(p) => p,
        Err(e) => return format!("Could not access `{path}`: {e}"),
    };

    let _ = tx.send("\n[REACT: reading image file...]\n".into()).await;

    let image_bytes = match tokio::fs::read(&safe_path).await {
        Ok(b) => {
            if b.len() as u64 > MAX_IMAGE_BYTES {
                return format!(
                    "Image too large ({} bytes). Maximum is {} MB. Compress or resize first.",
                    b.len(),
                    MAX_IMAGE_BYTES / 1_000_000
                );
            }
            b
        }
        Err(e) => return format!("Could not read `{path}`: {e}"),
    };

    let _ = tx
        .send("\n[REACT: sending to vision model...]\n".into())
        .await;

    let b64 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&image_bytes)
    };
    let mime = infer_mime_type(path);

    let vision_prompt = "Describe this image in detail. Include any visible text, \
                         UI elements, diagrams, charts, or code. Be thorough.";

    let vision_model = std::env::var("VISION_MODEL").unwrap_or_else(|_| "llava".to_string());

    let body = serde_json::json!({
        "model": vision_model,
        "messages": [{
            "role": "user",
            "content": vision_prompt,
            "images": [format!("data:{mime};base64,{b64}")]
        }],
        "stream": false,
        "options": { "num_predict": 512 }
    });

    let ollama_url =
        std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let resp = SHARED_HTTP_CLIENT
        .post(format!("{ollama_url}/api/chat"))
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await;

    match resp {
        Ok(r) if r.status().is_success() => match r.json::<serde_json::Value>().await {
            Ok(json) => {
                let text = json
                    .get("message")
                    .and_then(|m| m.get("content"))
                    .and_then(|c| c.as_str())
                    .unwrap_or("(no description returned)");
                format!("Image Description:\n{text}")
            }
            Err(e) => format!("Vision model returned invalid response: {e}"),
        },
        Ok(r) if r.status() == 404 => {
            format!(
                "Vision model `{vision_model}` not found. Run `ollama pull {vision_model}` or \
                 set VISION_MODEL environment variable to an installed vision model. \
                 Available models: llava, moondream, qwen2.5-vl, llama3.2-vision, minicpm-v"
            )
        }
        Ok(r) => format!("Vision model returned error (HTTP {})", r.status()),
        Err(e) => {
            format!(
                "Could not reach Ollama at {ollama_url}. Make sure Ollama \
                 is running and a vision model is installed: {e}"
            )
        }
    }
}

/// Infer the MIME type from a file extension for base64 encoding.
fn infer_mime_type(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".bmp") {
        "image/bmp"
    } else {
        "image/png" // safe default
    }
}

fn calculate_expression(expr: &str) -> Result<String> {
    struct Parser<'a> {
        chars: std::iter::Peekable<std::str::Chars<'a>>,
    }

    impl<'a> Parser<'a> {
        fn skip_ws(&mut self) {
            while matches!(self.chars.peek(), Some(ch) if ch.is_whitespace()) {
                self.chars.next();
            }
        }

        fn parse_number(&mut self) -> Result<f64> {
            self.skip_ws();
            let mut buf = String::new();
            while let Some(ch) = self.chars.peek().copied() {
                if ch.is_ascii_digit() || ch == '.' {
                    buf.push(ch);
                    self.chars.next();
                } else {
                    break;
                }
            }
            if matches!(self.chars.peek(), Some('e') | Some('E')) {
                buf.push(self.chars.next().unwrap());
                if matches!(self.chars.peek(), Some('+') | Some('-')) {
                    buf.push(self.chars.next().unwrap());
                }
                while matches!(self.chars.peek(), Some(ch) if ch.is_ascii_digit()) {
                    buf.push(self.chars.next().unwrap());
                }
            }
            if buf.is_empty() {
                anyhow::bail!("expected a number");
            }
            Ok(buf
                .parse::<f64>()
                .with_context(|| format!("could not parse number `{buf}`"))?)
        }

        fn parse_named_value(&mut self) -> Result<f64> {
            let mut name = String::new();
            while matches!(self.chars.peek(), Some(ch) if ch.is_ascii_alphanumeric()) {
                name.push(self.chars.next().unwrap().to_ascii_lowercase());
            }
            match name.as_str() {
                "pi" => Ok(std::f64::consts::PI),
                "e" => Ok(std::f64::consts::E),
                _ => {
                    self.skip_ws();
                    if self.chars.next() != Some('(') {
                        anyhow::bail!("function `{name}` requires parentheses");
                    }
                    let value = self.parse_expr()?;
                    self.skip_ws();
                    if self.chars.next() != Some(')') {
                        anyhow::bail!("missing closing `)` after `{name}`");
                    }
                    match name.as_str() {
                        "sqrt" => {
                            if value < 0.0 {
                                anyhow::bail!("square root is undefined for negative values");
                            }
                            Ok(value.sqrt())
                        }
                        "abs" => Ok(value.abs()),
                        "sin" => Ok(value.sin()),
                        "cos" => Ok(value.cos()),
                        "tan" => Ok(value.tan()),
                        "ln" => Ok(value.ln()),
                        "log" | "log10" => Ok(value.log10()),
                        _ => anyhow::bail!("unsupported function `{name}`"),
                    }
                }
            }
        }

        fn parse_primary(&mut self) -> Result<f64> {
            self.skip_ws();
            match self.chars.peek().copied() {
                Some('(') => {
                    self.chars.next();
                    let value = self.parse_expr()?;
                    self.skip_ws();
                    match self.chars.next() {
                        Some(')') => Ok(value),
                        _ => anyhow::bail!("missing closing `)`"),
                    }
                }
                Some('√') => {
                    self.chars.next();
                    let value = self.parse_primary()?;
                    if value < 0.0 {
                        anyhow::bail!("square root is undefined for negative values");
                    }
                    Ok(value.sqrt())
                }
                Some(ch) if ch.is_ascii_alphabetic() => self.parse_named_value(),
                Some(_) => self.parse_number(),
                None => anyhow::bail!("expected an expression"),
            }
        }

        fn parse_unary(&mut self) -> Result<f64> {
            self.skip_ws();
            match self.chars.peek().copied() {
                Some('+') => {
                    self.chars.next();
                    self.parse_unary()
                }
                Some('-') => {
                    self.chars.next();
                    Ok(-self.parse_unary()?)
                }
                _ => self.parse_primary(),
            }
        }

        fn parse_power(&mut self) -> Result<f64> {
            let value = self.parse_unary()?;
            self.skip_ws();
            if self.chars.peek() == Some(&'^') {
                self.chars.next();
                return Ok(value.powf(self.parse_power()?));
            }
            Ok(value)
        }

        fn parse_term(&mut self) -> Result<f64> {
            let mut value = self.parse_power()?;
            loop {
                self.skip_ws();
                let op = match self.chars.peek().copied() {
                    Some('*') => '*',
                    Some('/') => '/',
                    Some('%') => '%',
                    _ => break,
                };
                self.chars.next();
                let rhs = self.parse_power()?;
                value = match op {
                    '*' => value * rhs,
                    '/' => {
                        if rhs == 0.0 {
                            anyhow::bail!("division by zero");
                        }
                        value / rhs
                    }
                    '%' => {
                        if rhs == 0.0 {
                            anyhow::bail!("division by zero");
                        }
                        value % rhs
                    }
                    _ => unreachable!(),
                };
            }
            Ok(value)
        }

        fn parse_expr(&mut self) -> Result<f64> {
            let mut value = self.parse_term()?;
            loop {
                self.skip_ws();
                let op = match self.chars.peek().copied() {
                    Some('+') => '+',
                    Some('-') => '-',
                    _ => break,
                };
                self.chars.next();
                let rhs = self.parse_term()?;
                value = match op {
                    '+' => value + rhs,
                    '-' => value - rhs,
                    _ => unreachable!(),
                };
            }
            Ok(value)
        }
    }

    let mut parser = Parser {
        chars: expr.chars().peekable(),
    };
    let value = parser.parse_expr()?;
    parser.skip_ws();
    if parser.chars.peek().is_some() {
        anyhow::bail!("unexpected trailing input in expression");
    }
    if !value.is_finite() {
        anyhow::bail!("calculation produced a non-finite result");
    }

    Ok(if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    })
}

async fn search_knowledge_base(query: &str, rag_client: &RagClient, session_id: &str) -> String {
    match rag_client.retrieve(query, 5, 0.0, session_id).await {
        Ok(outcome) if outcome.chunks.is_empty() => {
            "No relevant knowledge chunks were found.".to_string()
        }
        Ok(outcome) => {
            let mut output = String::new();
            output.push_str(&format!(
                "RAG results: {} chunks, avg similarity {:.3}, backend {}\n",
                outcome.metrics.chunk_count,
                outcome.metrics.avg_similarity,
                outcome.metrics.backend
            ));
            for chunk in outcome.chunks {
                output.push_str(&format!(
                    "\n---\n[{}] {}{}\n{}",
                    chunk.source,
                    chunk
                        .page
                        .map(|page| format!("page {page} "))
                        .unwrap_or_default(),
                    format!("score {:.3}", chunk.score),
                    chunk.text
                ));
            }
            output
        }
        Err(error) => format!("Knowledge search failed: {error}"),
    }
}

#[derive(Debug)]
enum ModelDecision {
    Tool(ToolCall),
    FinalAnswer(String),
}

#[derive(Deserialize)]
struct ReActResponse {
    #[allow(dead_code)]
    thought: Option<String>,
    tool: Option<String>,
    arguments: Option<serde_json::Value>,
    answer: Option<String>,
}

fn parse_react_response(raw: &str) -> Result<ModelDecision> {
    let json = extract_json(raw);
    let parsed: ReActResponse = match serde_json::from_str(&json) {
        Ok(parsed) => parsed,
        Err(_) if looks_like_plain_answer(raw) => {
            return Ok(ModelDecision::FinalAnswer(sanitize_final_answer(raw)));
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to parse model JSON response:\n{raw}"));
        }
    };

    if let Some(answer) = parsed.answer {
        let answer = answer.trim().to_string();
        if !answer.is_empty() {
            return Ok(ModelDecision::FinalAnswer(answer));
        }
    }

    if let (Some(tool_name), Some(args)) = (parsed.tool, parsed.arguments) {
        let tool_call = match tool_name.as_str() {
            "search" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("search tool missing 'query' argument"))?;
                ToolCall::Search(query.to_string())
            }
            "write_file" | "write" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("write_file tool missing 'path' argument"))?;
                let content = args
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("write_file tool missing 'content' argument"))?;
                ToolCall::WriteFile(path.to_string(), content.to_string())
            }
            "search_files" | "find" => {
                let pattern = args
                    .get("pattern")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("search_files tool missing 'pattern' argument")
                    })?;
                let dir = args
                    .get("dir")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                ToolCall::SearchFiles(pattern.to_string(), dir)
            }
            "calculate" | "calc" | "math" => {
                let expr = args
                    .get("expression")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("calculate tool missing 'expression' argument")
                    })?;
                ToolCall::Calculate(expr.to_string())
            }
            "git_status" | "git" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("git_status tool missing 'path' argument"))?;
                ToolCall::GitStatus(path.to_string())
            }
            "list_directory" | "ls" => {
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                    anyhow::anyhow!("list_directory tool missing 'path' argument")
                })?;
                ToolCall::ListDirectory(path.to_string())
            }
            "search_knowledge" | "knowledge" => {
                let query = args.get("query").and_then(|v| v.as_str()).ok_or_else(|| {
                    anyhow::anyhow!("search_knowledge tool missing 'query' argument")
                })?;
                ToolCall::KnowledgeSearch(query.to_string())
            }
            "read_file" | "read" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("read_file tool missing 'path' argument"))?;
                ToolCall::ReadFile(path.to_string())
            }
            "rag" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("rag tool missing 'query' argument"))?;
                ToolCall::Rag(query.to_string())
            }
            "run_terminal" | "terminal" => {
                let cmd = args
                    .get("command")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        anyhow::anyhow!("run_terminal tool missing 'command' argument")
                    })?;
                ToolCall::RunTerminal(cmd.to_string())
            }
            "ocr_image" | "ocr" => {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("ocr_image tool missing 'path' argument"))?;
                ToolCall::OcrImage(path.to_string())
            }
            "describe_image" | "vision" | "describe" => {
                let path = args.get("path").and_then(|v| v.as_str()).ok_or_else(|| {
                    anyhow::anyhow!("describe_image tool missing 'path' argument")
                })?;
                ToolCall::DescribeImage(path.to_string())
            }
            "zotero" => {
                let query = args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("zotero tool missing 'query' argument"))?;
                ToolCall::Zotero(query.to_string())
            }
            other => {
                anyhow::bail!(
                    "Unknown tool `{other}`. Valid tools: search, read_file, search_files, run_terminal, git_status, list_directory, calculate, search_knowledge, ocr_image, describe_image, rag, zotero"
                )
            }
        };
        return Ok(ModelDecision::Tool(tool_call));
    }

    anyhow::bail!("Model response did not contain a tool call or final answer.\nResponse:\n{raw}")
}

fn looks_like_plain_answer(raw: &str) -> bool {
    let trimmed = raw.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with('{')
        && !trimmed.starts_with("```json")
        && !trimmed.contains("\"tool\"")
        && !trimmed.contains("\"arguments\"")
}

fn extract_json(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.starts_with("```") {
        let without_fence = trimmed
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();
        if !without_fence.is_empty() {
            return without_fence.to_string();
        }
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}')) {
        return trimmed[start..=end].to_string();
    }
    trimmed.to_string()
}

// ── prompt building ───────────────────────────────────────────────────

fn build_system_prompt(plan: &ToolPlan) -> String {
    let mut tools = String::new();
    for tool in &plan.allowed {
        let description = match *tool {
            "search" => "search(query): Search code content in the active project.",
            "read_file" => "read_file(path): Read one project-relative text file.",
            "search_files" => "search_files(pattern, dir): Find project files by name.",
            "run_terminal" => {
                "run_terminal(command): Run one read-only build, test, or inspection command."
            }
            "git_status" => "git_status(path): Inspect branch, status, and diff summaries.",
            "list_directory" => "list_directory(path): List one project-relative directory.",
            "calculate" => "calculate(expression): Evaluate an arithmetic expression.",
            "search_knowledge" => {
                "search_knowledge(query): Search indexed user knowledge and prior context."
            }
            "zotero" => "zotero(query): Search the user's Zotero research library.",
            _ => continue,
        };
        tools.push_str("- ");
        tools.push_str(description);
        tools.push('\n');
    }

    format!(
        "You are AEGIS, a careful local assistant. A fast router selected only the tools relevant \
         to this request. This route requires evidence from at least one selected tool before the \
         final answer. Never claim a result that the tool did not return.\n\nALLOWED TOOLS\n{tools}\n\
         RESPONSE FORMAT\nReturn exactly one JSON object. To answer: \
         {{\"answer\":\"concise Markdown final answer\"}}. Only executable code requested by the user \
         belongs in a language-tagged fence; headings and ordinary prose must stay outside fences. \
         Typeset mathematical notation as LaTeX using `$...$` inline or `$$...$$` for standalone \
         equations, never in code fences. To call one allowed tool: \
         {{\"tool\":\"tool_name\",\"arguments\":{{...}}}}.\n\nRULES\n\
         - Call at most one tool per round and only when its result is necessary for correctness.\n\
         - You must call one allowed tool before returning the final answer.\n\
         - Do not repeat or broaden searches without clear missing evidence.\n\
         - Use project-relative paths only.\n\
         - After a useful result, prefer answering over another tool call.\n\
         - Never output chain-of-thought or text outside the JSON object."
    )
}

fn required_tool_fallback(plan: &ToolPlan, query: &str) -> Option<ToolCall> {
    if plan.allowed.contains(&"calculate") {
        return calculation_call_from_query(query);
    }
    if plan.allowed.contains(&"search_knowledge") {
        return Some(ToolCall::KnowledgeSearch(query.to_string()));
    }
    if plan.allowed.contains(&"zotero") {
        return Some(ToolCall::Zotero(query.to_string()));
    }
    if plan.allowed.contains(&"search") {
        return Some(ToolCall::Search(query.to_string()));
    }
    if plan.allowed.contains(&"read_file") {
        return extract_project_file_path(query).map(ToolCall::ReadFile);
    }
    None
}

fn calculation_call_from_query(query: &str) -> Option<ToolCall> {
    let lower = query.to_ascii_lowercase();
    let numbers = query
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .filter(|part| !part.is_empty() && part.parse::<f64>().is_ok())
        .collect::<Vec<_>>();
    if lower.contains("square root") {
        return numbers
            .first()
            .map(|number| ToolCall::Calculate(format!("sqrt({number})")));
    }
    if lower.contains("percentage of") && numbers.len() >= 2 {
        return Some(ToolCall::Calculate(format!(
            "{} / 100 * {}",
            numbers[0], numbers[1]
        )));
    }
    None
}

fn extract_project_file_path(query: &str) -> Option<String> {
    query
        .split_whitespace()
        .map(|part| {
            part.trim_matches(|character: char| {
                matches!(character, '`' | '\'' | '"' | ',' | ':' | ';' | '(' | ')')
            })
        })
        .find(|part| {
            contains_any(
                &part.to_ascii_lowercase(),
                &[".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java"],
            )
        })
        .map(str::to_string)
}

fn select_tool_plan(
    query: &str,
    workflow: WorkflowId,
    mode: &str,
    has_attachments: bool,
    has_project: bool,
) -> ToolPlan {
    let lower = query.to_ascii_lowercase();
    let code_request = mode.eq_ignore_ascii_case("coder")
        || matches!(
            workflow,
            WorkflowId::CodeExplain | WorkflowId::CodeGenerate | WorkflowId::CodeDebug
        );
    if code_request && has_project && needs_project_evidence(&lower) {
        let explicit_path = contains_any(
            &lower,
            &["src/", "src\\", ".rs", ".ts", ".tsx", ".js", ".py"],
        );
        let mut allowed = vec!["read_file"];
        if !explicit_path {
            allowed.push("search");
        }
        if contains_any(&lower, &["find ", "where is", "locate ", "file named"]) {
            allowed.push("search_files");
        }
        if contains_any(
            &lower,
            &[
                "run test",
                "failing test",
                "build error",
                "compile error",
                "cargo test",
                "npm test",
            ],
        ) {
            allowed.push("run_terminal");
        }
        if contains_any(
            &lower,
            &["git status", "git diff", "working tree", "changed files"],
        ) {
            allowed.push("git_status");
        }
        if contains_any(
            &lower,
            &["project structure", "folder", "directory", "list files"],
        ) {
            allowed.push("list_directory");
        }
        return ToolPlan {
            allowed,
            max_calls: 3,
            rationale: "This request targets the active code project, so AEGIS may use a small set of read-only project tools.",
        };
    }
    if has_attachments {
        return ToolPlan {
            allowed: Vec::new(),
            max_calls: 0,
            rationale: "Relevant document passages were already retrieved before reasoning, so another search is unnecessary.",
        };
    }
    if contains_any(
        &lower,
        &[
            "zotero",
            "my research library",
            "my zotero",
            "papers in my library",
        ],
    ) {
        return ToolPlan {
            allowed: vec!["zotero"],
            max_calls: 1,
            rationale: "The request explicitly refers to the user's research library, so only Zotero search is available.",
        };
    }
    if contains_any(
        &lower,
        &[
            "my notes",
            "knowledge base",
            "indexed knowledge",
            "search my documents",
        ],
    ) {
        return ToolPlan {
            allowed: vec!["search_knowledge"],
            max_calls: 1,
            rationale: "The request explicitly asks for saved user context, so only local knowledge search is available.",
        };
    }
    if looks_like_calculation(&lower) {
        return ToolPlan {
            allowed: vec!["calculate"],
            max_calls: 1,
            rationale: "The request contains a calculation where deterministic evaluation can improve accuracy.",
        };
    }
    ToolPlan {
        allowed: Vec::new(),
        max_calls: 0,
        rationale: if code_request && needs_project_evidence(&lower) {
            "No active project is available to inspect, so AEGIS will answer directly without pretending to search files."
        } else {
            "The request can be answered from the conversation and model knowledge without external tools."
        },
    }
}

fn contains_any(text: &str, values: &[&str]) -> bool {
    values.iter().any(|value| text.contains(value))
}

fn looks_like_calculation(text: &str) -> bool {
    contains_any(
        text,
        &["calculate", "compute", "square root", "percentage of"],
    ) || (text.chars().any(|character| character.is_ascii_digit())
        && text
            .chars()
            .any(|character| matches!(character, '+' | '*' | '/' | '^' | '%')))
}

fn needs_project_evidence(text: &str) -> bool {
    contains_any(
        text,
        &[
            "this project",
            "this repo",
            "repository",
            "codebase",
            "our code",
            "my code",
            "this code",
            "this file",
            "find ",
            "where is",
            "locate ",
            "fix ",
            "debug",
            "implement",
            "refactor",
            "review",
            "update ",
            "change ",
            "add ",
            "run test",
            "failing test",
            "build error",
            "compile error",
            "stack trace",
            "src/",
            "src\\",
            ".rs",
            ".ts",
            ".tsx",
            ".js",
            ".py",
        ],
    )
}

// ── helpers ───────────────────────────────────────────────────────────

/// Truncate a string to `max` characters at a UTF-8 safe boundary.
fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // Find the byte offset of the `max`-th character boundary.
    let cutoff = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    let mut result: String = s[..cutoff].to_string();
    result.push_str(&format!("\n[Truncated to {max} characters]"));
    result
}

// ── tool implementations ──────────────────────────────────────────────

/// Safe arithmetic expression evaluator using recursive descent parsing.
/// Supports: +, -, *, /, %, ^ (power), parentheses, basic math constants/functions.
/// Returns a string result or a descriptive error — never panics.
fn calculate_expression_legacy(expr: &str) -> String {
    let trimmed = expr.trim();
    if trimmed.is_empty() {
        return "Error: empty expression.".to_string();
    }

    // Try parsing and evaluating
    match eval_arithmetic(trimmed) {
        Ok(result) => {
            // Format nicely: integer if whole, decimal otherwise
            if result.fract() == 0.0 && result.abs() < 1e15 {
                format!(" = {}", result as i64)
            } else if result.abs() < 1e-6 || result.abs() > 1e15 {
                format!(" = {:.6e}", result)
            } else {
                format!(" = {}", result)
            }
        }
        Err(e) => format!("Error evaluating `{trimmed}`: {e}"),
    }
}

/// Recursive descent parser for arithmetic expressions.
fn eval_arithmetic(expr: &str) -> Result<f64, String> {
    let chars: Vec<char> = expr.chars().collect();
    let mut pos = 0;
    let result = parse_expr(&chars, &mut pos)?;
    skip_whitespace(&chars, &mut pos);
    if pos < chars.len() {
        return Err(format!(
            "Unexpected character `{}` at position {pos}",
            chars[pos]
        ));
    }
    Ok(result)
}

fn parse_expr(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_term(chars, pos)?;
    loop {
        skip_whitespace(chars, pos);
        if *pos >= chars.len() {
            break;
        }
        match chars[*pos] {
            '+' => {
                *pos += 1;
                let right = parse_term(chars, pos)?;
                left += right;
            }
            '-' => {
                *pos += 1;
                let right = parse_term(chars, pos)?;
                left -= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_term(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_power(chars, pos)?;
    loop {
        skip_whitespace(chars, pos);
        if *pos >= chars.len() {
            break;
        }
        match chars[*pos] {
            '*' => {
                *pos += 1;
                let right = parse_power(chars, pos)?;
                left *= right;
            }
            '/' => {
                *pos += 1;
                let right = parse_power(chars, pos)?;
                if right == 0.0 {
                    return Err("Division by zero.".to_string());
                }
                left /= right;
            }
            '%' => {
                *pos += 1;
                let right = parse_power(chars, pos)?;
                if right == 0.0 {
                    return Err("Modulo by zero.".to_string());
                }
                left = left % right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_power(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_unary(chars, pos)?;
    skip_whitespace(chars, pos);
    if *pos < chars.len() && chars[*pos] == '^' {
        *pos += 1;
        let right = parse_unary(chars, pos)?;
        left = left.powf(right);
    }
    Ok(left)
}

fn parse_unary(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    skip_whitespace(chars, pos);
    if *pos >= chars.len() {
        return Err("Unexpected end of expression.".to_string());
    }
    match chars[*pos] {
        '+' => {
            *pos += 1;
            parse_atom(chars, pos)
        }
        '-' => {
            *pos += 1;
            Ok(-parse_atom(chars, pos)?)
        }
        _ => parse_atom(chars, pos),
    }
}

fn parse_atom(chars: &[char], pos: &mut usize) -> Result<f64, String> {
    skip_whitespace(chars, pos);
    if *pos >= chars.len() {
        return Err("Unexpected end of expression.".to_string());
    }

    // Parenthesized sub-expression
    if chars[*pos] == '(' {
        *pos += 1;
        let result = parse_expr(chars, pos)?;
        skip_whitespace(chars, pos);
        if *pos >= chars.len() || chars[*pos] != ')' {
            return Err("Missing closing parenthesis.".to_string());
        }
        *pos += 1;
        return Ok(result);
    }

    // Number (integer or decimal)
    if chars[*pos].is_ascii_digit() || chars[*pos] == '.' {
        let start = *pos;
        while *pos < chars.len()
            && (chars[*pos].is_ascii_digit()
                || chars[*pos] == '.'
                || chars[*pos] == 'e'
                || chars[*pos] == 'E'
                || chars[*pos] == '+'
                || chars[*pos] == '-')
        {
            // Handle scientific notation properly
            if (chars[*pos] == '+' || chars[*pos] == '-')
                && *pos > start
                && (chars[*pos - 1] == 'e' || chars[*pos - 1] == 'E')
            {
                *pos += 1;
            } else if chars[*pos] == '+' || chars[*pos] == '-' {
                break; // End of number, operator follows
            } else {
                *pos += 1;
            }
        }
        let num_str: String = chars[start..*pos].iter().collect();
        return num_str
            .parse::<f64>()
            .map_err(|_| format!("Invalid number: `{num_str}`"));
    }

    // Named constants / functions: pi, e, sqrt(), abs(), round(), floor(), ceil(), sin(), cos(), tan(), log(), ln()
    if chars[*pos].is_ascii_alphabetic() || chars[*pos] == '_' {
        let start = *pos;
        while *pos < chars.len() && (chars[*pos].is_ascii_alphanumeric() || chars[*pos] == '_') {
            *pos += 1;
        }
        let name: String = chars[start..*pos].iter().collect();
        skip_whitespace(chars, pos);

        match name.as_str() {
            "pi" => Ok(std::f64::consts::PI),
            "e" => Ok(std::f64::consts::E),
            "inf" | "infinity" => Ok(f64::INFINITY),
            "nan" => Ok(f64::NAN),
            func if *pos < chars.len() && chars[*pos] == '(' => {
                *pos += 1; // consume (
                let arg = parse_expr(chars, pos)?;
                skip_whitespace(chars, pos);
                if *pos >= chars.len() || chars[*pos] != ')' {
                    return Err(format!("Missing closing parenthesis for `{func}(...)`."));
                }
                *pos += 1; // consume )
                match func {
                    "sqrt" => Ok(arg.sqrt()),
                    "abs" => Ok(arg.abs()),
                    "round" => Ok(arg.round()),
                    "floor" => Ok(arg.floor()),
                    "ceil" => Ok(arg.ceil()),
                    "sin" => Ok(arg.sin()),
                    "cos" => Ok(arg.cos()),
                    "tan" => Ok(arg.tan()),
                    "asin" => Ok(arg.asin()),
                    "acos" => Ok(arg.acos()),
                    "atan" => Ok(arg.atan()),
                    "ln" | "log" => {
                        if arg <= 0.0 {
                            return Err("Logarithm of non-positive number.".to_string());
                        }
                        Ok(arg.ln())
                    }
                    "log2" => {
                        if arg <= 0.0 {
                            return Err("Logarithm of non-positive number.".to_string());
                        }
                        Ok(arg.log2())
                    }
                    "log10" => {
                        if arg <= 0.0 {
                            return Err("Logarithm of non-positive number.".to_string());
                        }
                        Ok(arg.log10())
                    }
                    "exp" => Ok(arg.exp()),
                    "deg" => Ok(arg.to_degrees()),
                    "rad" => Ok(arg.to_radians()),
                    _ => Err(format!("Unknown function: `{func}`")),
                }
            }
            _ => Err(format!("Unknown identifier: `{name}`")),
        }
    } else {
        Err(format!(
            "Unexpected character `{}` at position {pos}",
            chars[*pos]
        ))
    }
}

fn skip_whitespace(chars: &[char], pos: &mut usize) {
    while *pos < chars.len() && chars[*pos].is_ascii_whitespace() {
        *pos += 1;
    }
}

async fn search_knowledge_base_legacy(
    query: &str,
    rag_client: &RagClient,
    session_id: &str,
) -> String {
    match rag_client.retrieve(query, 5, 0.0, session_id).await {
        Ok(outcome) if !outcome.chunks.is_empty() => {
            let mut result = String::from("Found relevant chunks:\n");
            for (i, chunk) in outcome.chunks.iter().enumerate() {
                let src = std::path::Path::new(&chunk.source)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&chunk.source);
                result.push_str(&format!("\n--- Result {} (from {}) ---\n", i + 1, src));
                if let Some(page) = chunk.page {
                    result.push_str(&format!("  Page {}\n", page));
                }
                result.push_str(&chunk.text);
                result.push('\n');
            }
            result
        }
        Ok(_) => format!("No results found for query: {query}"),
        Err(e) => format!("Knowledge search failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn general_reasoning_routes_directly() {
        let plan = select_tool_plan(
            "Explain why the sky is blue",
            WorkflowId::Default,
            "general",
            false,
            true,
        );
        assert!(plan.is_direct());
    }

    #[test]
    fn conceptual_code_question_does_not_search_project() {
        let plan = select_tool_plan(
            "Explain Rust ownership",
            WorkflowId::CodeExplain,
            "coder",
            false,
            true,
        );
        assert!(plan.is_direct());
    }

    #[test]
    fn workspace_fix_gets_only_project_tools() {
        let plan = select_tool_plan(
            "Fix the failing tests in this project",
            WorkflowId::CodeDebug,
            "coder",
            false,
            true,
        );
        assert!(plan.allowed.contains(&"search"));
        assert!(!plan.allowed.contains(&"zotero"));
        assert!(!plan.allowed.contains(&"search_knowledge"));
        assert_eq!(plan.max_calls, 3);
    }

    #[test]
    fn attached_document_context_does_not_trigger_second_search() {
        let plan = select_tool_plan(
            "Summarize the attached report",
            WorkflowId::Summarize,
            "academic",
            true,
            false,
        );
        assert!(plan.is_direct());
    }

    #[test]
    fn calculation_gets_only_calculator() {
        let plan = select_tool_plan(
            "What is the square root of 8977?",
            WorkflowId::Default,
            "general",
            false,
            false,
        );
        assert_eq!(plan.allowed, vec!["calculate"]);
        assert_eq!(plan.max_calls, 1);
    }

    #[test]
    fn calculator_supports_the_operations_selected_by_the_router() {
        let square_root = calculate_expression("sqrt(8977)")
            .unwrap()
            .parse::<f64>()
            .unwrap();

        assert!((square_root - 94.74703161577148).abs() < 1e-10);
        assert_eq!(calculate_expression("2^3^2").unwrap(), "512");
        assert_eq!(calculate_expression("17 % 5").unwrap(), "2");
        assert_eq!(calculate_expression("sqrt(16) + abs(-2)").unwrap(), "6");
    }

    #[test]
    fn calculator_rejects_invalid_domains() {
        assert!(calculate_expression("sqrt(-1)").is_err());
        assert!(calculate_expression("1 / 0").is_err());
    }

    #[test]
    fn required_calculator_route_derives_square_root_expression() {
        let plan = select_tool_plan(
            "Calculate the square root of 8977 accurately",
            WorkflowId::Default,
            "general",
            false,
            false,
        );

        match required_tool_fallback(&plan, "Calculate the square root of 8977 accurately") {
            Some(ToolCall::Calculate(expression)) => assert_eq!(expression, "sqrt(8977)"),
            _ => panic!("expected a deterministic calculator fallback"),
        }
    }

    #[test]
    fn required_project_route_derives_explicit_file_path() {
        assert_eq!(
            extract_project_file_path("Review `src/react_loop/mod.rs` for bugs"),
            Some("src/react_loop/mod.rs".to_string())
        );
    }

    #[test]
    fn current_conversation_recall_does_not_launch_knowledge_search() {
        let plan = select_tool_plan(
            "What did I say earlier in this conversation?",
            WorkflowId::Default,
            "general",
            false,
            false,
        );

        assert!(plan.is_direct());
    }

    #[test]
    fn coder_mode_greeting_does_not_claim_a_project_is_missing() {
        let plan = select_tool_plan(
            "Hello, how are you?",
            WorkflowId::Default,
            "coder",
            false,
            false,
        );

        assert!(plan.is_direct());
        assert!(!plan.rationale.contains("No active project"));
    }

    #[test]
    fn project_request_without_project_explains_the_direct_fallback() {
        let plan = select_tool_plan(
            "Fix the failing tests in this project",
            WorkflowId::CodeDebug,
            "coder",
            false,
            false,
        );

        assert!(plan.is_direct());
        assert!(plan.rationale.contains("No active project"));
    }

    #[test]
    fn code_requests_receive_code_formatting_guidance() {
        assert!(
            response_format_instruction("Create a Python function").contains("fenced Markdown")
        );
    }

    #[test]
    fn greetings_receive_prose_formatting_guidance() {
        let instruction = response_format_instruction("Hello, who are you?");
        assert!(instruction.contains("normal Markdown prose"));
        assert!(instruction.contains("Do not use a code fence"));
    }

    #[test]
    fn math_requests_receive_latex_formatting_guidance() {
        let instruction = response_format_instruction("Compute the derivative of x^12");

        assert!(instruction.contains("valid LaTeX"));
        assert!(instruction.contains("`$...$`"));
        assert!(instruction.contains("`$$...$$`"));
        assert!(instruction.contains("never put math in a code fence"));
    }

    #[test]
    fn direct_reasoning_prompt_keeps_history_and_latest_request_separate() {
        let prompt = build_direct_reasoned_prompt(
            "What should you call me?",
            "PRIOR CONVERSATION:\nuser: Call me Sam\nassistant: Hello Sam.",
        );

        assert!(prompt.contains("Conversation context:\nPRIOR CONVERSATION:"));
        assert!(prompt.contains("user: Call me Sam"));
        assert!(prompt.contains("User request:\nWhat should you call me?"));
        assert!(
            prompt.find("user: Call me Sam").unwrap()
                < prompt
                    .find("User request:\nWhat should you call me?")
                    .unwrap()
        );
    }

    #[test]
    fn direct_reasoning_prompt_includes_persistent_memory() {
        let prompt = build_direct_reasoned_prompt(
            "What is my name?",
            "SELECTED PERSISTENT MEMORY:\n- category: identity; priority: high; note: My name is Sam",
        );

        assert!(prompt.contains("note: My name is Sam"));
        assert!(prompt.contains("latest request always takes priority"));
    }

    #[test]
    fn empty_reasoning_context_does_not_add_placeholder_noise() {
        let prompt = build_direct_reasoned_prompt("Hello", "  ");

        assert!(!prompt.contains("Conversation context:"));
        assert!(prompt.contains("User request:\nHello"));
    }

    #[test]
    fn tool_reasoning_prompt_receives_context_before_current_query() {
        let prompt = build_reasoning_conversation(
            "system rules",
            "user: Remember token alpha\nassistant: I will remember it.",
            "What token did I give you?",
        );

        assert!(prompt.starts_with("[System]\nsystem rules"));
        assert!(prompt.contains("Conversation context:\nuser: Remember token alpha"));
        assert!(prompt.ends_with("[User]\nWhat token did I give you?"));
    }

    // ── extract_json ──────────────────────────────────────────────────

    #[test]
    fn extracts_raw_json() {
        assert_eq!(
            extract_json(r#"{"tool":"search","arguments":{"query":"test"}}"#),
            r#"{"tool":"search","arguments":{"query":"test"}}"#
        );
    }

    #[test]
    fn extracts_json_from_code_fence() {
        assert_eq!(
            extract_json("```json\n{\"tool\":\"search\"}\n```"),
            r#"{"tool":"search"}"#
        );
    }

    #[test]
    fn extracts_json_from_plain_fence() {
        assert_eq!(
            extract_json("```\n{\"tool\":\"search\"}\n```"),
            r#"{"tool":"search"}"#
        );
    }

    // ── parse_react_response ──────────────────────────────────────────

    #[test]
    fn parses_final_answer() {
        let input = r#"{"thought":"done","answer":"The answer is 42."}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::FinalAnswer(a) => assert_eq!(a, "The answer is 42."),
            _ => panic!("Expected FinalAnswer"),
        }
    }

    #[test]
    fn accepts_plain_model_answer_without_json_repair() {
        match parse_react_response("A direct answer without routing JSON.").unwrap() {
            ModelDecision::FinalAnswer(answer) => {
                assert_eq!(answer, "A direct answer without routing JSON.")
            }
            _ => panic!("Expected FinalAnswer"),
        }
    }

    #[test]
    fn sanitize_final_answer_strips_think_tags_and_extracts_answer_json() {
        let input =
            r#"<think>private scratchpad</think>{"thought":"hidden","answer":"Visible answer."}"#;
        assert_eq!(sanitize_final_answer(input), "Visible answer.");
    }

    #[test]
    fn parses_search_tool() {
        let input = r#"{"thought":"need code","tool":"search","arguments":{"query":"sort fn"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::Search(q)) => assert_eq!(q, "sort fn"),
            _ => panic!("Expected Tool(Search)"),
        }
    }

    #[test]
    fn parses_read_file_tool() {
        let input =
            r#"{"thought":"read code","tool":"read_file","arguments":{"path":"src/main.rs"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::ReadFile(p)) => assert_eq!(p, "src/main.rs"),
            _ => panic!("Expected Tool(ReadFile)"),
        }
    }

    #[test]
    fn rejects_empty_answer() {
        let input = r#"{"thought":"done","answer":""}"#;
        assert!(parse_react_response(input).is_err());
    }

    #[test]
    fn rejects_unknown_tool() {
        let input = r#"{"thought":"hmm","tool":"unknown_tool","arguments":{}}"#;
        assert!(parse_react_response(input).is_err());
    }

    #[test]
    fn rejects_malformed_tool_json() {
        assert!(parse_react_response(r#"{"tool":"search","arguments":oops}"#).is_err());
    }

    // ── truncate ──────────────────────────────────────────────────────

    #[test]
    fn truncate_short_string_is_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn truncate_long_string_is_cut() {
        let result = truncate("hello world", 5);
        assert!(result.starts_with("hello"));
        assert!(result.contains("[Truncated"));
    }

    #[test]
    fn truncate_handles_multi_byte_utf8() {
        // 4-byte emoji at position 5 would panic with byte slicing
        let s = "hello😀world";
        let result = truncate(s, 6); // 5 ASCII + 1 emoji = 5 + 4 bytes
        assert!(result.starts_with("hello😀"));
        // Should not panic
    }

    // ── validate_read_path ──────────────────────────────────────────

    #[test]
    fn validate_read_path_rejects_absolute_escape() {
        let err = validate_read_path("C:\\Windows\\system32\\config", None).unwrap_err();
        assert!(err.to_string().contains("Access denied"));
    }

    #[test]
    fn validate_read_path_rejects_traversal_escape() {
        let err = validate_read_path("../../../../etc/passwd", None).unwrap_err();
        assert!(
            err.to_string().contains("Access denied") || err.to_string().contains("does not exist")
        );
    }

    #[test]
    fn validate_read_path_accepts_relative_file() {
        // Cargo.toml should exist in the engine directory
        let result = validate_read_path("Cargo.toml", None);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_read_path_uses_explicit_project_root() {
        let project = env!("CARGO_MANIFEST_DIR");
        let result = validate_read_path("src/react_loop/mod.rs", Some(project)).unwrap();
        assert!(result.starts_with(Path::new(project).canonicalize().unwrap()));
    }

    #[test]
    fn validate_read_path_rejects_escape_from_explicit_project_root() {
        let project = env!("CARGO_MANIFEST_DIR");
        let error = validate_read_path("../cli/Cargo.toml", Some(project)).unwrap_err();
        assert!(error.to_string().contains("Access denied"));
    }

    // ── run_terminal parsing ──────────────────────────────────────────

    #[test]
    fn parses_run_terminal_tool() {
        let input =
            r#"{"thought":"compile","tool":"run_terminal","arguments":{"command":"cargo check"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::RunTerminal(cmd)) => assert_eq!(cmd, "cargo check"),
            _ => panic!("Expected Tool(RunTerminal)"),
        }
    }

    #[test]
    fn parses_terminal_alias() {
        // "terminal" is an alias for "run_terminal"
        let input =
            r#"{"thought":"compile","tool":"terminal","arguments":{"command":"cargo check"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::RunTerminal(cmd)) => assert_eq!(cmd, "cargo check"),
            _ => panic!("Expected Tool(RunTerminal)"),
        }
    }

    #[test]
    fn rejects_terminal_missing_command() {
        let input = r#"{"thought":"compile","tool":"run_terminal","arguments":{}}"#;
        let err = parse_react_response(input).unwrap_err();
        assert!(err.to_string().contains("missing"));
    }

    // ── run_terminal safety guard ─────────────────────────────────────

    #[test]
    fn parses_write_file_tool() {
        let input = r#"{"thought":"writing","tool":"write_file","arguments":{"path":"src/main.rs","content":"fn main() {}"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::WriteFile(p, c)) => {
                assert_eq!(p, "src/main.rs");
                assert_eq!(c, "fn main() {}");
            }
            _ => panic!("Expected Tool(WriteFile)"),
        }
    }

    #[test]
    fn parses_write_alias() {
        let input = r#"{"thought":"writing","tool":"write","arguments":{"path":"src/lib.rs","content":"pub fn hello() {}"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::WriteFile(p, c)) => {
                assert_eq!(p, "src/lib.rs");
                assert!(!c.is_empty());
            }
            _ => panic!("Expected Tool(WriteFile)"),
        }
    }

    #[test]
    fn parses_search_files_tool() {
        let input = r#"{"thought":"finding","tool":"search_files","arguments":{"pattern":"test","dir":"src"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::SearchFiles(p, d)) => {
                assert_eq!(p, "test");
                assert_eq!(d, Some("src".to_string()));
            }
            _ => panic!("Expected Tool(SearchFiles)"),
        }
    }

    #[test]
    fn parses_search_files_without_dir() {
        let input =
            r#"{"thought":"finding","tool":"search_files","arguments":{"pattern":"Cargo.toml"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::SearchFiles(p, d)) => {
                assert_eq!(p, "Cargo.toml");
                assert!(d.is_none());
            }
            _ => panic!("Expected Tool(SearchFiles)"),
        }
    }

    #[test]
    fn parses_find_alias() {
        let input = r#"{"thought":"finding","tool":"find","arguments":{"pattern":"README"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::SearchFiles(p, _)) => assert_eq!(p, "README"),
            _ => panic!("Expected Tool(SearchFiles)"),
        }
    }

    #[tokio::test]
    async fn safety_guard_rejects_rm_rf() {
        let result = run_terminal_command("rm -rf /", None).await;
        assert!(result.contains("rejected"));
    }

    #[tokio::test]
    async fn safety_guard_rejects_mkfs() {
        let result = run_terminal_command("mkfs.ext4 /dev/sda1", None).await;
        assert!(result.contains("rejected"));
    }

    #[tokio::test]
    async fn safety_guard_rejects_mutating_shell_commands() {
        let result = run_terminal_command("Set-Content note.txt hello", None).await;
        assert!(result.contains("rejected"));
    }

    #[tokio::test]
    async fn safety_guard_allows_safe_commands() {
        // We can't actually run this in a test, but we can verify it
        // passes the safety guard by checking it doesn't contain the
        // rejection message.
        let result = run_terminal_command("echo ok", None).await;
        assert!(!result.contains("rejected"));
    }

    // ── ocr_image / describe_image parsing ──────────────────────────

    #[test]
    fn parses_ocr_image_tool() {
        let input = r#"{"thought":"ocr","tool":"ocr_image","arguments":{"path":"screenshot.png"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::OcrImage(p)) => assert_eq!(p, "screenshot.png"),
            _ => panic!("Expected Tool(OcrImage)"),
        }
    }

    #[test]
    fn parses_ocr_alias() {
        let input = r#"{"thought":"ocr","tool":"ocr","arguments":{"path":"test.png"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::OcrImage(p)) => assert_eq!(p, "test.png"),
            _ => panic!("Expected Tool(OcrImage)"),
        }
    }

    #[test]
    fn parses_describe_image_tool() {
        let input =
            r#"{"thought":"describe","tool":"describe_image","arguments":{"path":"diagram.png"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::DescribeImage(p)) => assert_eq!(p, "diagram.png"),
            _ => panic!("Expected Tool(DescribeImage)"),
        }
    }

    #[test]
    fn parses_vision_alias() {
        let input = r#"{"thought":"describe","tool":"vision","arguments":{"path":"chart.png"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::DescribeImage(p)) => assert_eq!(p, "chart.png"),
            _ => panic!("Expected Tool(DescribeImage)"),
        }
    }

    #[test]
    fn parses_describe_alias() {
        let input = r#"{"thought":"describe","tool":"describe","arguments":{"path":"arch.png"}}"#;
        match parse_react_response(input).unwrap() {
            ModelDecision::Tool(ToolCall::DescribeImage(p)) => assert_eq!(p, "arch.png"),
            _ => panic!("Expected Tool(DescribeImage)"),
        }
    }
}
