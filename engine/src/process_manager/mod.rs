//! Process manager helpers for local runtime dependencies.
//!
//! The current implementation focuses on LM Studio because AEGIS can treat it
//! as a backend-owned local service rather than making each UI client manage it.

use std::collections::HashSet;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use reqwest::StatusCode;
use serde_json::Value;
use tokio::process::Command;
use tokio::time::{Instant, sleep};

const LM_STUDIO_PROBE_TIMEOUT: Duration = Duration::from_millis(1_200);
const LM_STUDIO_READY_TIMEOUT: Duration = Duration::from_secs(20);

pub async fn ensure_lm_studio_server(base_url: &str) -> anyhow::Result<()> {
    if lm_studio_server_reachable(base_url).await {
        return Ok(());
    }

    if !lm_studio_autostart_enabled() {
        anyhow::bail!(
            "LM Studio is not reachable at `{base_url}`, and AEGIS LM Studio autostart is disabled."
        );
    }

    let cli = lm_studio_cli_program();
    let mut attempt_errors = Vec::new();

    if let Err(error) = start_lm_studio_server(&cli, base_url).await {
        attempt_errors.push(error.to_string());
    }
    if wait_for_lm_studio_server(base_url, LM_STUDIO_READY_TIMEOUT).await {
        return Ok(());
    }

    if let Err(error) = start_lm_studio_daemon(&cli).await {
        attempt_errors.push(error.to_string());
    }
    if let Err(error) = start_lm_studio_server(&cli, base_url).await {
        attempt_errors.push(error.to_string());
    }
    if wait_for_lm_studio_server(base_url, LM_STUDIO_READY_TIMEOUT).await {
        return Ok(());
    }

    let details = if attempt_errors.is_empty() {
        "AEGIS could not make the LM Studio API reachable.".to_string()
    } else {
        format!(
            "AEGIS tried to start LM Studio automatically, but it still was not reachable. Details: {}",
            attempt_errors.join(" | ")
        )
    };

    anyhow::bail!(
        "{details} Install LM Studio, run it once so `lms` is initialized, and make sure the server can listen on `{base_url}`."
    );
}

pub async fn unload_lm_studio_model(model: &str) -> anyhow::Result<()> {
    let model = model.trim();
    if model.is_empty() {
        anyhow::bail!("LM Studio model unload requested an empty model name.");
    }

    let cli = lm_studio_cli_program();
    let args = vec!["unload".to_string(), model.to_string()];

    run_lms_command(&cli, &args).await?;
    Ok(())
}

pub async fn load_lm_studio_model(
    model: &str,
    context_length: Option<usize>,
) -> anyhow::Result<()> {
    let model = model.trim();
    if model.is_empty() {
        anyhow::bail!("LM Studio model load requested an empty model name.");
    }

    let cli = lm_studio_cli_program();
    let mut args = vec!["load".to_string(), model.to_string()];
    if let Some(context_length) = context_length.filter(|value| *value > 0) {
        args.push("--context-length".to_string());
        args.push(context_length.to_string());
    }

    run_lms_command(&cli, &args).await?;
    Ok(())
}

pub async fn list_lm_studio_downloaded_models() -> anyhow::Result<Vec<String>> {
    let cli = lm_studio_cli_program();
    let output = run_lms_command(
        &cli,
        &["ls".to_string(), "--llm".to_string(), "--json".to_string()],
    )
    .await?;

    let json_models = parse_lm_studio_models_json(&output.stdout);
    if !json_models.is_empty() {
        return Ok(json_models);
    }

    Ok(parse_lm_studio_models_table(&output.stdout))
}

async fn start_lm_studio_server(cli: &PathBuf, base_url: &str) -> anyhow::Result<()> {
    let mut args = vec!["server".to_string(), "start".to_string()];
    if let Some(port) = lm_studio_port(base_url) {
        args.push("--port".to_string());
        args.push(port.to_string());
    }

    run_lms_command(cli, &args).await?;
    Ok(())
}

async fn start_lm_studio_daemon(cli: &PathBuf) -> anyhow::Result<()> {
    run_lms_command(cli, &["daemon".to_string(), "up".to_string()]).await?;
    Ok(())
}

async fn run_lms_command(cli: &PathBuf, args: &[String]) -> anyhow::Result<CommandOutput> {
    let output = Command::new(cli)
        .args(args)
        .output()
        .await
        .with_context(|| format!("Could not execute `{}`.", render_command(cli, args)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() {
        return Ok(CommandOutput { stdout });
    }

    let mut detail = format!(
        "`{}` exited with status {}.",
        render_command(cli, args),
        output
            .status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_string())
    );
    if !stdout.is_empty() {
        detail.push_str(&format!(" stdout: {stdout}"));
    }
    if !stderr.is_empty() {
        detail.push_str(&format!(" stderr: {stderr}"));
    }

    anyhow::bail!(detail);
}

async fn lm_studio_server_reachable(base_url: &str) -> bool {
    let request_path = format!("{}/v1/models", base_url.trim_end_matches('/'));
    reqwest::Client::new()
        .get(request_path)
        .timeout(LM_STUDIO_PROBE_TIMEOUT)
        .send()
        .await
        .map(|response| {
            response.status().is_success() || response.status() == StatusCode::UNAUTHORIZED
        })
        .unwrap_or(false)
}

async fn wait_for_lm_studio_server(base_url: &str, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if lm_studio_server_reachable(base_url).await {
            return true;
        }
        sleep(Duration::from_millis(400)).await;
    }

    false
}

fn lm_studio_cli_program() -> PathBuf {
    if let Some(path) = env::var_os("AEGIS_LM_STUDIO_CLI").filter(|value| !value.is_empty()) {
        return PathBuf::from(path);
    }

    if let Some(home) = env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
        let candidate = PathBuf::from(home)
            .join(".lmstudio")
            .join("bin")
            .join("lms.exe");
        if candidate.exists() {
            return candidate;
        }
    }

    if let Some(home) = env::var_os("HOME").filter(|value| !value.is_empty()) {
        let candidate = PathBuf::from(home)
            .join(".lmstudio")
            .join("bin")
            .join("lms");
        if candidate.exists() {
            return candidate;
        }
    }

    if cfg!(windows) {
        PathBuf::from("lms.exe")
    } else {
        PathBuf::from("lms")
    }
}

fn lm_studio_port(base_url: &str) -> Option<u16> {
    reqwest::Url::parse(base_url)
        .ok()
        .and_then(|url| url.port_or_known_default())
}

fn lm_studio_autostart_enabled() -> bool {
    !matches!(
        env::var("AEGIS_LM_STUDIO_AUTOSTART")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase()),
        Some(value) if matches!(value.as_str(), "0" | "false" | "no" | "off")
    )
}

fn render_command(cli: &PathBuf, args: &[String]) -> String {
    let rendered_args = if args.is_empty() {
        String::new()
    } else {
        format!(" {}", args.join(" "))
    };
    format!("{}{}", cli.display(), rendered_args)
}

fn parse_lm_studio_models_json(raw: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return Vec::new();
    };

    let mut models = Vec::new();
    let mut seen = HashSet::new();
    collect_model_keys(&value, &mut models, &mut seen);
    models
}

fn collect_model_keys(value: &Value, models: &mut Vec<String>, seen: &mut HashSet<String>) {
    match value {
        Value::Array(items) => {
            for item in items {
                collect_model_keys(item, models, seen);
            }
        }
        Value::Object(map) => {
            if let Some(candidate) = extract_model_key(map)
                && seen.insert(candidate.clone())
            {
                models.push(candidate);
            }

            for item in map.values() {
                collect_model_keys(item, models, seen);
            }
        }
        _ => {}
    }
}

fn extract_model_key(map: &serde_json::Map<String, Value>) -> Option<String> {
    for field in [
        "modelKey",
        "model_key",
        "key",
        "path",
        "identifier",
        "id",
        "name",
    ] {
        if let Some(candidate) = map.get(field).and_then(Value::as_str) {
            let candidate = candidate.trim();
            if looks_like_model_key(candidate) {
                return Some(candidate.to_string());
            }
        }
    }

    None
}

fn parse_lm_studio_models_table(raw: &str) -> Vec<String> {
    let mut models = Vec::new();
    let mut seen = HashSet::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty()
            || trimmed.starts_with("You have ")
            || trimmed.starts_with("LLMs ")
            || trimmed.starts_with("Embedding Models")
            || trimmed.contains("PARAMS")
            || trimmed.contains("ARCHITECTURE")
            || trimmed.starts_with("...")
        {
            continue;
        }

        let Some(first_token) = trimmed.split_whitespace().next() else {
            continue;
        };

        if looks_like_model_key(first_token) && seen.insert(first_token.to_string()) {
            models.push(first_token.to_string());
        }
    }

    models
}

fn looks_like_model_key(candidate: &str) -> bool {
    let candidate = candidate.trim();
    if candidate.is_empty()
        || candidate.contains('\n')
        || candidate.contains('\r')
        || candidate.contains(' ')
        || candidate.starts_with("C:\\")
        || candidate.starts_with('/')
    {
        return false;
    }

    candidate
        .chars()
        .any(|character| character.is_ascii_alphanumeric())
}

struct CommandOutput {
    stdout: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_model_keys() {
        let raw = r#"[
          {"modelKey":"lmstudio-community/meta-llama-3.1-8b-instruct"},
          {"path":"hugging-quants/llama-3.2-1b-instruct"}
        ]"#;

        assert_eq!(
            parse_lm_studio_models_json(raw),
            vec![
                "lmstudio-community/meta-llama-3.1-8b-instruct".to_string(),
                "hugging-quants/llama-3.2-1b-instruct".to_string()
            ]
        );
    }

    #[test]
    fn parses_table_model_keys() {
        let raw = "You have 2 models, taking up 9.00 GB of disk space.\n\nLLMs (Large Language Models) PARAMS ARCHITECTURE SIZE\nlmstudio-community/meta-llama-3.1-8b-instruct 8B Llama 4.92 GB\nhugging-quants/llama-3.2-1b-instruct 1B Llama 1.32 GB";

        assert_eq!(
            parse_lm_studio_models_table(raw),
            vec![
                "lmstudio-community/meta-llama-3.1-8b-instruct".to_string(),
                "hugging-quants/llama-3.2-1b-instruct".to_string()
            ]
        );
    }

    #[test]
    fn rejects_obvious_non_model_paths() {
        assert!(!looks_like_model_key("C:\\Models\\foo.gguf"));
        assert!(!looks_like_model_key("/tmp/model.gguf"));
        assert!(looks_like_model_key("openai/gpt-oss-20b"));
    }
}
