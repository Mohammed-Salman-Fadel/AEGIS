use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CommandLinePolicy {
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

impl Default for CommandLinePolicy {
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

impl CommandLinePolicy {
    pub fn load() -> Self {
        let mut policy: CommandLinePolicy = fs::read_to_string(settings_path())
            .ok()
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_default();
        // Workspace boundaries and dirty-tree protection are invariants, even
        // if a settings file is manually edited or produced by an old client.
        policy.git_safety = true;
        policy
    }
}

pub fn settings_path() -> PathBuf {
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

pub fn state_root() -> AppResult<PathBuf> {
    let root = settings_path()
        .parent()
        .map(|path| path.join("cli-state"))
        .ok_or_else(|| "Could not resolve the CLI state directory.".to_string())?;
    fs::create_dir_all(&root).map_err(|error| {
        format!(
            "Could not create CLI state at `{}`: {error}",
            root.display()
        )
    })?;
    Ok(root)
}
