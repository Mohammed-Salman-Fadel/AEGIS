//! Role: the only command-dispatch layer below `main.rs`.
//! Called by: `main.rs` after parsing and banner rendering.
//! Calls into: `doctor`, `engine_client`, `install`, `menu`, `runner`, `ui`, and `workspace`.
//! Owns: thin handler stubs, placeholder output, and CLI-side validation for interactive selection.
//! Does not own: engine orchestration, session history, provider/model state, or dependency installation internals.
//! Next TODOs: replace placeholder prints with real HTTP calls and move repeated text into richer UI helpers.

use std::io::{self, IsTerminal, Read, Write};
use std::mem;

use clap::Parser;

use crate::banner;
use crate::cli::Cli;
use crate::cli::{CommandKind, ModelCommand, ProviderCommand, SessionCommand};
use crate::doctor::{CheckItem, DoctorReport, Health};
use crate::engine_client::{ActionStatus, ModelSummary, ProviderSummary, SessionSummary};
use crate::install;
use crate::menu::{self, MenuChoice};
use crate::runner;
use crate::signals;
use crate::{AppContext, AppResult};

pub fn dispatch(ctx: &AppContext, command: Option<CommandKind>) -> AppResult<()> {
    match command {
        None => {
            show_home(ctx)?;
            if io::stdin().is_terminal() {
                run_interactive_shell(ctx)?;
            }
            Ok(())
        }
        Some(command) => dispatch_command(ctx, command),
    }
}

fn dispatch_command(ctx: &AppContext, command: CommandKind) -> AppResult<()> {
    match command {
        CommandKind::Install(args) => handle_install(ctx, args),
        CommandKind::Chat(args) => handle_chat(ctx, &args.prompt, args.session_id.as_deref()),
        CommandKind::Ask(args) => handle_ask(ctx, args.stdin, args.session_id.as_deref()),
        CommandKind::Repl(args) => handle_repl(ctx, args.session_id.as_deref()),
        CommandKind::Session { command } => handle_session(ctx, command),
        CommandKind::Provider { command } => handle_provider(ctx, command),
        CommandKind::Model { command } => handle_model(ctx, command),
        CommandKind::Status => show_status(ctx),
        CommandKind::Doctor { strict } => show_doctor(ctx, strict),
    }
}

//? PRIMARY HOME INTERFACE
fn show_home(ctx: &AppContext) -> AppResult<()> {
    let report = DoctorReport::collect(&ctx.workspace);
    // ctx.ui.print_banner(banner::AEGIS_ASCII_ART);

    println!("{}", ctx.ui.header("AEGIS CLI"));
    println!("Private, local-first assistant scaffold built to stay inside the Rust CLI boundary.");
    // println!("{}", ctx.ui.muted("This pass is intentionally TODO-heavy: commands explain how the CLI should connect to the engine without pretending the backend wiring is finished."));
    println!();
    println!("Workspace : {}", ctx.workspace.root.display());
    println!("Localhost URL: {}", ctx.engine.base_url());
    // println!();
    // println!("{}", ctx.ui.header("Command Families"));
    // println!("- install");
    // println!("- chat");
    // println!("- ask --stdin");
    // println!("- repl");
    // println!("- session");
    // println!("- provider");
    // println!("- model");
    // println!("- status");
    // println!("- doctor");
    println!();
    println!("{}", ctx.ui.header("Readiness Snapshot"));
    println!(
        "{} blocking issue(s), {} warning(s), {} missing item(s)",
        report.blocking_issues(),
        report.warnings(),
        report.missing()
    );
    // println!("{}", ctx.ui.todo("TODO: once the engine endpoints are real, this home screen should show active session, provider, and model summaries from the backend."));
    if io::stdin().is_terminal() {
        println!();
        println!("{}", ctx.ui.header("Live Shell"));
        println!(
            "{}",
            ctx.ui
                .muted("Enter commands like `status`, `chat \"hello\"`, or `provider list`, type `help` for full commands list.")
        );
        println!(
            "{}",
            ctx.ui
                .muted("Type `quit` or `exit` to leave the shell. Or simply use Ctrl + C.")
        );
    }
    Ok(())
}

fn handle_clear(ctx: &AppContext) -> AppResult<()> {
    ctx.ui.print_banner(banner::AEGIS_ASCII_ART);

    println!("{}", ctx.ui.header("AEGIS CLI"));
    // println!();
    println!("Workspace : {}", ctx.workspace.root.display());
    println!("Localhost URL: {}", ctx.engine.base_url());

    if io::stdin().is_terminal() {
        println!();
        // println!("{}", ctx.ui.header("Live Shell"));
        println!(
            "{}",
            ctx.ui.muted("Enter commands `chat \"hello\"`, or `provider list`, type `help` for full commands list.")
        );
        println!();
    }
    Ok(())
}

fn handle_install(ctx: &AppContext, args: crate::args::InstallArgs) -> AppResult<()> {
    let plan = install::build_install_plan(&ctx.workspace);
    install::print_install_plan(&ctx.ui, &plan);
    println!();

    if args.yes && !args.plan_only {
        println!("{}", ctx.ui.warning("TODO: map each install step to runner.rs subprocess plans before `--yes` performs system changes."));
    } else {
        println!(
            "{}",
            ctx.ui
                .muted("Scaffold mode: no dependency installation is executed yet.")
        );
    }

    Ok(())
}

fn handle_chat(ctx: &AppContext, prompt: &str, session_id: Option<&str>) -> AppResult<()> {
    println!("{}", ctx.ui.header("Chat"));
    let reply = ctx.engine.chat(prompt, session_id)?;
    if ctx.ui.verbose {
        println!();
        println!("Endpoint: {}", reply.request_path);
    }
    Ok(())
}

fn handle_ask(ctx: &AppContext, read_from_stdin: bool, session_id: Option<&str>) -> AppResult<()> {
    println!("{}", ctx.ui.header("Ask Scaffold"));

    if !read_from_stdin {
        println!(
            "{}",
            ctx.ui
                .warning("`aegis ask` currently expects `--stdin` in this scaffold.")
        );
        println!("{}", ctx.ui.todo("TODO: keep stdin as the single prompt source for `ask`, or add a positional fallback deliberately."));
        return Ok(());
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input).map_err(|error| {
        if signals::was_ctrl_c(&error) {
            signals::ctrl_c_exit_error()
        } else {
            format!("Could not read stdin: {error}")
        }
    })?;

    let prompt = input.trim();
    if prompt.is_empty() {
        println!("{}", ctx.ui.warning("No stdin content was provided."));
        println!(
            "{}",
            ctx.ui
                .todo("TODO: add better empty-input guidance once the UX is finalized.")
        );
        return Ok(());
    }

    let reply = ctx.engine.chat_from_stdin(prompt, session_id)?;
    println!("{}", reply.message);
    println!("Endpoint: {}", reply.request_path);
    Ok(())
}

fn handle_repl(ctx: &AppContext, session_id: Option<&str>) -> AppResult<()> {
    println!("{}", ctx.ui.header("REPL Scaffold"));
    println!("{}", ctx.ui.muted("Type `exit` or `quit` to leave, or press Ctrl+C to stop the whole CLI. Each turn is still a placeholder HTTP call description."));
    println!("{}", ctx.ui.todo("TODO: keep session ownership in the engine and use a local active-session pointer only as a convenience hint."));

    if !io::stdin().is_terminal() {
        println!(
            "{}",
            ctx.ui
                .warning("The scaffold REPL expects an interactive terminal.")
        );
        return Ok(());
    }

    loop {
        print!("aegis> ");
        io::stdout()
            .flush()
            .map_err(|error| format!("Could not flush REPL prompt: {error}"))?;

        let mut input = String::new();
        let bytes = match io::stdin().read_line(&mut input) {
            Ok(bytes) => bytes,
            Err(error) if signals::was_ctrl_c(&error) => {
                return Err(signals::ctrl_c_exit_error());
            }
            Err(error) => return Err(format!("Could not read REPL input: {error}")),
        };

        if bytes == 0 {
            println!();
            break;
        }

        let prompt = input.trim();
        if prompt.is_empty() {
            continue;
        }
        if matches!(prompt, "exit" | "quit") {
            break;
        }

        let reply = ctx.engine.repl_turn(prompt, session_id)?;
        println!("{}", reply.message);
    }

    Ok(())
}

// Interactive shell logic
fn run_interactive_shell(ctx: &AppContext) -> AppResult<()> {
    println!();

    // Shell command loop
    loop {
        print!("aegis-shell> ");
        io::stdout()
            .flush()
            .map_err(|error| format!("Could not flush shell prompt: {error}"))?;

        let mut input = String::new();
        let bytes = match io::stdin().read_line(&mut input) {
            Ok(bytes) => bytes,
            Err(error) if signals::was_ctrl_c(&error) => {
                return Err(signals::ctrl_c_exit_error());
            }
            Err(error) => return Err(format!("Could not read shell input: {error}")),
        };

        if bytes == 0 {
            println!();
            break;
        }

        let line = input.trim();
        if line.is_empty() {
            continue;
        }

        match line {
            "exit" | "quit" => {
                println!("{}", ctx.ui.muted("Leaving the interactive shell."));
                break;
            }
            "help" => {
                print_shell_help(ctx);
                continue;
            }
            "banner" => {
                ctx.ui.print_banner(banner::AEGIS_ASCII_ART);
                continue;
            }
            "clear" => {
                ctx.ui.clear_screen();
                handle_clear(ctx)?;
                continue;
            }
            "home" => {
                show_home(ctx)?;
                continue;
            }
            _ => {}
        }

        let parsed = match parse_shell_cli(line) {
            Ok(Some(parsed)) => parsed,
            Ok(None) => continue,
            Err(error) => {
                eprintln!("{}", ctx.ui.error(&error));
                continue;
            }
        };

        if let Some(command) = parsed.command {
            if matches!(&command, CommandKind::Ask(args) if args.stdin) {
                println!(
                    "{}",
                    ctx.ui.warning(
                        "`ask --stdin` should be run directly from the terminal, not from inside the live shell."
                    )
                );
                println!(
                    "{}",
                    ctx.ui.muted(
                        "Use `chat \"prompt\"` here, or run `aegis ask --stdin` outside the shell."
                    )
                );
                continue;
            }

            if banner::should_render_banner(Some(&command)) {
                ctx.ui.print_banner(banner::AEGIS_ASCII_ART);
            }

            if let Err(error) = dispatch_command(ctx, command) {
                if signals::is_ctrl_c_error(&error) {
                    return Err(error);
                }

                eprintln!("{}", ctx.ui.error(&format!("Error: {error}")));
            }
        } else {
            show_home(ctx)?;
        }
    }

    Ok(())
}

fn print_shell_help(ctx: &AppContext) {
    println!("{}", ctx.ui.header("Aegis Help"));
    println!("You can run the following commands without the `aegis` prefix:");
    // println!("Examples:");
    println!("- status");
    println!("- chat \"hello\"");
    println!("- repl");
    println!("- session list");
    println!("- provider select ollama");
    println!("- model list");
    println!("");
    println!("Built-ins:");
    println!("- help");
    println!("- home");
    println!("- banner");
    println!("- clear");
    println!("- quit");
    println!(
        "{}",
        ctx.ui
            .muted("Type `quit` or `exit` at any time to stop the CLI immediately.")
    );
}

fn parse_shell_cli(line: &str) -> AppResult<Option<Cli>> {
    let mut tokens = tokenize_command_line(line)?;
    if tokens.is_empty() {
        return Ok(None);
    }

    if matches!(
        tokens.first().map(String::as_str),
        Some("aegis" | "aegis.exe")
    ) {
        tokens.remove(0);
    }

    let args = std::iter::once("aegis".to_string())
        .chain(tokens)
        .collect::<Vec<_>>();

    Cli::try_parse_from(args)
        .map(Some)
        .map_err(|error| error.to_string().trim().to_string())
}

fn tokenize_command_line(line: &str) -> AppResult<Vec<String>> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut active_quote = None;
    let mut escaping = false;

    for ch in line.chars() {
        if escaping {
            current.push(ch);
            escaping = false;
            continue;
        }

        match ch {
            '\\' if active_quote != Some('\'') => escaping = true,
            '"' | '\'' if active_quote == Some(ch) => active_quote = None,
            '"' | '\'' if active_quote.is_none() => active_quote = Some(ch),
            _ if ch.is_whitespace() && active_quote.is_none() => {
                if !current.is_empty() {
                    tokens.push(mem::take(&mut current));
                }
            }
            _ => current.push(ch),
        }
    }

    if escaping {
        current.push('\\');
    }

    if active_quote.is_some() {
        return Err("The shell command has an unclosed quote.".to_string());
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

fn handle_session(ctx: &AppContext, command: SessionCommand) -> AppResult<()> {
    // Sessions are engine-owned. The CLI should only request operations and maybe
    // remember a lightweight "active session" pointer later for convenience.
    match command {
        SessionCommand::New => {
            println!("{}", ctx.ui.header("Session New"));
            print_action_status(ctx, ctx.engine.create_session()?);
        }
        SessionCommand::List => {
            println!("{}", ctx.ui.header("Session List"));
            let sessions = ctx.engine.list_sessions()?;
            print_sessions(ctx, &sessions);
        }
        SessionCommand::Show(args) => {
            println!("{}", ctx.ui.header("Session Show"));
            let detail = ctx.engine.show_session(&args.id)?;
            println!("Session : {}", detail.id);
            println!("Title   : {}", detail.title);
            println!("Note    : {}", detail.note);
            for turn in detail.recent_turns {
                println!("- {}", turn);
            }
        }
        SessionCommand::Use(args) => {
            println!("{}", ctx.ui.header("Session Use"));
            if let Some(session_id) = args.id {
                print_action_status(ctx, ctx.engine.use_session(&session_id)?);
            } else {
                handle_interactive_session_use(ctx)?;
            }
        }
        SessionCommand::Reset(args) => {
            println!("{}", ctx.ui.header("Session Reset"));
            print_action_status(ctx, ctx.engine.reset_session(&args.id)?);
        }
    }

    Ok(())
}

fn handle_provider(ctx: &AppContext, command: ProviderCommand) -> AppResult<()> {
    // Provider selection belongs in the backend so the CLI, GUI, and other clients
    // can share the same active runtime configuration.
    match command {
        ProviderCommand::List => {
            println!("{}", ctx.ui.header("Provider List"));
            let providers = ctx.engine.list_providers()?;
            print_providers(ctx, &providers);
        }
        ProviderCommand::Select(args) => {
            println!("{}", ctx.ui.header("Provider Select"));
            if let Some(name) = args.name {
                print_action_status(ctx, ctx.engine.select_provider(&name)?);
            } else {
                handle_interactive_provider_select(ctx)?;
            }
        }
    }

    Ok(())
}

fn handle_model(ctx: &AppContext, command: ModelCommand) -> AppResult<()> {
    // Model selection follows the same rule as provider selection:
    // ask the engine to own it instead of persisting model state only in the CLI.
    match command {
        ModelCommand::List => {
            println!("{}", ctx.ui.header("Model List"));
            let models = ctx.engine.list_models()?;
            print_models(ctx, &models);
        }
        ModelCommand::Select(args) => {
            println!("{}", ctx.ui.header("Model Select"));
            if let Some(name) = args.name {
                print_action_status(ctx, ctx.engine.select_model(&name)?);
            } else {
                handle_interactive_model_select(ctx)?;
            }
        }
    }

    Ok(())
}

//? HANDLES "STATUS" COMMAND
fn show_status(ctx: &AppContext) -> AppResult<()> {
    let report = DoctorReport::collect(&ctx.workspace);
    let health = ctx.engine.health();

    println!("{}", ctx.ui.header("Status"));
    println!("Workspace root : {}", ctx.workspace.root.display());
    println!("Localhost URL     : {}", health.base_url);
    println!("Health path    : {}", health.request_path);
    println!(
        "Engine ready   : {}",
        if health.reachable { "yes" } else { "no" }
    );
    println!("Health note    : {}", health.note);
    println!(
        "CLI target dir : {}",
        ctx.workspace.cli_build_target_dir(false).display()
    );
    println!();
    println!("{}", ctx.ui.header("Components"));
    for component in ctx.workspace.components() {
        println!(
            "{} {:<10} {}",
            ctx.ui.component_badge(component.state),
            component.name,
            component.note
        );
        if ctx.ui.verbose {
            println!(
                "{} {}",
                ctx.ui.muted("           path:"),
                component.path.display()
            );
        }
    }
    println!();
    println!("{}", ctx.ui.header("Doctor Snapshot"));
    println!(
        "{} blocking issue(s), {} warning(s), {} missing item(s)",
        report.blocking_issues(),
        report.warnings(),
        report.missing()
    );
    if let Some(plan) = runner::engine_launch_plan(&ctx.workspace) {
        println!("Engine start preview: {}", plan.command_preview());
    }
    println!("{}", ctx.ui.todo("TODO: query the engine `/health` endpoint and report live provider/model/session state."));
    Ok(())
}

fn show_doctor(ctx: &AppContext, strict: bool) -> AppResult<()> {
    let report = DoctorReport::collect(&ctx.workspace);

    println!("{}", ctx.ui.header("Doctor"));
    println!("Workspace: {}", ctx.workspace.root.display());
    println!();
    println!("{}", ctx.ui.header("Dependencies"));
    for item in &report.dependencies {
        print_check(ctx, item);
    }
    println!();
    println!("{}", ctx.ui.header("Components"));
    for item in &report.components {
        print_check(ctx, item);
    }
    println!();
    println!("{}", ctx.ui.header("Summary"));
    println!(
        "{} blocking issue(s), {} warning(s), {} missing item(s)",
        report.blocking_issues(),
        report.warnings(),
        report.missing()
    );
    println!();
    println!("{}", ctx.ui.header("Next TODOs"));
    for (index, action) in report.setup_actions().iter().enumerate() {
        println!("{}. {}", index + 1, action);
    }

    if strict && report.blocking_issues() > 0 {
        return Err(format!(
            "Strict doctor failed because {} blocking issue(s) still remain.",
            report.blocking_issues()
        ));
    }

    Ok(())
}

fn handle_interactive_session_use(ctx: &AppContext) -> AppResult<()> {
    if !io::stdin().is_terminal() {
        println!("{}", ctx.ui.warning("No session id was provided."));
        println!(
            "{}",
            ctx.ui
                .muted("Use `aegis session use <id>` in non-interactive environments.")
        );
        return Ok(());
    }

    let sessions = ctx.engine.list_sessions()?;
    // The menu only renders options. It should never invent or persist session data itself.
    let choices: Vec<MenuChoice> = sessions
        .iter()
        .map(|session| {
            MenuChoice::new(
                format!("{} ({})", session.title, session.id),
                session.id.clone(),
                session.description.clone(),
            )
        })
        .collect();

    match menu::choose_from_stdin(
        &ctx.ui,
        "Choose a session",
        "Select a session number: ",
        &choices,
    )? {
        Some(choice) => print_action_status(ctx, ctx.engine.use_session(&choice.value)?),
        None => println!("{}", ctx.ui.warning("No session was selected.")),
    }

    Ok(())
}

fn handle_interactive_provider_select(ctx: &AppContext) -> AppResult<()> {
    if !io::stdin().is_terminal() {
        println!("{}", ctx.ui.warning("No provider name was provided."));
        println!(
            "{}",
            ctx.ui
                .muted("Use `aegis provider select <name>` in non-interactive environments.")
        );
        return Ok(());
    }

    let providers = ctx.engine.list_providers()?;
    // The selectable values come from the engine client placeholder today and from
    // the future `/providers` endpoint later.
    let choices: Vec<MenuChoice> = providers
        .iter()
        .map(|provider| MenuChoice::new(&provider.name, &provider.name, &provider.description))
        .collect();

    match menu::choose_from_stdin(
        &ctx.ui,
        "Choose a provider",
        "Select a provider number: ",
        &choices,
    )? {
        Some(choice) => print_action_status(ctx, ctx.engine.select_provider(&choice.value)?),
        None => println!("{}", ctx.ui.warning("No provider was selected.")),
    }

    Ok(())
}

fn handle_interactive_model_select(ctx: &AppContext) -> AppResult<()> {
    if !io::stdin().is_terminal() {
        println!("{}", ctx.ui.warning("No model name was provided."));
        println!(
            "{}",
            ctx.ui
                .muted("Use `aegis model select <name>` in non-interactive environments.")
        );
        return Ok(());
    }

    let models = ctx.engine.list_models()?;
    // Keep this menu generic so it can be reused once the real model catalog arrives.
    let choices: Vec<MenuChoice> = models
        .iter()
        .map(|model| {
            MenuChoice::new(
                format!("{} via {}", model.name, model.provider),
                model.name.clone(),
                model.description.clone(),
            )
        })
        .collect();

    match menu::choose_from_stdin(
        &ctx.ui,
        "Choose a model",
        "Select a model number: ",
        &choices,
    )? {
        Some(choice) => print_action_status(ctx, ctx.engine.select_model(&choice.value)?),
        None => println!("{}", ctx.ui.warning("No model was selected.")),
    }

    Ok(())
}

fn print_sessions(ctx: &AppContext, sessions: &[SessionSummary]) {
    if sessions.is_empty() {
        println!("{}", ctx.ui.warning("No sessions are available yet."));
        return;
    }

    for session in sessions {
        println!("- {} [{}]", session.title, session.id);
        println!("  {}", session.description);
    }
}

fn print_providers(ctx: &AppContext, providers: &[ProviderSummary]) {
    if providers.is_empty() {
        println!("{}", ctx.ui.warning("No providers are available yet."));
        return;
    }

    for provider in providers {
        println!("- {}", provider.name);
        println!("  {}", provider.description);
    }
}

fn print_models(ctx: &AppContext, models: &[ModelSummary]) {
    if models.is_empty() {
        println!("{}", ctx.ui.warning("No models are available yet."));
        return;
    }

    for model in models {
        println!("- {} [{}]", model.name, model.provider);
        println!("  {}", model.description);
    }
}

fn print_action_status(ctx: &AppContext, status: ActionStatus) {
    println!("Target   : {}", status.target);
    println!("Endpoint : {}", status.request_path);
    println!("Persisted: {}", if status.persisted { "yes" } else { "no" });
    println!("{}", status.message);
    println!("{}", ctx.ui.todo("TODO: confirm these actions with real engine endpoints instead of placeholder acceptance messages."));
}

fn print_check(ctx: &AppContext, item: &CheckItem) {
    println!(
        "{} {:<18} {}",
        ctx.ui.badge(item.health),
        item.name,
        item.detail
    );
    if ctx.ui.verbose || matches!(item.health, Health::Warn | Health::Missing) {
        if let Some(guidance) = &item.guidance {
            println!("{} {}", ctx.ui.muted("                   next:"), guidance);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_quoted_shell_commands() {
        let tokens = tokenize_command_line("chat \"hello world\"").unwrap();
        assert_eq!(tokens, vec!["chat", "hello world"]);
    }

    #[test]
    fn tokenizes_prefixed_shell_commands() {
        let parsed = parse_shell_cli("aegis session use todo-session-001").unwrap();
        let Some(cli) = parsed else {
            panic!("shell parser should produce a CLI command");
        };

        match cli.command {
            Some(CommandKind::Session {
                command: SessionCommand::Use(args),
            }) => assert_eq!(args.id.as_deref(), Some("todo-session-001")),
            _ => panic!("expected session use command"),
        }
    }
}
