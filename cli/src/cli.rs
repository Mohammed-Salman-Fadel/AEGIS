//! Role: public Clap command tree for the scaffolded AEGIS CLI.
//! Called by: `main.rs` during argument parsing.
//! Calls into: shared argument structs from `args.rs`.
//! Owns: user-facing command names, nesting, help text, and examples.
//! Does not own: command behavior, menus, backend calls, or installation steps.
//! Next TODOs: refine help text once the engine endpoints and installer flow stabilize.

use clap::{Parser, Subcommand};

use crate::args::{
    AskArgs, ChatArgs, InstallArgs, OptionalNameArg, OptionalSessionIdArg, ReplArgs,
    RequiredSessionIdArg,
};

pub const HELP_EXAMPLES: &str = "\
Examples:
  aegis
  aegis install
  aegis chat \"What can you do?\"
  aegis repl
  aegis ask --stdin
  aegis session new
  aegis provider list
  aegis model select mistral:7b
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
    Install(InstallArgs),
    Chat(ChatArgs),
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
        command: ModelCommand,
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
    Show(RequiredSessionIdArg),
    Use(OptionalSessionIdArg),
    Reset(RequiredSessionIdArg),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProviderCommand {
    List,
    Select(OptionalNameArg),
}

#[derive(Debug, Clone, Subcommand)]
pub enum ModelCommand {
    List,
    Select(OptionalNameArg),
}
