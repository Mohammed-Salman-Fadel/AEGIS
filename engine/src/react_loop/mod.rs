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
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};

use crate::inference::InferenceBackend;
use crate::rag_client::RagClient;
use crate::tool_registry::ToolRegistry;

// ── constants ─────────────────────────────────────────────────────────

const MAX_ROUNDS: usize = 6;
const MAX_FILE_CHARS: usize = 10_000;
const MAX_TOOL_OUTPUT_CHARS: usize = 20_000;
const MAX_TERMINAL_OUTPUT_CHARS: usize = 10_000;
const TERMINAL_TIMEOUT_SECS: u64 = 30;
const MAX_FILE_WRITE_BYTES: usize = 100_000;
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
        session_id: &str,
        model_name: &str,
        project_path: Option<&str>,
        tx: mpsc::Sender<String>,
    ) -> Result<String> {
        let system_prompt = build_system_prompt();
        let mut conversation = format!("[System]\n{system_prompt}\n\n[User]\n{query}");
        let mut last_call: Option<(String, String)> = None;

        for round in 1..=MAX_ROUNDS {
            let _ = tx
                .send(format!(
                    "\n[REACT: thinking (round {round}/{MAX_ROUNDS})]\n"
                ))
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
                    let _ = tx
                        .send(format!(
                            "\n[REACT: JSON parse error — asking model to correct: {e}]\n"
                        ))
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
                        Err(e2) => {
                            // Second failure — give the user what we have.
                            let _ = tx
                                .send(format!(
                                    "\n[REACT: still unparseable — falling back to text: {e2}]\n"
                                ))
                                .await;
                            return Ok(raw2);
                        }
                    }
                }
            };

            match decision {
                ModelDecision::FinalAnswer(answer) => {
                    return Ok(answer);
                }
                ModelDecision::Tool(tool_call) => {
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
                            let _ = tx
                                .send(format!(
                                    "\n[REACT: you just made the same call (`{}`); \
                                     try a different approach.]\n",
                                    tool_call.tool_name()
                                ))
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
                    let _ = tx
                        .send(format!("\n[REACT: calling `{}`]\n", tool_call.tool_name()))
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
                            format!("\n[Tool Result: {}]\n```\n{}\n```", r.tool_name, truncated)
                        }
                        Err(e) => {
                            format!("\n[Tool Error: {}]\n{e}", tool_call.tool_name())
                        }
                    };

                    conversation.push_str(&result_str);
                }
            }
        }

        // ── round limit reached — force final answer ────────────────
        let _ = tx
            .send(format!(
                "\n[REACT: round limit ({MAX_ROUNDS}) reached, forcing final answer]\n"
            ))
            .await;

        let force_prompt = format!(
            "{}\n\nYou have used all {MAX_ROUNDS} rounds. \
             Provide your final answer now. Do not call any more tools.\n\n\
             [Assistant — final answer only, no JSON]",
            conversation
        );

        // Best-effort: if the force-finalize call itself fails, return
        // whatever conversation we built up so the user isn't left empty-handed.
        let inference = inference_lock.read().await;
        match inference.call(&force_prompt, model_name).await {
            Ok(answer) => Ok(answer),
            Err(e) => {
                let fallback = format!(
                    "{}\n\n[Note: The engine reached its reasoning limit and \
                     encountered an error generating the final answer: {e}. \
                     The information above was gathered during the process.]",
                    conversation
                );
                Ok(fallback)
            }
        }
    }
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
            let output = tool_registry
                .execute_code_search(query, project_path)
                .await
                .unwrap_or_else(|e| format!("Search failed: {e}"));
            Ok(ToolResult {
                tool_name: "search".into(),
                input: query.clone(),
                output,
            })
        }
        ToolCall::ReadFile(path) => {
            let safe_path = validate_read_path(path)?;
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
            let safe_path = validate_write_path(path)?;
            let size = content.len();
            if size > MAX_FILE_WRITE_BYTES {
                Ok(ToolResult {
                    tool_name: "write_file".into(),
                    input: format!("{path} ({size} bytes)"),
                    output: format!(
                        "Write rejected: content is {size} bytes which exceeds the \
                         maximum of {MAX_FILE_WRITE_BYTES} bytes."
                    ),
                })
            } else {
                // Create parent directories before writing so new paths work.
                if let Some(parent) = safe_path.parent() {
                    if let Err(e) = tokio::fs::create_dir_all(parent).await {
                        return Ok(ToolResult {
                            tool_name: "write_file".into(),
                            input: format!("{path} ({size} bytes)"),
                            output: format!(
                                "Failed to create parent directories for `{path}`: {e}"
                            ),
                        });
                    }
                }
                match tokio::fs::write(&safe_path, content).await {
                    Ok(()) => Ok(ToolResult {
                        tool_name: "write_file".into(),
                        input: format!("{path} ({size} bytes)"),
                        output: format!("Successfully wrote {size} bytes to `{path}`."),
                    }),
                    Err(e) => Ok(ToolResult {
                        tool_name: "write_file".into(),
                        input: format!("{path} ({size} bytes)"),
                        output: format!("Failed to write `{path}`: {e}"),
                    }),
                }
            }
        }
        ToolCall::SearchFiles(pattern, dir) => {
            let output = search_files_locally(pattern, dir.as_deref()).await;
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
            let output = run_terminal_command(command).await;
            Ok(ToolResult {
                tool_name: "run_terminal".into(),
                input: command.clone(),
                output,
            })
        }
        ToolCall::GitStatus(path) => {
            let output = git_status_command(path).await;
            Ok(ToolResult {
                tool_name: "git_status".into(),
                input: path.clone(),
                output,
            })
        }
        ToolCall::ListDirectory(path) => {
            let output = list_directory_command(path).await;
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

/// Path-sanitisation guard for `read_file`.
///
/// Resolves the path to its canonical form and rejects it if it does
/// not reside inside an allowed directory.  Currently allows reading
/// files under the current working directory.  Extend `ALLOWED_ROOTS`
/// if the tool should cover wider filesystem access.
fn validate_read_path(requested: &str) -> Result<std::path::PathBuf> {
    let cwd = std::env::current_dir().context("Could not determine current working directory")?;
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

/// Path-sanitisation guard for `write_file`.
///
/// Unlike `validate_read_path`, the *file itself* does not need to exist
/// (we're creating it).  We canonicalize the *parent directory* to verify
/// it is within the allowed project root, then append the filename.
///
/// Also blocks writes to known sensitive system paths as a safety net.
fn validate_write_path(requested: &str) -> Result<std::path::PathBuf> {
    let cwd = std::env::current_dir().context("Could not determine current working directory")?;
    let cwd = cwd
        .canonicalize()
        .context("Could not canonicalize current working directory")?;

    let candidate = Path::new(requested);
    let candidate = if candidate.is_relative() {
        cwd.join(candidate)
    } else {
        candidate.to_path_buf()
    };

    // Block writes to sensitive system paths regardless of cwd.
    let candidate_lower = candidate.to_string_lossy().to_lowercase();
    let system_paths = [
        "/etc/",
        "/usr/",
        "/bin/",
        "/boot/",
        "/dev/",
        "/proc/",
        "/sys/",
        "/var/",
        "/lib/",
        "/lib64/",
        "/opt/",
        "/root/",
        "/sbin/",
        "c:\\windows",
        "c:\\program files",
        "c:\\programdata",
        "c:\\system volume information",
        "c:\\boot",
    ];
    if system_paths.iter().any(|p| candidate_lower.contains(p)) {
        anyhow::bail!(
            "Access denied: writing to system paths is not allowed through the \
             write_file tool. Requested: `{requested}`"
        );
    }

    // Canonicalize the parent — it must exist and be within bounds.
    let parent = candidate
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Path `{requested}` has no parent directory"))?;

    // If parent doesn't exist yet, try its parent recursively.
    let canonical_parent = match parent.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            // Walk up until we find an existing ancestor.
            let mut ancestor = Some(parent);
            while let Some(a) = ancestor {
                if a.canonicalize().is_ok() {
                    break;
                }
                ancestor = a.parent();
            }
            anyhow::bail!(
                "Parent directory for `{requested}` does not exist. \
                 Use run_terminal(\"mkdir -p ...\") to create it first, \
                 or use a path under an existing directory."
            );
        }
    };

    if !canonical_parent.starts_with(&cwd) {
        anyhow::bail!(
            "Access denied: `{requested}` resolves outside the \
             allowed directory (`{}`)",
            cwd.display()
        );
    }

    Ok(candidate)
}

/// Execute a shell command with a timeout.
///
/// Uses `sh -c` on Unix and `cmd /c` on Windows so pipes, redirects,
/// chaining (`&&`, `||`) work as expected.
///
/// Blocks for a maximum of `TERMINAL_TIMEOUT_SECS` — the ReAct loop
/// would deadlock if a command hangs forever.
async fn run_terminal_command(command: &str) -> String {
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

    // Determine the shell command.
    let (shell, flag) = if cfg!(windows) {
        ("cmd.exe", "/C")
    } else {
        ("sh", "-c")
    };

    let result = tokio::time::timeout(
        std::time::Duration::from_secs(TERMINAL_TIMEOUT_SECS),
        tokio::process::Command::new(shell)
            .arg(flag)
            .arg(command)
            .output(),
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

/// Recursively search for files matching a glob-like pattern.
///
/// Uses simple substring matching (not full glob) so no extra
/// dependencies are needed.  Searches up to `MAX_SEARCH_RESULTS`
/// files and returns a tree-style listing.
async fn search_files_locally(pattern: &str, dir: Option<&str>) -> String {
    if pattern.trim().is_empty() {
        return "Error: search pattern cannot be empty.".to_string();
    }

    let root = match dir {
        Some(d) => match std::path::Path::new(d).canonicalize() {
            Ok(p) => p,
            Err(e) => return format!("Could not access directory `{d}`: {e}"),
        },
        None => match std::env::current_dir() {
            Ok(p) => p,
            Err(e) => return format!("Could not determine current directory: {e}"),
        },
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
async fn git_status_command(path: &str) -> String {
    let dir = match validate_read_path(path) {
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
async fn list_directory_command(path: &str) -> String {
    let dir = match validate_read_path(path) {
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
    let safe_path = match validate_read_path(path) {
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
    let safe_path = match validate_read_path(path) {
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
            if matches!(self.chars.peek(), Some('+') | Some('-')) {
                buf.push(self.chars.next().unwrap());
            }
            while let Some(ch) = self.chars.peek().copied() {
                if ch.is_ascii_digit() || ch == '.' {
                    buf.push(ch);
                    self.chars.next();
                } else {
                    break;
                }
            }
            if buf.is_empty() || buf == "+" || buf == "-" {
                anyhow::bail!("expected a number");
            }
            Ok(buf
                .parse::<f64>()
                .with_context(|| format!("could not parse number `{buf}`"))?)
        }

        fn parse_factor(&mut self) -> Result<f64> {
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
                Some(_) => self.parse_number(),
                None => anyhow::bail!("expected an expression"),
            }
        }

        fn parse_term(&mut self) -> Result<f64> {
            let mut value = self.parse_factor()?;
            loop {
                self.skip_ws();
                let op = match self.chars.peek().copied() {
                    Some('*') => '*',
                    Some('/') => '/',
                    _ => break,
                };
                self.chars.next();
                let rhs = self.parse_factor()?;
                value = match op {
                    '*' => value * rhs,
                    '/' => {
                        if rhs == 0.0 {
                            anyhow::bail!("division by zero");
                        }
                        value / rhs
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
    let parsed: ReActResponse = serde_json::from_str(&json)
        .with_context(|| format!("Failed to parse model JSON response:\n{raw}"))?;

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
                    "Unknown tool `{other}`. Valid tools: search, read_file, write_file, search_files, run_terminal, git_status, list_directory, calculate, search_knowledge, ocr_image, describe_image, rag, zotero"
                )
            }
        };
        return Ok(ModelDecision::Tool(tool_call));
    }

    anyhow::bail!("Model response did not contain a tool call or final answer.\nResponse:\n{raw}")
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

fn build_system_prompt() -> String {
    r#"You are AEGIS, a local AI assistant that uses tools to gather information before answering.

AVAILABLE TOOLS

search(query) — Search the codebase for relevant code.
  Example: search("sort function in Rust")

read_file(path) — Read the full contents of a file.
  Path is relative to the project root. Do NOT use absolute paths.
  Example: read_file("src/main.rs")

rag(query) — Search the user's imported documents.
  Use when the user has attached documents or asks about document content.
  Example: rag("machine learning concepts")

zotero(query) — Search the Zotero research library.
  Use for research-related questions.
  Example: zotero("transformer architecture")

run_terminal(command) — Run a shell command and capture its output.
  Use this to compile code, run tests, check build output, or inspect
  the environment. The command runs in the project root directory with
  a 30-second timeout. Pipes, redirects, and chaining (&&, ||) work.
  Example: run_terminal("cargo check")
  Example: run_terminal("cd src && cargo test")

write_file(path, content) — Write content to a file.
  Use this to create new files or modify existing ones. Will create
  parent directories if needed. Content is capped at 100,000 bytes.
  Use relative paths from the project root.
  Example: write_file("src/main.rs", "fn main() { }")

search_files(pattern, dir) — Search for files by name.
  Use this to find files in the project when you don't know the exact
  path. Pattern is case-insensitive substring matching. dir is optional
  and defaults to the project root. Skips hidden dirs, node_modules,
  target, and .git.
  Example: search_files("test", "src")
  Example: search_files("Cargo.toml")

search_knowledge(query) — Search the persistent knowledge base.
  Use this to retrieve information from previously indexed documents
  or past conversations. Returns relevant text excerpts with
  relevance scores. Indexed documents persist across sessions.
  Example: search_knowledge("authentication flow")

ocr_image(path) — Extract text from an image using OCR.
  Use this for screenshots, scanned documents, or photos containing
  text. Routes through the Python RAG service with Tesseract OCR.
  Best for code screenshots, document scans, and terminal output.
  Examples: ocr_image("screenshot.png")

describe_image(path) — Generate a detailed description of an image.
  Use this for UI layouts, diagrams, charts, architecture drawings,
  or any image where visual understanding matters more than exact
  text. Uses Ollama vision model (llava). Falls back gracefully if
  no vision model is loaded.
  Tip: The tool name is "describe_image", "vision", or "describe".
  Example: describe_image("diagram.png")

HOW TO RESPOND

You MUST respond with valid JSON in exactly one of these formats.

To call a tool:
{
  "thought": "I need to find the relevant code first.",
  "tool": "search",
  "arguments": { "query": "sort function implementation" }
}

To provide the final answer:
{
  "thought": "I have all the information needed.",
  "answer": "Here is the explanation..."
}

RULES

- Call at most ONE tool per round.
- After each tool result, decide if you need more information or can answer.
- Be specific with search queries.
- When you have enough information, ALWAYS provide the final answer.
- Do NOT repeat the same tool call with the same arguments.
- Use relative paths only for read_file (e.g. "src/main.rs" not "/abs/path").
- You have a maximum of 6 rounds. Finish before then.
- If a tool returns an error, try a different approach or answer with what you have.
- ALWAYS output valid JSON. Never wrap it in additional text or markdown."#
        .to_string()
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
    fn rejects_gibberish() {
        assert!(parse_react_response("not json at all").is_err());
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
        let err = validate_read_path("C:\\Windows\\system32\\config").unwrap_err();
        assert!(err.to_string().contains("Access denied"));
    }

    #[test]
    fn validate_read_path_rejects_traversal_escape() {
        let err = validate_read_path("../../../../etc/passwd").unwrap_err();
        assert!(
            err.to_string().contains("Access denied") || err.to_string().contains("does not exist")
        );
    }

    #[test]
    fn validate_read_path_accepts_relative_file() {
        // Cargo.toml should exist in the engine directory
        let result = validate_read_path("Cargo.toml");
        assert!(result.is_ok());
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

    #[test]
    fn safety_guard_rejects_rm_rf() {
        let result = run_terminal_command("rm -rf /").await;
        assert!(result.contains("rejected"));
    }

    #[test]
    fn safety_guard_rejects_mkfs() {
        let result = run_terminal_command("mkfs.ext4 /dev/sda1").await;
        assert!(result.contains("rejected"));
    }

    #[test]
    fn safety_guard_allows_safe_commands() {
        // We can't actually run this in a test, but we can verify it
        // passes the safety guard by checking it doesn't contain the
        // rejection message.
        let result = run_terminal_command("cargo check").await;
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
