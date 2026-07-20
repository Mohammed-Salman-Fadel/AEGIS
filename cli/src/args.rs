//! Role: shared Clap argument structs reused across the CLI command tree.
//! Called by: `cli.rs` when composing subcommands.
//! Calls into: Clap derive macros only.
//! Owns: positional arguments and reusable flag groups for scaffold commands.
//! Does not own: top-level command routing, validation side effects, or backend calls.
//! Next TODOs: add richer prompt/session flags once the engine contract and config format are finalized.
use std::path::PathBuf;

use clap::{Args, ValueEnum};

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

    /// Copy the completed assistant response to the system clipboard
    #[arg(long)]
    pub copy: bool,
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

    /// Copy the completed assistant response to the system clipboard
    #[arg(long)]
    pub copy: bool,
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, ValueEnum)]
pub enum PermissionMode {
    /// Inspect and propose changes without modifying files
    ReadOnly,
    /// Ask before applying every generated patch
    AskBeforeEdit,
    /// Apply validated workspace-local patches without an extra prompt
    WorkspaceWrite,
    /// Apply only conservative patches in a clean workspace, then run approved safe checks
    UnattendedSafe,
}

impl PermissionMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::AskBeforeEdit => "ask-before-edit",
            Self::WorkspaceWrite => "workspace-write",
            Self::UnattendedSafe => "unattended-safe",
        }
    }
}

#[derive(Debug, Clone, Args)]
pub struct CodeWorkspaceArgs {
    /// Repository or project directory; defaults to the current directory
    #[arg(long, short = 'p', value_name = "path", default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct CodeCheckpointArgs {
    /// Checkpoint identifier shown by `aegis code checkpoints`
    #[arg(value_name = "checkpoint-id")]
    pub id: String,

    /// Repository or project directory; defaults to the current directory
    #[arg(long, short = 'p', value_name = "path", default_value = ".")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Args)]
pub struct CodeTaskArgs {
    /// Coding task for AEGIS to investigate and implement
    #[arg(value_name = "task")]
    pub task: String,

    /// Repository or project directory; defaults to the current directory
    #[arg(long, short = 'p', value_name = "path", default_value = ".")]
    pub path: PathBuf,

    /// File access policy for generated changes
    #[arg(long, value_enum, default_value = "ask-before-edit")]
    pub permission: PermissionMode,

    /// Use deeper reasoning while investigating the task
    #[arg(long)]
    pub reason: bool,

    /// Print only the proposed unified diff
    #[arg(long, conflicts_with_all = ["json", "explain"])]
    pub diff_only: bool,

    /// Emit a machine-readable JSON result
    #[arg(long, conflicts_with_all = ["diff_only", "explain"])]
    pub json: bool,

    /// Suppress progress details and print only the final summary
    #[arg(long)]
    pub quiet: bool,

    /// Explain the proposed changes without applying them
    #[arg(long, conflicts_with_all = ["json", "diff_only"])]
    pub explain: bool,
}

#[derive(Debug, Clone, Args)]
pub struct CodeQueryArgs {
    /// File, symbol, subsystem, or question to investigate
    #[arg(value_name = "query")]
    pub query: Option<String>,

    #[arg(long, short = 'p', value_name = "path", default_value = ".")]
    pub path: PathBuf,

    /// Emit machine-readable JSON where supported
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct CodeFindArgs {
    /// Text or symbol to find across project files
    #[arg(value_name = "query")]
    pub query: String,

    #[arg(long, short = 'p', value_name = "path", default_value = ".")]
    pub path: PathBuf,

    /// Maximum number of matching lines
    #[arg(long, default_value_t = 50)]
    pub limit: usize,

    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Clone, Args)]
pub struct CodeTestArgs {
    /// Repository or project directory
    #[arg(long, short = 'p', value_name = "path", default_value = ".")]
    pub path: PathBuf,

    /// Run without an interactive confirmation
    #[arg(long)]
    pub yes: bool,

    /// Emit a JSON test-selection report without running commands
    #[arg(long)]
    pub json: bool,
}
