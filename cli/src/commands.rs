//! Role: the only command-dispatch layer below `main.rs`.
//! Called by: `main.rs` after parsing and banner rendering.
//! Calls into: `doctor`, `engine_client`, `install`, `menu`, `runner`, `ui`, and `workspace`.
//! Owns: thin handler stubs, placeholder output, and CLI-side validation for interactive selection.
//! Does not own: engine orchestration, session history, provider/model state, or dependency installation internals.
//! Next TODOs: replace placeholder prints with real HTTP calls and move repeated text into richer UI helpers.

use std::fs;
use std::io::{self, IsTerminal, Read, Write};
use std::mem;
use std::path::PathBuf;

use clap::Parser;
use crossterm::cursor::{MoveDown, MoveToColumn, MoveUp};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    MouseButton, MouseEventKind,
};
use crossterm::style::{Attribute, Print, SetAttribute};
use crossterm::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode};
use crossterm::{execute, queue};

use crate::banner;
use crate::cli::Cli;
use crate::cli::{CommandKind, ModelCommand, ProviderCommand, SessionCommand};
use crate::doctor::{CheckItem, DoctorReport, Health};
use crate::engine_client::{
    ActionStatus, CreatedSession, ModelSummary, ProviderSummary, SessionSummary,
};
use crate::install;
use crate::menu::{self, MenuChoice};
use crate::runner;
use crate::signals;
use crate::user_profile;
use crate::{AppContext, AppResult};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InvocationMode {
    Direct,
    Shell,
}

enum SessionPromptInput {
    Submit(String),
    Tool(String),
    Eof,
}

pub fn dispatch(ctx: &AppContext, command: Option<CommandKind>) -> AppResult<()> {
    match command {
        None => {
            show_home(ctx)?;
            if io::stdin().is_terminal() {
                run_interactive_shell(ctx)?;
            }
            Ok(())
        }
        Some(command) => dispatch_command(ctx, command, InvocationMode::Direct),
    }
}

fn dispatch_command(
    ctx: &AppContext,
    command: CommandKind,
    invocation_mode: InvocationMode,
) -> AppResult<()> {
    match command {
        CommandKind::Install(args) => handle_install(ctx, args),
        CommandKind::Save(args) => handle_save(ctx, &args.note),
        CommandKind::Chat(args) => handle_chat(ctx, &args.prompt, args.session_id.as_deref()),
        CommandKind::Load(args) => handle_load(ctx, &args.id, invocation_mode),
        CommandKind::Ask(args) => handle_ask(ctx, args.stdin, args.session_id.as_deref()),
        CommandKind::Repl(args) => handle_repl(ctx, args.session_id.as_deref()),
        CommandKind::Session { command } => handle_session(ctx, command, invocation_mode),
        CommandKind::Provider { command } => handle_provider(ctx, command),
        CommandKind::Model { command } => handle_model(ctx, command),
        CommandKind::Status => show_status(ctx),
        CommandKind::Doctor { strict } => show_doctor(ctx, strict),
    }
}

//? PRIMARY HOME INTERFACE
fn show_home(ctx: &AppContext) -> AppResult<()> {
    let report = DoctorReport::collect(&ctx.workspace);
    let web_ui_url = ctx.workspace.web_ui_url();
    println!("{}", ctx.ui.header("AEGIS CLI"));
    println!("Private, local-first assistant built to serve only you.");
    println!();
    println!("Workspace : {}", ctx.workspace.root.display());
    println!("Web UI URL: {web_ui_url}");
    println!();
    println!("{}", ctx.ui.header("Readiness Snapshot"));
    println!(
        "{} blocking issue(s), {} warning(s), {} missing item(s)",
        report.blocking_issues(),
        report.warnings(),
        report.missing()
    );
    if io::stdin().is_terminal() {
        println!();
        println!("{}", ctx.ui.header("Live Shell"));
        println!(
            "{}",
            ctx.ui.muted(
                "Explore the command list using 'help'. Type `quit` or `exit` to leave the shell."
            )
        );
    }
    Ok(())
}

fn handle_clear(ctx: &AppContext) -> AppResult<()> {
    ctx.print_banner();

    // println!("{}", ctx.ui.header("AEGIS CLI"));
    // println!();
    // println!("Workspace : {}", ctx.workspace.root.display());
    // println!("Web UI URL: {web_ui_url}");

    // if io::stdin().is_terminal() {
    //     println!();
    //     // println!("{}", ctx.ui.header("Live Shell"));
    //     println!(
    //         "{}",
    //         ctx.ui.muted("Enter commands `chat \"hello\"`, or `provider list`, type `help` for full commands list.")
    //     );
    //     println!();
    // }
    Ok(())
}

fn handle_install(ctx: &AppContext, args: crate::args::InstallArgs) -> AppResult<()> {
    let (install_root, install_root_source) = if let Some(path) = args.path.as_deref() {
        (
            crate::workspace::Workspace::normalize_install_root(path),
            "--path".to_string(),
        )
    } else if std::env::var_os("AEGIS_INSTALL_ROOT").is_some() {
        (
            ctx.workspace.install_root.clone(),
            "AEGIS_INSTALL_ROOT".to_string(),
        )
    } else if ctx.workspace.install_root != ctx.workspace.default_install_root {
        (
            ctx.workspace.install_root.clone(),
            "saved preference".to_string(),
        )
    } else {
        (ctx.workspace.install_root.clone(), "default".to_string())
    };

    let plan = install::build_install_plan(
        &ctx.workspace,
        install_root.clone(),
        install_root_source.clone(),
    );
    install::print_install_plan(&ctx.ui, &plan);
    println!();

    if args.yes && !args.plan_only {
        if args.path.is_some() {
            install::persist_install_root(&ctx.ui, &install_root)?;
            println!();
        }
        println!("{}", ctx.ui.warning("TODO: map each install step to runner.rs subprocess plans before `--yes` performs system changes."));
    } else {
        if args.path.is_some() && !args.plan_only {
            install::persist_install_root(&ctx.ui, &install_root)?;
            println!();
        }
        println!(
            "{}",
            ctx.ui
                .muted("Scaffold mode: no dependency installation is executed yet.")
        );
    }

    Ok(())
}

fn handle_save(ctx: &AppContext, note: &str) -> AppResult<()> {
    let path = user_profile::append_note(note)?;
    println!("{}", ctx.ui.header("Personalization Saved"));
    println!(
        "{}",
        ctx.ui.success("Saved your note for future responses.")
    );
    println!(
        "{}",
        ctx.ui
            .muted("AEGIS will feed this information to the model when it is relevant.")
    );
    if ctx.ui.verbose {
        println!("File: {}", path.display());
    }
    Ok(())
}

fn handle_chat(ctx: &AppContext, prompt: &str, session_id: Option<&str>) -> AppResult<()> {
    let reply = stream_llm_response(ctx, |on_token| {
        ctx.engine.chat(prompt, session_id, on_token)
    })?;
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

    let reply = stream_llm_response(ctx, |on_token| {
        ctx.engine.chat_from_stdin(prompt, session_id, on_token)
    })?;
    if ctx.ui.verbose {
        println!("Endpoint: {}", reply.request_path);
    }
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

        let _reply = stream_llm_response(ctx, |on_token| {
            ctx.engine.repl_turn(prompt, session_id, on_token)
        })?;
    }

    Ok(())
}

fn stream_llm_response<F>(
    ctx: &AppContext,
    operation: F,
) -> AppResult<crate::engine_client::ChatReply>
where
    F: FnOnce(&mut dyn FnMut(&str) -> AppResult<()>) -> AppResult<crate::engine_client::ChatReply>,
{
    let mut loading = Some(ctx.ui.start_loading_animation("Calling LLM"));
    let mut saw_token = false;
    let mut renderer = ctx.ui.streamed_markdown_renderer();
    let mut on_token = |token: &str| -> AppResult<()> {
        if !saw_token {
            if let Some(active) = loading.take() {
                active.finish();
            }
            println!();
            saw_token = true;
        }

        renderer
            .push(token)
            .map_err(|error| format!("Could not flush streamed response: {error}"))?;
        Ok(())
    };

    let result = operation(&mut on_token);
    drop(on_token);

    if let Some(active) = loading.take() {
        active.finish();
    }

    match result {
        Ok(reply) => {
            if saw_token {
                renderer
                    .finish()
                    .map_err(|error| format!("Could not finish streamed response: {error}"))?;
                println!();
            } else {
                ctx.ui.print_markdownish_response(&reply.message);
            }
            Ok(reply)
        }
        Err(error) => {
            if saw_token {
                let _ = renderer.finish();
                println!();
            }
            Err(error)
        }
    }
}

// Interactive shell logic
fn run_interactive_shell(ctx: &AppContext) -> AppResult<()> {
    println!();

    // Shell command loop
    loop {
        print!("{} ", ctx.ui.info("aegis-shell>"));
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
                ctx.print_banner();
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
                ctx.print_banner();
            }

            if let Err(error) = dispatch_command(ctx, command, InvocationMode::Shell) {
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
    println!(" status                          reveal current status of the system");
    println!(" chat     \"[user_prompt]\"        one-time prompt to the llm");
    println!(" load      [session_id]          load previous sessions");
    println!(" save      [your_information]    can store personal information");
    println!(" session   [argument]            session commands");
    println!(" provider  [argument]            provider related argument");
    println!(" model     [argument]            model related command");
    println!("");
    println!(
        " [command] --help                displays all the arguments you can pass into a command."
    );
    println!("");
    println!(
        "{}",
        ctx.ui
            .muted("Type `quit` or `exit` at any time to stop the CLI immediately.")
    );
}

fn handle_load(
    ctx: &AppContext,
    session_id: &str,
    invocation_mode: InvocationMode,
) -> AppResult<()> {
    println!("{}", ctx.ui.header("Session Load"));
    enter_session(ctx, session_id, invocation_mode)
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

fn handle_session(
    ctx: &AppContext,
    command: SessionCommand,
    invocation_mode: InvocationMode,
) -> AppResult<()> {
    // Sessions are engine-owned. The CLI should only request operations and maybe
    // remember a lightweight "active session" pointer later for convenience.
    match command {
        SessionCommand::New => {
            println!("{}", ctx.ui.header("Session New"));
            let session = ctx.engine.create_session()?;
            let session_id = session.id.clone();
            print_created_session(ctx, session);
            if io::stdin().is_terminal() {
                run_session_prompt_loop(ctx, &session_id, invocation_mode)?;
            }
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
                enter_session(ctx, &session_id, invocation_mode)?;
            } else {
                handle_interactive_session_use(ctx, invocation_mode)?;
            }
        }
        SessionCommand::Delete(args) => {
            println!("{}", ctx.ui.header("Session Delete"));
            print_action_status(ctx, ctx.engine.delete_session(&args.id)?);
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
            let active_provider = ctx.engine.current_provider().ok();
            print_providers(ctx, &providers, active_provider.as_deref());
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

fn handle_provider_select(ctx: &AppContext, name: &str) -> AppResult<()> {
    let result = ctx.engine.select_provider(name)?;
    print_action_status(ctx, result);
    Ok(())
}

fn handle_model(ctx: &AppContext, command: Option<ModelCommand>) -> AppResult<()> {
    // Model selection follows the same rule as provider selection:
    // ask the engine to own it instead of persisting model state only in the CLI.
    match command {
        None => {
            println!("{}", ctx.ui.header("Current Model"));
            println!("Using : {}", ctx.engine.current_model()?);
        }
        Some(ModelCommand::List) => {
            println!("{}", ctx.ui.header("Model List"));
            let models = ctx.engine.list_models()?;
            let current_model = ctx.engine.current_model().ok();
            print_models(ctx, &models, current_model.as_deref());
        }
        Some(ModelCommand::Switch(args)) => {
            println!("{}", ctx.ui.header("Model Switch"));
            if let Some(name) = args.name {
                handle_model_switch(ctx, &name)?;
            } else {
                handle_interactive_model_select(ctx)?;
            }
        }
        Some(ModelCommand::Download(args)) => {
            println!("{}", ctx.ui.header("Model Download"));
            if let Some(name) = args.name {
                handle_model_download(ctx, &name)?;
            } else {
                println!("{}", ctx.ui.warning("No model name was provided."));
            }
        }
    }

    Ok(())
}

//? HANDLES "STATUS" COMMAND
fn show_status(ctx: &AppContext) -> AppResult<()> {
    let report = DoctorReport::collect(&ctx.workspace);
    let health = ctx.engine.health();
    let web_ui_url = ctx.workspace.web_ui_url();

    println!("{}", ctx.ui.header("Status"));
    println!("Workspace root : {}", ctx.workspace.root.display());
    println!("Web UI URL     : {}", web_ui_url);
    println!("Engine URL     : {}", health.base_url);
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
    let report = DoctorReport::collect_live(&ctx.workspace, &ctx.engine);

    println!("{}", ctx.ui.header("Doctor"));
    println!("Workspace: {}", ctx.workspace.root.display());
    println!();
    println!("{}", ctx.ui.header("Dependencies"));
    for item in &report.dependencies {
        print_check(ctx, item);
    }
    if !report.runtime.is_empty() {
        println!();
        println!("{}", ctx.ui.header("Runtime"));
        for item in &report.runtime {
            print_check(ctx, item);
        }
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

fn handle_interactive_session_use(
    ctx: &AppContext,
    invocation_mode: InvocationMode,
) -> AppResult<()> {
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
        Some(choice) => enter_session(ctx, &choice.value, invocation_mode)?,
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
                .muted("Use `aegis model switch <name>` in non-interactive environments.")
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
        Some(choice) => handle_model_switch(ctx, &choice.value)?,
        None => println!("{}", ctx.ui.warning("No model was selected.")),
    }

    Ok(())
}

fn handle_model_download(ctx: &AppContext, model_name: &str) -> AppResult<()> {
    if ctx.engine.current_provider()? != "lmstudio" {
        println!("{}", ctx.ui.warning("Model downloads are intended for LM Studio in this build. Switch to `provider select lmstudio` first."));
        return Ok(());
    }

    println!("{}", ctx.ui.muted("LM Studio exposes model download through its OpenAI-compatible server; the engine currently switches providers but does not yet proxy the LM Studio download API."));
    println!("Requested model: {model_name}");
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

fn print_providers(
    ctx: &AppContext,
    providers: &[ProviderSummary],
    current_provider: Option<&str>,
) {
    if providers.is_empty() {
        println!("{}", ctx.ui.warning("No providers are available yet."));
        return;
    }

    for provider in providers {
        if provider.name.eq_ignore_ascii_case("openai-compatible")
            || provider.name.eq_ignore_ascii_case("openai-compat")
        {
            continue;
        }

        let active = current_provider
            .map(|current| current.eq_ignore_ascii_case(&provider.name))
            .unwrap_or(false);
        println!(
            "- {}{}",
            provider.name,
            if active { " (active)" } else { "" }
        );
        println!("  {}", provider.description);
    }
}

fn print_models(ctx: &AppContext, models: &[ModelSummary], current_model: Option<&str>) {
    if models.is_empty() {
        println!("{}", ctx.ui.warning("No models are available yet."));
        return;
    }

    for model in models {
        let active = current_model
            .map(|current| current.eq_ignore_ascii_case(&model.name))
            .unwrap_or(false);
        println!(
            "- {} [{}]{}",
            model.name,
            model.provider,
            if active { " (active)" } else { "" }
        );
    }
}

fn handle_model_switch(ctx: &AppContext, new_model: &str) -> AppResult<()> {
    let available_models = ctx.engine.list_models()?;
    let model_exists = available_models
        .iter()
        .any(|model| model.name.eq_ignore_ascii_case(new_model));

    if !model_exists {
        return Err(format!(
            "Model `{new_model}` is not available for the active provider. Run `model list` to see available models."
        ));
    }

    let current_model = ctx.engine.current_model()?;
    if current_model.eq_ignore_ascii_case(new_model) {
        println!(
            "{}",
            ctx.ui
                .muted(&format!("`{new_model}` is already the active model."))
        );
        return Ok(());
    }

    let switch_result =
        run_with_loading_message(ctx, &format!("Now switching to {new_model}"), || {
            ctx.engine.select_model(new_model)
        })
        .map_err(|error| {
            format!(
                "Could not switch to `{new_model}`. The previous model is still active. {error}"
            )
        })?;

    println!("{}", ctx.ui.success(&switch_result.message));
    Ok(())
}

fn enter_session(
    ctx: &AppContext,
    session_id: &str,
    invocation_mode: InvocationMode,
) -> AppResult<()> {
    let detail = ctx.engine.show_session(session_id)?;

    if !io::stdin().is_terminal() {
        println!(
            "{}",
            ctx.ui.success(&format!("Session `{session_id}` is ready."))
        );
        println!(
            "{}",
            ctx.ui.muted(&format!(
                "Run `aegis chat --session-id {session_id} \"your message\"` to continue it from a non-interactive terminal."
            ))
        );
        return Ok(());
    }

    println!(
        "{}",
        ctx.ui.success(&format!(
            "Entering session: {} ({session_id})",
            detail.title
        ))
    );
    print_session_mode_hint(ctx);
    run_session_prompt_loop(ctx, session_id, invocation_mode)
}

fn print_created_session(ctx: &AppContext, session: CreatedSession) {
    println!("{}", ctx.ui.success("New session started"));
    println!("Session ID: {}", session.id);
    print_session_mode_hint(ctx);
    if ctx.ui.verbose {
        println!("Created at: {}", session.created_at);
    }
}

fn print_session_mode_hint(ctx: &AppContext) {
    println!(
        "{}",
        ctx.ui
            .muted("Type `quit` or `exit` to leave this session and return to `aegis-shell>`.")
    );
    println!(
        "{}",
        ctx.ui.muted(
            "Type `/` to open the live tools palette; Backspace closes it, arrows or mouse select."
        )
    );
    println!();
}

fn session_tool_choices() -> Vec<MenuChoice> {
    vec![
        MenuChoice::new(
            "Import document",
            "import",
            "Upload a PDF or TXT into this session's RAG context.",
        ),
        MenuChoice::new(
            "Calendar",
            "calendar",
            "Create a local Outlook calendar event from a natural-language prompt.",
        ),
        MenuChoice::new(
            "Export chat",
            "export",
            "Save this session transcript as a local Markdown file.",
        ),
    ]
}

fn dispatch_session_tool(ctx: &AppContext, session_id: &str, tool: &str) -> AppResult<()> {
    match tool {
        "import" => handle_session_tool_import(ctx, session_id),
        "calendar" => handle_session_tool_calendar(ctx),
        "export" => handle_session_tool_export(ctx, session_id),
        _ => Ok(()),
    }
}

fn prompt_for_session_tool_input(
    ctx: &AppContext,
    prompt: &str,
    empty_message: &str,
) -> AppResult<Option<String>> {
    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not flush session tool prompt: {error}"))?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|error| {
        if signals::was_ctrl_c(&error) {
            signals::ctrl_c_exit_error()
        } else {
            format!("Could not read session tool input: {error}")
        }
    })?;

    let value = input.trim();
    if value.is_empty() {
        println!("{}", ctx.ui.muted(empty_message));
        Ok(None)
    } else {
        Ok(Some(value.trim_matches('"').to_string()))
    }
}

fn handle_session_tool_import(ctx: &AppContext, session_id: &str) -> AppResult<()> {
    println!("{}", ctx.ui.header("Import Document"));
    println!(
        "{}",
        ctx.ui
            .muted("Supported files: PDF and TXT. The document will attach only to this session.")
    );

    let Some(path) = prompt_for_session_tool_input(
        ctx,
        "File path: ",
        "Import cancelled; no file path entered.",
    )?
    else {
        return Ok(());
    };

    let path = PathBuf::from(path);
    let outcome = run_with_loading_message(ctx, "Indexing document", || {
        ctx.engine.ingest_document(session_id, &path)
    })?;

    println!(
        "{}",
        ctx.ui.success(&format!(
            "Imported {} document(s), indexed {} chunk(s).",
            outcome.documents.len(),
            outcome.total_chunks
        ))
    );
    println!("{}", ctx.ui.muted(&format!("Status: {}", outcome.status)));
    for document in outcome.documents {
        println!(
            "- {} ({} chunk(s))",
            document.file_name, document.chunks_added
        );
        if ctx.ui.verbose {
            println!("  {}", ctx.ui.muted(&document.stored_path));
        }
    }

    Ok(())
}

fn handle_session_tool_calendar(ctx: &AppContext) -> AppResult<()> {
    println!("{}", ctx.ui.header("Calendar"));
    println!(
        "{}",
        ctx.ui
            .muted("Set up a time block from 1pm to 2pm tomorrow for a meeting.")
    );

    let Some(prompt) = prompt_for_session_tool_input(
        ctx,
        "Event prompt: ",
        "Calendar cancelled; no event prompt entered.",
    )?
    else {
        return Ok(());
    };

    let outcome = run_with_loading_message(ctx, "Creating calendar event", || {
        ctx.engine.create_calendar_event_from_prompt(&prompt)
    })?;

    println!("{}", ctx.ui.success(&outcome.message));
    println!("Event   : {}", outcome.event);
    println!("Delivery: {}", outcome.delivery_method);
    println!(
        "Saved   : {}",
        if outcome.saved_to_calendar {
            "yes"
        } else {
            "no"
        }
    );
    println!(
        "Opened  : {}",
        if outcome.file_opened { "yes" } else { "no" }
    );

    if let Some(parsed) = outcome.parsed {
        println!("Title   : {}", parsed.title);
        println!("Start   : {}", parsed.start);
        println!("End     : {}", parsed.end);
        if let Some(location) = parsed.location {
            println!("Location: {location}");
        }
        if let Some(description) = parsed.description {
            println!("Notes   : {description}");
        }
    }

    Ok(())
}

fn handle_session_tool_export(ctx: &AppContext, session_id: &str) -> AppResult<()> {
    println!("{}", ctx.ui.header("Export Chat"));
    let detail = ctx.engine.show_session(session_id)?;
    let default_path = format!("{}.md", safe_export_file_name(&detail.title, session_id));

    println!(
        "{}",
        ctx.ui.muted(&format!(
            "Press Enter to save as `{default_path}`, or enter a custom `.md` path."
        ))
    );
    print!("Export path: ");
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not flush export prompt: {error}"))?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|error| {
        if signals::was_ctrl_c(&error) {
            signals::ctrl_c_exit_error()
        } else {
            format!("Could not read export path: {error}")
        }
    })?;

    let export_path = input.trim().trim_matches('"');
    let export_path = if export_path.is_empty() {
        PathBuf::from(default_path)
    } else {
        PathBuf::from(export_path)
    };

    let mut transcript = String::new();
    transcript.push_str("# AEGIS Chat Export\n\n");
    transcript.push_str(&format!("Session: {}\n\n", detail.title));
    transcript.push_str(&format!("Session ID: `{}`\n\n", detail.id));
    transcript.push_str(&format!("{}\n\n", detail.note));

    if detail.recent_turns.is_empty() {
        transcript.push_str("_No saved turns in this session yet._\n");
    } else {
        for turn in detail.recent_turns {
            if let Some(message) = turn.strip_prefix("user> ") {
                transcript.push_str("## User\n\n");
                transcript.push_str(message);
                transcript.push_str("\n\n");
            } else if let Some(message) = turn.strip_prefix("assistant> ") {
                transcript.push_str("## AEGIS\n\n");
                transcript.push_str(message);
                transcript.push_str("\n\n");
            } else {
                transcript.push_str(&turn);
                transcript.push_str("\n\n");
            }
        }
    }

    fs::write(&export_path, transcript)
        .map_err(|error| format!("Could not write `{}`: {error}", export_path.display()))?;
    println!(
        "{}",
        ctx.ui
            .success(&format!("Chat exported to `{}`.", export_path.display()))
    );

    Ok(())
}

fn safe_export_file_name(title: &str, session_id: &str) -> String {
    let mut safe = title
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();

    while safe.contains("--") {
        safe = safe.replace("--", "-");
    }

    safe = safe.trim_matches('-').chars().take(48).collect();
    if safe.is_empty() || safe == "new-chat" {
        format!("aegis-session-{session_id}")
    } else {
        safe
    }
}

fn print_session_tool_error(ctx: &AppContext, tool: &str, error: &str) {
    println!();
    println!(
        "{}",
        ctx.ui
            .error(&format!("The `{tool}` tool could not complete."))
    );
    println!("{}", ctx.ui.muted(&format!("Details: {error}")));

    let lower = error.to_lowercase();
    let guidance = if lower.contains("/ingest")
        || lower.contains("rag")
        || lower.contains("upload")
        || lower.contains("document")
    {
        Some(
            "Possible causes: the Rust engine is not running, the Python RAG service is not running on 127.0.0.1:8000, the file path is invalid, the file is too large, or the file type is not PDF/TXT.",
        )
    } else if lower.contains("calendar") || lower.contains("outlook") {
        Some(
            "Possible causes: the Rust engine is not running, classic Outlook is not available, no local calendar is selected, or the event prompt could not be parsed.",
        )
    } else if lower.contains("export") || lower.contains("write") || lower.contains("session") {
        Some(
            "Possible causes: the session could not be loaded, the export path is not writable, or the file is already locked by another program.",
        )
    } else if lower.contains("connection")
        || lower.contains("could not reach")
        || lower.contains("127.0.0.1")
        || lower.contains("localhost")
    {
        Some(
            "Possible causes: the local engine service is not running or the configured localhost URL is different from the active engine.",
        )
    } else {
        None
    };

    if let Some(guidance) = guidance {
        println!("{}", ctx.ui.warning(guidance));
    }

    println!(
        "{}",
        ctx.ui
            .muted("The session is still active. Returning to `prompt>`.")
    );
    println!();
}

struct SessionPromptTerminalGuard;

impl SessionPromptTerminalGuard {
    fn enter() -> AppResult<Self> {
        enable_raw_mode()
            .map_err(|error| format!("Could not enable terminal raw mode: {error}"))?;
        execute!(io::stdout(), EnableMouseCapture)
            .map_err(|error| format!("Could not enable mouse capture: {error}"))?;
        Ok(Self)
    }
}

impl Drop for SessionPromptTerminalGuard {
    fn drop(&mut self) {
        let _ = execute!(
            io::stdout(),
            DisableMouseCapture,
            SetAttribute(Attribute::Reset)
        );
        let _ = disable_raw_mode();
    }
}

fn filtered_session_tool_indexes(buffer: &str, choices: &[MenuChoice]) -> Vec<usize> {
    let Some(query) = buffer.strip_prefix('/') else {
        return Vec::new();
    };
    let query = query.trim().to_lowercase();

    choices
        .iter()
        .enumerate()
        .filter_map(|(index, choice)| {
            if query.is_empty()
                || choice.label.to_lowercase().contains(&query)
                || choice.value.to_lowercase().contains(&query)
                || choice.description.to_lowercase().contains(&query)
            {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

fn clear_session_prompt_render(previous_popup_lines: usize) -> AppResult<()> {
    let mut stdout = io::stdout();

    queue!(stdout, MoveToColumn(0), Clear(ClearType::CurrentLine))
        .map_err(|error| format!("Could not clear session prompt: {error}"))?;

    for _ in 0..previous_popup_lines {
        queue!(stdout, MoveDown(1), Clear(ClearType::CurrentLine))
            .map_err(|error| format!("Could not clear session tools menu: {error}"))?;
    }

    if previous_popup_lines > 0 {
        queue!(stdout, MoveUp(previous_popup_lines as u16))
            .map_err(|error| format!("Could not restore session prompt cursor: {error}"))?;
    }

    stdout
        .flush()
        .map_err(|error| format!("Could not flush session prompt: {error}"))
}

fn render_session_prompt_palette(
    ctx: &AppContext,
    prompt_label: &str,
    buffer: &str,
    choices: &[MenuChoice],
    filtered_indexes: &[usize],
    selected_index: usize,
    previous_popup_lines: usize,
) -> AppResult<usize> {
    let mut stdout = io::stdout();
    let palette_visible = buffer.starts_with('/');
    let popup_lines = if palette_visible {
        filtered_indexes.len().max(1)
    } else {
        0
    };

    clear_session_prompt_render(previous_popup_lines)?;
    queue!(stdout, Print(prompt_label), Print(buffer))
        .map_err(|error| format!("Could not render session prompt: {error}"))?;

    if palette_visible {
        if filtered_indexes.is_empty() {
            queue!(
                stdout,
                Print("\r\n  No matching session tools. Backspace to close.")
            )
            .map_err(|error| format!("Could not render session tools menu: {error}"))?;
        } else {
            for (row_index, choice_index) in filtered_indexes.iter().enumerate() {
                let choice = &choices[*choice_index];
                let selected = row_index == selected_index;
                queue!(stdout, Print("\r\n"))
                    .map_err(|error| format!("Could not render session tools menu: {error}"))?;

                if selected {
                    queue!(stdout, SetAttribute(Attribute::Reverse))
                        .map_err(|error| format!("Could not highlight session tool: {error}"))?;
                }

                queue!(
                    stdout,
                    Print(format!(
                        "  {:<18} {}",
                        choice.label,
                        ctx.ui.muted(&choice.description)
                    ))
                )
                .map_err(|error| format!("Could not render session tool: {error}"))?;

                if selected {
                    queue!(stdout, SetAttribute(Attribute::Reset)).map_err(|error| {
                        format!("Could not reset session tool highlight: {error}")
                    })?;
                }
            }
        }

        queue!(
            stdout,
            MoveUp(popup_lines as u16),
            MoveToColumn((prompt_label.chars().count() + buffer.chars().count()) as u16)
        )
        .map_err(|error| format!("Could not restore session prompt cursor: {error}"))?;
    }

    stdout
        .flush()
        .map_err(|error| format!("Could not flush session prompt: {error}"))?;

    Ok(popup_lines)
}

fn read_session_prompt_input(ctx: &AppContext) -> AppResult<SessionPromptInput> {
    let prompt_label = "prompt> ";

    if !io::stdin().is_terminal() {
        print!("{prompt_label}");
        io::stdout()
            .flush()
            .map_err(|error| format!("Could not flush session prompt: {error}"))?;

        let mut input = String::new();
        let bytes = io::stdin().read_line(&mut input).map_err(|error| {
            if signals::was_ctrl_c(&error) {
                signals::ctrl_c_exit_error()
            } else {
                format!("Could not read session input: {error}")
            }
        })?;

        if bytes == 0 {
            return Ok(SessionPromptInput::Eof);
        }

        return Ok(SessionPromptInput::Submit(input.trim().to_string()));
    }

    let _guard = SessionPromptTerminalGuard::enter()?;
    let choices = session_tool_choices();
    let mut buffer = String::new();
    let mut selected_index = 0usize;
    let mut previous_popup_lines = 0usize;

    print!("{prompt_label}");
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not flush session prompt: {error}"))?;
    let mut prompt_row = crossterm::cursor::position()
        .map(|(_, row)| row)
        .unwrap_or(0);

    loop {
        match event::read().map_err(|error| format!("Could not read terminal event: {error}"))? {
            Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
                match key.code {
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        clear_session_prompt_render(previous_popup_lines)?;
                        println!();
                        return Err(signals::ctrl_c_exit_error());
                    }
                    KeyCode::Char('d')
                        if key.modifiers.contains(KeyModifiers::CONTROL) && buffer.is_empty() =>
                    {
                        clear_session_prompt_render(previous_popup_lines)?;
                        println!();
                        return Ok(SessionPromptInput::Eof);
                    }
                    KeyCode::Char(character) => {
                        let was_palette_visible = buffer.starts_with('/');
                        buffer.push(character);

                        if was_palette_visible
                            || buffer.starts_with('/')
                            || previous_popup_lines > 0
                        {
                            let filtered_indexes = filtered_session_tool_indexes(&buffer, &choices);
                            if selected_index >= filtered_indexes.len() {
                                selected_index = filtered_indexes.len().saturating_sub(1);
                            }
                            previous_popup_lines = render_session_prompt_palette(
                                ctx,
                                prompt_label,
                                &buffer,
                                &choices,
                                &filtered_indexes,
                                selected_index,
                                previous_popup_lines,
                            )?;
                            prompt_row = crossterm::cursor::position()
                                .map(|(_, row)| row)
                                .unwrap_or(prompt_row);
                        } else {
                            print!("{character}");
                            io::stdout().flush().map_err(|error| {
                                format!("Could not flush session prompt: {error}")
                            })?;
                        }
                    }
                    KeyCode::Backspace => {
                        if buffer.is_empty() {
                            print!("\x07");
                            io::stdout().flush().ok();
                            continue;
                        }

                        let was_palette_visible = buffer.starts_with('/');
                        buffer.pop();

                        if was_palette_visible
                            || buffer.starts_with('/')
                            || previous_popup_lines > 0
                        {
                            let filtered_indexes = filtered_session_tool_indexes(&buffer, &choices);
                            if selected_index >= filtered_indexes.len() {
                                selected_index = filtered_indexes.len().saturating_sub(1);
                            }
                            previous_popup_lines = render_session_prompt_palette(
                                ctx,
                                prompt_label,
                                &buffer,
                                &choices,
                                &filtered_indexes,
                                selected_index,
                                previous_popup_lines,
                            )?;
                            prompt_row = crossterm::cursor::position()
                                .map(|(_, row)| row)
                                .unwrap_or(prompt_row);
                        } else {
                            print!("\x08 \x08");
                            io::stdout().flush().map_err(|error| {
                                format!("Could not flush session prompt: {error}")
                            })?;
                        }
                    }
                    KeyCode::Esc => {
                        let had_visible_palette =
                            buffer.starts_with('/') || previous_popup_lines > 0;
                        buffer.clear();
                        selected_index = 0;
                        if had_visible_palette {
                            previous_popup_lines = render_session_prompt_palette(
                                ctx,
                                prompt_label,
                                &buffer,
                                &choices,
                                &[],
                                selected_index,
                                previous_popup_lines,
                            )?;
                            prompt_row = crossterm::cursor::position()
                                .map(|(_, row)| row)
                                .unwrap_or(prompt_row);
                        }
                    }
                    KeyCode::Up if buffer.starts_with('/') => {
                        let filtered_indexes = filtered_session_tool_indexes(&buffer, &choices);
                        if filtered_indexes.is_empty() {
                            continue;
                        }
                        selected_index = selected_index.saturating_sub(1);
                        previous_popup_lines = render_session_prompt_palette(
                            ctx,
                            prompt_label,
                            &buffer,
                            &choices,
                            &filtered_indexes,
                            selected_index,
                            previous_popup_lines,
                        )?;
                        prompt_row = crossterm::cursor::position()
                            .map(|(_, row)| row)
                            .unwrap_or(prompt_row);
                    }
                    KeyCode::Down if buffer.starts_with('/') => {
                        let filtered_indexes = filtered_session_tool_indexes(&buffer, &choices);
                        if filtered_indexes.is_empty() {
                            continue;
                        }
                        selected_index = (selected_index + 1).min(filtered_indexes.len() - 1);
                        previous_popup_lines = render_session_prompt_palette(
                            ctx,
                            prompt_label,
                            &buffer,
                            &choices,
                            &filtered_indexes,
                            selected_index,
                            previous_popup_lines,
                        )?;
                        prompt_row = crossterm::cursor::position()
                            .map(|(_, row)| row)
                            .unwrap_or(prompt_row);
                    }
                    KeyCode::Enter if buffer.starts_with('/') => {
                        let filtered_indexes = filtered_session_tool_indexes(&buffer, &choices);
                        if selected_index >= filtered_indexes.len() {
                            selected_index = filtered_indexes.len().saturating_sub(1);
                        }
                        if filtered_indexes.is_empty() {
                            print!("\x07");
                            io::stdout().flush().ok();
                            continue;
                        }

                        let choice = &choices[filtered_indexes[selected_index]];
                        clear_session_prompt_render(previous_popup_lines)?;
                        println!("{prompt_label}/{}", choice.value);
                        return Ok(SessionPromptInput::Tool(choice.value.clone()));
                    }
                    KeyCode::Enter => {
                        let submitted = buffer.trim().to_string();
                        clear_session_prompt_render(previous_popup_lines)?;
                        println!("{prompt_label}{submitted}");
                        return Ok(SessionPromptInput::Submit(submitted));
                    }
                    _ => {}
                }
            }
            Event::Mouse(mouse) if buffer.starts_with('/') => {
                let filtered_indexes = filtered_session_tool_indexes(&buffer, &choices);
                if filtered_indexes.is_empty() {
                    continue;
                }
                match mouse.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        let clicked_row = mouse.row;
                        if clicked_row > prompt_row {
                            let row_index = usize::from(clicked_row - prompt_row - 1);
                            if row_index < filtered_indexes.len() {
                                let choice = &choices[filtered_indexes[row_index]];
                                clear_session_prompt_render(previous_popup_lines)?;
                                println!("{prompt_label}/{}", choice.value);
                                return Ok(SessionPromptInput::Tool(choice.value.clone()));
                            }
                        }
                    }
                    MouseEventKind::ScrollUp => {
                        selected_index = selected_index.saturating_sub(1);
                        previous_popup_lines = render_session_prompt_palette(
                            ctx,
                            prompt_label,
                            &buffer,
                            &choices,
                            &filtered_indexes,
                            selected_index,
                            previous_popup_lines,
                        )?;
                        prompt_row = crossterm::cursor::position()
                            .map(|(_, row)| row)
                            .unwrap_or(prompt_row);
                    }
                    MouseEventKind::ScrollDown => {
                        selected_index = (selected_index + 1).min(filtered_indexes.len() - 1);
                        previous_popup_lines = render_session_prompt_palette(
                            ctx,
                            prompt_label,
                            &buffer,
                            &choices,
                            &filtered_indexes,
                            selected_index,
                            previous_popup_lines,
                        )?;
                        prompt_row = crossterm::cursor::position()
                            .map(|(_, row)| row)
                            .unwrap_or(prompt_row);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

fn run_session_prompt_loop(
    ctx: &AppContext,
    session_id: &str,
    invocation_mode: InvocationMode,
) -> AppResult<()> {
    loop {
        let prompt_input = read_session_prompt_input(ctx)?;
        let prompt = match prompt_input {
            SessionPromptInput::Eof => {
                println!();
                break;
            }
            SessionPromptInput::Tool(tool) => {
                match dispatch_session_tool(ctx, session_id, &tool) {
                    Ok(()) => {}
                    Err(error) if signals::is_ctrl_c_error(&error) => return Err(error),
                    Err(error) => print_session_tool_error(ctx, &tool, &error),
                }
                continue;
            }
            SessionPromptInput::Submit(prompt) => prompt,
        };

        if prompt.is_empty() {
            continue;
        }

        if matches!(prompt.as_str(), "quit" | "exit") {
            break;
        }

        let _reply = stream_llm_response(ctx, |on_token| {
            ctx.engine.chat(&prompt, Some(session_id), on_token)
        })?;
    }

    leave_session_prompt(ctx, invocation_mode)
}

fn leave_session_prompt(ctx: &AppContext, invocation_mode: InvocationMode) -> AppResult<()> {
    println!("{}", ctx.ui.success("Session saved. Returning to home."));
    println!();
    show_home(ctx)?;

    if matches!(invocation_mode, InvocationMode::Direct) && io::stdin().is_terminal() {
        run_interactive_shell(ctx)?;
    }

    Ok(())
}

fn print_action_status(_ctx: &AppContext, status: ActionStatus) {
    println!("Target   : {}", status.target);
    println!("Endpoint : {}", status.request_path);
    println!("Persisted: {}", if status.persisted { "yes" } else { "no" });
    println!("{}", status.message);
}

fn run_with_loading_message<T, F>(ctx: &AppContext, message: &str, operation: F) -> AppResult<T>
where
    F: FnOnce() -> AppResult<T>,
{
    let loading = ctx.ui.start_loading_animation(message);
    let result = operation();
    loading.finish();
    result
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

    #[test]
    fn tokenizes_load_command() {
        let parsed = parse_shell_cli("load 1189578c-9c96-4b4c-8015-4d0673544a6a").unwrap();
        let Some(cli) = parsed else {
            panic!("shell parser should produce a CLI command");
        };

        match cli.command {
            Some(CommandKind::Load(args)) => {
                assert_eq!(args.id, "1189578c-9c96-4b4c-8015-4d0673544a6a")
            }
            _ => panic!("expected load command"),
        }
    }
}
