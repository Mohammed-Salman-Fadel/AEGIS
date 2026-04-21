//! Role: installed-runtime layout, metadata, and process bookkeeping for the Windows bootstrap flow.
//! Called by: `main.rs`, `commands.rs`, `doctor.rs`, and `install.rs`.
//! Calls into: the local filesystem and environment only.
//! Owns: `%LOCALAPPDATA%\\AEGIS` path conventions, install-state files, engine env files, and PID bookkeeping.
//! Does not own: command routing, HTTP requests, or installer download/extraction behavior.
//! Next TODOs: add richer version compatibility checks and migrate v2 RAG/runtime state into the same layout.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::AppResult;

pub const DEFAULT_ENGINE_HOST: &str = "127.0.0.1";
pub const DEFAULT_ENGINE_PORT: u16 = 8080;
pub const DEFAULT_OLLAMA_URL: &str = "http://127.0.0.1:11434";
pub const DEFAULT_MODEL: &str = "qwen3:4b";
pub const DEFAULT_UI_URL: &str = "http://localhost:8080";
pub const DEFAULT_MANIFEST_URL: &str =
    "https://github.com/aegis-project/AEGIS/releases/latest/download/installer-manifest.json";

#[derive(Debug, Clone)]
pub struct RuntimeLayout {
    pub root: PathBuf,
    pub bin_dir: PathBuf,
    pub runtime_dir: PathBuf,
    pub current_runtime_dir: PathBuf,
    pub staged_runtime_dir: PathBuf,
    pub backups_dir: PathBuf,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub run_dir: PathBuf,
    pub install_state_path: PathBuf,
    pub engine_env_path: PathBuf,
    pub manifest_cache_path: PathBuf,
    pub engine_pid_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallerManifest {
    pub version: String,
    pub runtime_url: String,
    pub runtime_sha256: String,
    pub ollama_installer_url: String,
    pub ollama_installer_sha256: String,
    pub default_model: String,
    pub engine_host: String,
    pub engine_port: u16,
    pub ui_url: String,
    pub minimum_supported_windows: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallState {
    pub version: String,
    pub installed_at_epoch_seconds: u64,
    pub manifest_url: String,
    pub runtime_sha256: String,
    pub default_model: String,
    pub engine_host: String,
    pub engine_port: u16,
    pub ui_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnginePidRecord {
    pub pid: u32,
    pub started_at_epoch_seconds: u64,
    pub version: String,
    pub url: String,
}

impl RuntimeLayout {
    pub fn discover() -> Self {
        let root = env::var_os("AEGIS_HOME")
            .map(PathBuf::from)
            .or_else(default_runtime_root)
            .unwrap_or_else(|| env::temp_dir().join("AEGIS"));

        Self {
            bin_dir: root.join("bin"),
            runtime_dir: root.join("runtime"),
            current_runtime_dir: root.join("runtime").join("current"),
            staged_runtime_dir: root.join("runtime").join("staged"),
            backups_dir: root.join("runtime").join("backups"),
            config_dir: root.join("config"),
            data_dir: root.join("data"),
            logs_dir: root.join("logs"),
            run_dir: root.join("run"),
            install_state_path: root.join("config").join("install-state.json"),
            engine_env_path: root.join("config").join("engine.env"),
            manifest_cache_path: root.join("config").join("installer-manifest.json"),
            engine_pid_path: root.join("run").join("engine.json"),
            root,
        }
    }

    pub fn ensure_base_dirs(&self) -> AppResult<()> {
        for dir in [
            &self.root,
            &self.bin_dir,
            &self.runtime_dir,
            &self.staged_runtime_dir,
            &self.backups_dir,
            &self.config_dir,
            &self.data_dir,
            &self.logs_dir,
            &self.run_dir,
        ] {
            fs::create_dir_all(dir)
                .map_err(|error| format!("Could not create runtime directory `{}`: {error}", dir.display()))?;
        }

        Ok(())
    }

    pub fn stage_dir_for(&self, version: &str) -> PathBuf {
        self.staged_runtime_dir.join(version)
    }

    pub fn backup_dir_for(&self, version: &str) -> PathBuf {
        self.backups_dir.join(version)
    }

    pub fn installed_cli_exe(&self) -> PathBuf {
        self.current_runtime_dir.join("bin").join("aegis.exe")
    }

    pub fn installed_engine_exe(&self) -> PathBuf {
        self.current_runtime_dir.join("bin").join("aegis-engine.exe")
    }

    pub fn user_cli_exe(&self) -> PathBuf {
        self.bin_dir.join("aegis.exe")
    }

    pub fn user_engine_exe(&self) -> PathBuf {
        self.bin_dir.join("aegis-engine.exe")
    }

    pub fn installed_ui_dir(&self) -> PathBuf {
        self.current_runtime_dir.join("ui")
    }

    pub fn runtime_default_env_path(&self) -> PathBuf {
        self.current_runtime_dir.join("config").join("default.env")
    }

    pub fn engine_stdout_log(&self) -> PathBuf {
        self.logs_dir.join("engine.stdout.log")
    }

    pub fn engine_stderr_log(&self) -> PathBuf {
        self.logs_dir.join("engine.stderr.log")
    }

    pub fn is_installed(&self) -> bool {
        self.install_state_path.exists()
            && self.user_engine_exe().exists()
            && self.installed_ui_dir().join("index.html").exists()
    }

    pub fn manifest_url(&self) -> String {
        env::var("AEGIS_INSTALLER_MANIFEST_URL").unwrap_or_else(|_| DEFAULT_MANIFEST_URL.to_string())
    }

    pub fn current_engine_url(&self) -> String {
        self.load_install_state()
            .map(|state| format!("http://{}:{}", state.engine_host, state.engine_port))
            .unwrap_or_else(|| format!("http://{DEFAULT_ENGINE_HOST}:{DEFAULT_ENGINE_PORT}"))
    }

    pub fn load_install_state(&self) -> Option<InstallState> {
        read_json_file(&self.install_state_path).ok()
    }

    pub fn save_install_state(&self, state: &InstallState) -> AppResult<()> {
        write_json_file(&self.install_state_path, state)
    }

    pub fn save_manifest_cache(&self, manifest: &InstallerManifest) -> AppResult<()> {
        write_json_file(&self.manifest_cache_path, manifest)
    }

    pub fn load_pid_record(&self) -> Option<EnginePidRecord> {
        read_json_file(&self.engine_pid_path).ok()
    }

    pub fn save_pid_record(&self, record: &EnginePidRecord) -> AppResult<()> {
        write_json_file(&self.engine_pid_path, record)
    }

    pub fn clear_pid_record(&self) -> AppResult<()> {
        if self.engine_pid_path.exists() {
            fs::remove_file(&self.engine_pid_path).map_err(|error| {
                format!(
                    "Could not remove engine PID file `{}`: {error}",
                    self.engine_pid_path.display()
                )
            })?;
        }

        Ok(())
    }

    pub fn write_engine_env(&self, manifest: &InstallerManifest) -> AppResult<()> {
        let ui_dir = self.installed_ui_dir();
        let contents = format!(
            "AEGIS_INFERENCE_PROVIDER=ollama\nAEGIS_OLLAMA_URL={}\nAEGIS_MODEL={}\nAEGIS_ENGINE_HOST={}\nAEGIS_ENGINE_PORT={}\nAEGIS_UI_DIR={}\n",
            DEFAULT_OLLAMA_URL,
            manifest.default_model,
            manifest.engine_host,
            manifest.engine_port,
            ui_dir.display()
        );

        fs::write(&self.engine_env_path, contents).map_err(|error| {
            format!(
                "Could not write engine env file `{}`: {error}",
                self.engine_env_path.display()
            )
        })
    }

    pub fn load_engine_env_pairs(&self) -> AppResult<Vec<(String, String)>> {
        let mut pairs = Vec::new();

        if self.runtime_default_env_path().exists() {
            pairs.extend(parse_env_file(&self.runtime_default_env_path())?);
        }

        if self.engine_env_path.exists() {
            pairs.extend(parse_env_file(&self.engine_env_path)?);
        }

        pairs.push(("RAG_DATA_DIR".to_string(), self.data_dir.display().to_string()));
        pairs.push((
            "AEGIS_UI_DIR".to_string(),
            self.installed_ui_dir().display().to_string(),
        ));

        Ok(pairs)
    }
}

pub fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn default_runtime_root() -> Option<PathBuf> {
    env::var_os("LOCALAPPDATA").map(|value| PathBuf::from(value).join("AEGIS"))
}

fn read_json_file<T>(path: &Path) -> AppResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Could not read `{}`: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("Could not parse `{}`: {error}", path.display()))
}

fn write_json_file<T>(path: &Path, value: &T) -> AppResult<()>
where
    T: Serialize,
{
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create `{}`: {error}", parent.display()))?;
    }

    let raw = serde_json::to_string_pretty(value)
        .map_err(|error| format!("Could not serialize `{}`: {error}", path.display()))?;

    fs::write(path, raw)
        .map_err(|error| format!("Could not write `{}`: {error}", path.display()))
}

fn parse_env_file(path: &Path) -> AppResult<Vec<(String, String)>> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Could not read env file `{}`: {error}", path.display()))?;

    let mut pairs = Vec::new();
    for line in raw.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };

        pairs.push((key.trim().to_string(), value.trim().to_string()));
    }

    Ok(pairs)
}
