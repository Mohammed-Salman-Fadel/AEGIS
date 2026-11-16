use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use axum::{Json, http::StatusCode};
use serde::{Deserialize, Serialize};
use tokio::process::Command;
use tokio::time::timeout;

const CLI_COMMAND_TIMEOUT: Duration = Duration::from_secs(120);
const MAX_COMMAND_CHARS: usize = 8_192;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandLineSettings {
    pub agentic_loop: bool,
    pub repository_detection: bool,
    pub repository_instructions: bool,
    pub semantic_index: bool,
    pub persistent_task_plan: bool,
    pub task_checkpoints: bool,
    pub context_budgeting: bool,
    pub patch_application: bool,
    pub command_execution: bool,
    pub automatic_verification: bool,
    pub deep_reasoning: bool,
    pub git_safety: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteCommandRequest {
    command: String,
}

#[derive(Debug, Serialize)]
pub struct ExecuteCommandResponse {
    command: String,
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

impl Default for CommandLineSettings {
    fn default() -> Self {
        Self {
            agentic_loop: true,
            repository_detection: true,
            repository_instructions: true,
            semantic_index: true,
            persistent_task_plan: true,
            task_checkpoints: true,
            context_budgeting: true,
            patch_application: true,
            command_execution: true,
            automatic_verification: true,
            deep_reasoning: false,
            git_safety: true,
        }
    }
}

pub async fn get_settings() -> Json<CommandLineSettings> {
    Json(load_settings())
}

pub async fn save_settings(
    Json(mut settings): Json<CommandLineSettings>,
) -> Result<Json<CommandLineSettings>, (StatusCode, String)> {
    // Git boundary and dirty-tree protection are safety invariants, not a
    // discretionary capability. Keep the policy true even if an old client
    // submits false.
    settings.git_safety = true;
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(settings_error)?;
    }
    let content = serde_json::to_vec_pretty(&settings).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Could not serialize Command Line settings: {error}"),
        )
    })?;
    fs::write(&path, content).map_err(settings_error)?;
    Ok(Json(settings))
}

pub async fn execute_command(
    Json(payload): Json<ExecuteCommandRequest>,
) -> Result<Json<ExecuteCommandResponse>, (StatusCode, String)> {
    let command = payload.command.trim();
    if command.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Enter an AEGIS CLI command.".to_string(),
        ));
    }
    if command.chars().count() > MAX_COMMAND_CHARS {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            "The CLI command is too long.".to_string(),
        ));
    }

    let mut args =
        tokenize_command(command).map_err(|message| (StatusCode::BAD_REQUEST, message))?;
    if matches!(
        args.first().map(String::as_str),
        Some("aegis" | "aegis.exe")
    ) {
        args.remove(0);
    }
    if args.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "Enter a CLI command after `aegis`, or type `help`.".to_string(),
        ));
    }
    reject_embedded_unsupported_command(&args)?;

    let cli_path = resolve_cli_path().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Could not locate the AEGIS CLI binary. Set AEGIS_CLI_PATH to aegis.exe.".to_string(),
        )
    })?;

    let mut process = Command::new(&cli_path);
    process
        .arg("--no-color")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output = timeout(CLI_COMMAND_TIMEOUT, process.output())
        .await
        .map_err(|_| {
            (
                StatusCode::REQUEST_TIMEOUT,
                "The CLI command exceeded the 120-second terminal limit.".to_string(),
            )
        })?
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Could not start `{}`: {error}", cli_path.display()),
            )
        })?;

    Ok(Json(ExecuteCommandResponse {
        command: args.join(" "),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        exit_code: output.status.code(),
    }))
}

fn reject_embedded_unsupported_command(args: &[String]) -> Result<(), (StatusCode, String)> {
    let command = args.first().map(|value| value.to_ascii_lowercase());
    if matches!(
        command.as_deref(),
        Some("open" | "restart" | "install" | "upgrade" | "repl")
    ) {
        return Err((
            StatusCode::BAD_REQUEST,
            "That lifecycle or interactive command must be run from a system terminal.".to_string(),
        ));
    }
    if command.as_deref() == Some("ask") && args.iter().any(|arg| arg == "--stdin") {
        return Err((
            StatusCode::BAD_REQUEST,
            "`ask --stdin` requires a system terminal. Enter the prompt as a `chat` command instead.".to_string(),
        ));
    }
    Ok(())
}

fn resolve_cli_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("AEGIS_CLI_PATH").map(PathBuf::from) {
        if path.is_file() {
            return Some(path);
        }
    }

    let executable_name = if cfg!(windows) { "aegis.exe" } else { "aegis" };
    let current_exe = env::current_exe().ok();
    let mut candidates = Vec::new();
    if let Some(parent) = current_exe.as_deref().and_then(|path| path.parent()) {
        candidates.push(parent.join(executable_name));
        for root in parent.ancestors().take(5) {
            candidates.push(
                root.join("cli")
                    .join("target")
                    .join("release")
                    .join(executable_name),
            );
            candidates.push(root.join("build").join("release").join(executable_name));
        }
    }
    if let Ok(cwd) = env::current_dir() {
        for root in cwd.ancestors().take(4) {
            candidates.push(
                root.join("cli")
                    .join("target")
                    .join("release")
                    .join(executable_name),
            );
            candidates.push(root.join("build").join("release").join(executable_name));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn tokenize_command(input: &str) -> Result<Vec<String>, String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut characters = input.chars().peekable();

    while let Some(character) = characters.next() {
        if character == '\\' {
            if matches!(characters.peek(), Some('\\' | '\'' | '"' | ' ' | '\t')) {
                current.push(characters.next().expect("peeked escaped CLI character"));
            } else {
                current.push(character);
            }
            continue;
        }
        if let Some(active_quote) = quote {
            if character == active_quote {
                quote = None;
            } else {
                current.push(character);
            }
        } else if matches!(character, '\'' | '"') {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if quote.is_some() {
        return Err("The CLI command contains an unterminated quote.".to_string());
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenizes_cli_quotes_and_optional_prefix() {
        assert_eq!(
            tokenize_command("aegis chat \"hello world\"").unwrap(),
            vec!["aegis", "chat", "hello world"]
        );
    }

    #[test]
    fn preserves_windows_path_separators() {
        assert_eq!(
            tokenize_command(r#"code inspect --path D:\AEGIS\AEGIS"#).unwrap(),
            vec!["code", "inspect", "--path", r#"D:\AEGIS\AEGIS"#]
        );
    }

    #[test]
    fn rejects_unterminated_quotes() {
        assert!(tokenize_command("chat \"hello").is_err());
    }

    #[test]
    fn blocks_commands_that_require_the_system_terminal() {
        assert!(reject_embedded_unsupported_command(&["restart".into()]).is_err());
        assert!(reject_embedded_unsupported_command(&["status".into()]).is_ok());
    }
}

fn load_settings() -> CommandLineSettings {
    let mut settings: CommandLineSettings = fs::read_to_string(settings_path())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default();
    settings.git_safety = true;
    settings
}

fn settings_path() -> PathBuf {
    if let Some(path) = env::var_os("AEGIS_CLI_SETTINGS_PATH") {
        return PathBuf::from(path);
    }
    if cfg!(windows) {
        return env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(env::temp_dir)
            .join("AEGIS-User")
            .join("command-line.json");
    }
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .unwrap_or_else(env::temp_dir)
        .join("aegis")
        .join("command-line.json")
}

fn settings_error(error: std::io::Error) -> (StatusCode, String) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        format!("Could not persist Command Line settings: {error}"),
    )
}
