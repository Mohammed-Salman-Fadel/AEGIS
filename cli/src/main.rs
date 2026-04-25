//! Role: crate entrypoint for the TODO-first AEGIS CLI scaffold.
//! Called by: the compiled `aegis` binary at process startup.
//! Calls into: `banner`, `commands`, `engine_client`, `ui`, and `workspace`.
//! Owns: `AppContext` construction, banner policy, and top-level error handling.
//! Does not own: command-specific behavior, backend orchestration, or installation logic.
//! Next TODOs: load real CLI config, persist CLI-local state pointers, and source the engine URL from shared config.

mod args;
mod banner;
mod cli;
mod commands;
mod doctor;
mod engine_client;
mod install;
mod menu;
mod runner;
mod signals;
mod ui;
mod user_profile;
mod workspace;

use clap::Parser;

use cli::Cli;
use engine_client::EngineClient;
use ui::Ui;
use workspace::Workspace;

pub(crate) type AppResult<T> = Result<T, String>;

#[derive(Debug, Clone)]
pub(crate) struct AppContext {
    pub ui: Ui,
    pub workspace: Workspace,
    // The engine client is the only planned backend connection surface for runtime commands.
    // Keep orchestration state behind the Rust engine instead of recreating it inside the CLI.
    pub engine: EngineClient,
}

fn main() {
    let cli = Cli::parse();
    let ctx = AppContext {
        ui: Ui::new(cli.no_color, cli.verbose),
        workspace: Workspace::discover(),
        engine: EngineClient::from_env(),
    };

    if let Err(error) = signals::install_handler() {
        eprintln!(
            "{}",
            ctx.ui.warning(&format!(
                "Warning: could not install the Ctrl+C exit handler: {error}"
            ))
        );
    }

    // The banner stays a presentation concern. Main decides whether it appears,
    // then hands off all command behavior to `commands.rs`.
    if banner::should_render_banner(cli.command.as_ref()) {
        ctx.ui.print_banner(banner::AEGIS_ASCII_ART);
    }

    if let Err(error) = commands::dispatch(&ctx, cli.command) {
        if signals::is_ctrl_c_error(&error) {
            ctx.ui
                .play_exit_animation("Ctrl+C received. Exiting AEGIS...");
        } else {
            eprintln!("{}", ctx.ui.error(&format!("Error: {error}")));
            std::process::exit(1);
        }
    }
}
