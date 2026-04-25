//! Role: shared Clap argument structs reused across the CLI command tree.
//! Called by: `cli.rs` when composing subcommands.
//! Calls into: Clap derive macros only.
//! Owns: positional arguments and reusable flag groups for scaffold commands.
//! Does not own: top-level command routing, validation side effects, or backend calls.
//! Next TODOs: add richer prompt/session flags once the engine contract and config format are finalized.
use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
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
