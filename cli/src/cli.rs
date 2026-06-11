use clap::{Parser, Subcommand};

use crate::args::{
    AskArgs, ChatArgs, InstallArgs, OptionalNameArg, OptionalSessionIdArg, ReplArgs,
    RequiredSessionIdArg, SaveArgs,
};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "aegis",
    version = "0.0.1",
    about = "AEGIS is your local privacy first tool!",
    long_about = None,
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
    #[command(alias = "select")]
    Switch(OptionalNameArg),
    Download(OptionalNameArg),
}
