//! Role: public Clap command tree for the scaffolded AEGIS CLI.
//! Called by: `main.rs` during argument parsing.
//! Calls into: shared argument structs from `args.rs`.
//! Owns: user-facing command names, nesting, help text, and examples.
//! Does not own: command behavior, menus, backend calls, or installation steps.
//! Next TODOs: refine help text once the engine endpoints and installer flow stabilize.

use clap::{Parser, Subcommand};

use crate::args::{
    AskArgs, ChatArgs, InstallArgs, LogsArgs, OptionalNameArg, OptionalSessionIdArg, ReplArgs,
    RequiredSessionIdArg, SaveArgs, SearchArgs, SessionExportArgs, UpgradeArgs,
};

pub const HELP_EXAMPLES: &str = "\
Examples:
  aegis
  aegis open
  aegis install
  aegis install --path D:\\AEGIS
  aegis open
  aegis restart
  aegis setup
  aegis version
  aegis upgrade          (check + download latest from GitHub)
  aegis upgrade --check  (just check, don't download)
  aegis logs               (last 50 lines of all services)
  aegis logs engine        (last 50 lines of engine)
  aegis logs rag -n 200    (last 200 lines of RAG)
  aegis chat \"What can you do?\"
  aegis chat --attach notes.pdf \"Summarize this\"
  aegis load 1189578c-9c96-4b4c-8015-4d0673544a6a
  aegis repl
  aegis ask --stdin
  aegis session new
  aegis session search research
  aegis session resume
  aegis session export --format md
  aegis save \"my name is Sam\"
  aegis provider list
  aegis model
  aegis model list
  aegis model setup
  aegis model switch qwen3:4b
  aegis model download qwen3:4b
  aegis status
  aegis doctor";

#[derive(Debug, Clone, Parser)]
#[command(
    name = "aegis",
    version = "0.0.1",
    about = "AEGIS is your local privacy first tool!",
    long_about = None,
    after_help = HELP_EXAMPLES
)]

pub struct Cli {
    #[command(subcommand)]
    pub command: Option<CommandKind>,

    #[arg(
        long,
        global = true,
        help = "Show extra scaffold and diagnostics detail"
    )]
    pub verbose: bool,

    #[arg(long, global = true, help = "Disable ANSI colors")]
    pub no_color: bool,
}

#[derive(Debug, Clone, Subcommand)]
pub enum CommandKind {
    /// Start local AEGIS services, warm the active model, and open the Web UI.
    Open,
    Install(InstallArgs),
    Logs(LogsArgs),
    Restart,
    Setup,
    Version,
    Upgrade(UpgradeArgs),
    Save(SaveArgs),
    Chat(ChatArgs),
    Load(RequiredSessionIdArg),
    Ask(AskArgs),
    Repl(ReplArgs),
    Session {
        #[command(subcommand)]
        command: SessionCommand,
    },
    Provider {
        #[command(subcommand)]
        command: ProviderCommand,
    },
    Model {
        #[command(subcommand)]
        command: Option<ModelCommand>,
    },
    Status,
    Doctor {
        #[arg(long, help = "Exit with a non-zero status when blocking issues remain")]
        strict: bool,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum SessionCommand {
    New,
    List,
    Search(SearchArgs),
    Resume,
    Export(SessionExportArgs),
    Show(RequiredSessionIdArg),
    Use(OptionalSessionIdArg),
    #[command(alias = "reset")]
    Delete(RequiredSessionIdArg),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProviderCommand {
    List,
    Select(OptionalNameArg),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ModelCommand {
    List,
    Setup,
    #[command(alias = "select")]
    Switch(OptionalNameArg),
    Download(OptionalNameArg),
}
