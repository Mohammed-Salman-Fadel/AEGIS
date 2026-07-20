use std::env;
use std::fs;
use std::path::PathBuf;

use axum::{Json, http::StatusCode};
use serde::{Deserialize, Serialize};

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
