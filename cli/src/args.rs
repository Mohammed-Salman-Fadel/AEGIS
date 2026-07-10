//! Role: shared Clap argument structs reused across the CLI command tree.
//! Called by: `cli.rs` when composing subcommands.
//! Calls into: Clap derive macros only.
//! Owns: positional arguments and reusable flag groups for scaffold commands.
//! Does not own: top-level command routing, validation side effects, or backend calls.
//! Next TODOs: add richer prompt/session flags once the engine contract and config format are finalized.
use std::path::PathBuf;

use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct LogsArgs {
    /// Service to show logs for: engine, rag, web-ui, or all
    #[arg(default_value = "all")]
    pub service: String,

    /// Number of lines to show (from the end)
    #[arg(long, short = 'n', default_value = "50")]
    pub lines: usize,

    /// Follow/tail the log in real time
    #[arg(long, short = 'f')]
    pub follow: bool,
}

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    #[arg(
        long,
        alias = "install-root",
        alias = "root",
        value_name = "path",
        help = "Install AEGIS into this directory instead of the shown default path"
    )]
    pub path: Option<PathBuf>,

    #[arg(
        long,
        help = "Show the staged installation plan without attempting any actions"
    )]
    pub plan_only: bool,

    #[arg(
        long,
        help = "Future flag: run the Windows-first installer flow once TODOs are implemented"
    )]
    pub yes: bool,
}

#[derive(Debug, Clone, Args)]
pub struct SaveArgs {
    #[arg(
        value_name = "note",
        help = "A personalization note to store locally for future responses"
    )]
    pub note: String,
}

#[derive(Debug, Clone, Args)]
pub struct ChatArgs {
    #[arg(
        value_name = "prompt",
        help = "Prompt text to send to the future engine /chat endpoint"
    )]
    pub prompt: String,

    #[arg(
        long,
        value_name = "session-id",
        help = "Future session identifier managed by the engine"
    )]
    pub session_id: Option<String>,

    #[arg(
        long = "attach",
        short = 'a',
        value_name = "file",
        help = "Attach and index a PDF/TXT/code file into the chat session before sending"
    )]
    pub attachments: Vec<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct AskArgs {
    #[arg(
        long,
        help = "Read prompt content from stdin instead of a positional argument"
    )]
    pub stdin: bool,

    #[arg(
        long,
        value_name = "session-id",
        help = "Future session identifier managed by the engine"
    )]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ReplArgs {
    #[arg(
        long,
        value_name = "session-id",
        help = "Future session identifier managed by the engine"
    )]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct RequiredSessionIdArg {
    #[arg(value_name = "id", help = "Engine-owned session identifier")]
    pub id: String,
}

#[derive(Debug, Clone, Args)]
pub struct OptionalSessionIdArg {
    #[arg(value_name = "id", help = "Engine-owned session identifier")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct OptionalNameArg {
    #[arg(value_name = "name", help = "Engine-owned provider or model name")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct SearchArgs {
    #[arg(
        value_name = "query",
        help = "Text to fuzzy-search across session titles and recent turns"
    )]
    pub query: String,
}

#[derive(Debug, Clone, Args)]
pub struct SessionExportArgs {
    #[arg(
        value_name = "id",
        help = "Session id to export; defaults to the most recent session"
    )]
    pub id: Option<String>,

    #[arg(
        long,
        short = 'f',
        default_value = "md",
        value_name = "md|json|pdf",
        help = "Export format"
    )]
    pub format: String,

    #[arg(
        long,
        short = 'o',
        value_name = "path",
        help = "Output file path; defaults to a safe session-based name"
    )]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Args)]
pub struct UpgradeArgs {
    /// Only check for updates, don't download
    #[arg(long)]
    pub check: bool,

    /// Skip confirmation prompt
    #[arg(long, short = 'y')]
    pub yes: bool,
}
