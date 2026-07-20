#![allow(dead_code)]

//! Role: subprocess scaffolding for future engine startup, installer steps, and model downloads.
//! Called by: `commands.rs` for status previews and later by `install.rs` when real execution is approved.
//! Calls into: the host operating system through `std::process::Command`.
//! Owns: generic launch-plan descriptions and process execution helpers.
//! Does not own: command routing, dependency decisions, or CLI argument parsing.
//! Next TODOs: map installer and engine flows onto these helpers once the approved OS-specific commands are finalized.

use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::AppResult;
use crate::ui::Ui;
use crate::workspace::Workspace;

const ENGINE_HEALTH_PATH: &str = "/health";
const RAG_HEALTH_PATH: &str = "/health";
const OLLAMA_HEALTH_PATH: &str = "/api/tags";

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub env: Vec<(String, String)>,
}

#[derive(Debug, Default)]
pub struct RuntimeStartReport {
    pub started: Vec<String>,
    pub already_running: Vec<String>,
    pub starting: Vec<String>,
    pub warnings: Vec<String>,
    pub web_url: String,
}

pub fn engine_base_url_from_env() -> String {
    std::env::var("AEGIS_ENGINE_URL")
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            let host =
                std::env::var("AEGIS_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
            let port = std::env::var("AEGIS_ENGINE_PORT").unwrap_or_else(|_| "8080".to_string());
            format!("http://{}:{}", host.trim(), port.trim())
        })
}

impl LaunchPlan {
    pub fn command_preview(&self) -> String {
        let rendered_args = if self.args.is_empty() {
            String::new()
        } else {
            format!(" {}", self.args.join(" "))
        };

        format!("{}{}", self.program, rendered_args)
    }
}

pub fn ensure_local_runtime(ui: &Ui, workspace: &Workspace) -> RuntimeStartReport {
    ensure_local_runtime_with_output(ui, workspace, true)
}

pub fn ensure_local_runtime_quiet(ui: &Ui, workspace: &Workspace) -> RuntimeStartReport {
    ensure_local_runtime_with_output(ui, workspace, false)
}

fn ensure_local_runtime_with_output(
    ui: &Ui,
    workspace: &Workspace,
    render: bool,
) -> RuntimeStartReport {
    if std::env::var("AEGIS_NO_AUTOSTART")
        .ok()
        .is_some_and(|value| matches!(value.trim(), "1" | "true" | "yes" | "on"))
    {
        return RuntimeStartReport {
            warnings: vec![
                "AEGIS_NO_AUTOSTART is set, so CLI runtime auto-start was skipped.".to_string(),
            ],
            web_url: workspace.web_ui_url(),
            ..RuntimeStartReport::default()
        };
    }

    let mut report = RuntimeStartReport {
        web_url: workspace.web_ui_url(),
        ..RuntimeStartReport::default()
    };

    let logs_dir = logs_dir_path(workspace);
    if let Err(error) = fs::create_dir_all(&logs_dir) {
        report.warnings.push(format!(
            "Could not create AEGIS log directory `{}`: {error}",
            logs_dir.display()
        ));
    }

    ensure_rag_runtime(workspace, &logs_dir, &mut report);
    ensure_ollama_runtime(workspace, &logs_dir, &mut report);
    ensure_engine_runtime(workspace, &logs_dir, &mut report);
    if render {
        render_runtime_report(ui, &report);
    }

    report
}

fn ensure_rag_runtime(workspace: &Workspace, logs_dir: &Path, report: &mut RuntimeStartReport) {
    let base_url =
        std::env::var("AEGIS_RAG_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());
    let health_url = join_url(&base_url, RAG_HEALTH_PATH);

    if service_reachable(&health_url) {
        report.already_running.push(format!("RAG ({base_url})"));
        return;
    }

    if !workspace.rag_dir.join("app").exists() {
        report.warnings.push(format!(
            "RAG service was not started because `{}` does not look like the python-services folder.",
            workspace.rag_dir.display()
        ));
        return;
    }

    if existing_background_process_running(logs_dir, "RAG") {
        report.starting.push(format!("RAG ({health_url})"));
        return;
    }

    let python = python_program_for_rag(&workspace.rag_dir);
    let plan = LaunchPlan {
        label: "RAG".to_string(),
        program: python,
        args: vec![
            "-m".to_string(),
            "uvicorn".to_string(),
            "app.main:app".to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            "8000".to_string(),
        ],
        cwd: workspace.rag_dir.clone(),
        env: vec![("PYTHONUNBUFFERED".to_string(), "1".to_string())],
    };

    match spawn_background(&plan, logs_dir) {
        Ok(()) => {
            report.started.push("RAG".to_string());
            if !wait_for_service(&health_url, Duration::from_secs(8)) {
                report.starting.push(format!("RAG ({health_url})"));
            }
        }
        Err(error) => report.warnings.push(error),
    }
}

fn ensure_ollama_runtime(workspace: &Workspace, logs_dir: &Path, report: &mut RuntimeStartReport) {
    if !uses_ollama_provider() {
        return;
    }

    let base_url =
        std::env::var("AEGIS_OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".to_string());
    let health_url = join_url(&base_url, OLLAMA_HEALTH_PATH);

    if service_reachable(&health_url) {
        report.already_running.push(format!("Ollama ({base_url})"));
        return;
    }

    if existing_background_process_running(logs_dir, "Ollama") {
        report.starting.push(format!("Ollama ({health_url})"));
        return;
    }

    let Some(ollama_program) = resolve_ollama_program() else {
        report.warnings.push(
            "Ollama was not found. Set `AEGIS_OLLAMA_PATH` to the moved `ollama.exe`, or switch AEGIS to another installed provider."
                .to_string(),
        );
        return;
    };

    let plan = LaunchPlan {
        label: "Ollama".to_string(),
        program: ollama_program.display().to_string(),
        args: vec!["serve".to_string()],
        cwd: workspace.root.clone(),
        env: Vec::new(),
    };

    match spawn_background(&plan, logs_dir) {
        Ok(()) => {
            report.started.push("Ollama".to_string());
            if !wait_for_service(&health_url, Duration::from_secs(10)) {
                report.starting.push(format!("Ollama ({health_url})"));
            }
        }
        Err(error) => report.warnings.push(error),
    }
}

fn ensure_engine_runtime(workspace: &Workspace, logs_dir: &Path, report: &mut RuntimeStartReport) {
    let base_url = engine_base_url_from_env();
    let health_url = join_url(&base_url, ENGINE_HEALTH_PATH);

    if service_reachable(&health_url) {
        report.already_running.push(format!("Engine ({base_url})"));
        return;
    }

    // The engine also serves the Web UI, so it must still start when inference
    // is degraded. The engine reports model/backend readiness separately while
    // keeping the UI available for recovery and provider configuration.
    let _ = ensure_ollama_model(report);

    let Some(plan) = engine_launch_plan(workspace) else {
        report.warnings.push(
            "Engine was not started because no engine binary or Cargo.toml was found.".to_string(),
        );
        return;
    };

    if existing_background_process_running(logs_dir, "Engine") {
        report.starting.push(format!("Engine ({health_url})"));
        if wait_for_service(&health_url, Duration::from_secs(45)) {
            report
                .starting
                .retain(|service| !service.starts_with("Engine ("));
            report.already_running.push(format!("Engine ({base_url})"));
        } else {
            report.warnings.push(format!(
                "Engine is still not healthy at `{health_url}`. Check `{}`.",
                log_path(logs_dir, "Engine").display()
            ));
        }
        return;
    }

    match spawn_background(&plan, logs_dir) {
        Ok(()) => {
            report.started.push("Engine".to_string());
            let wait_timeout = if plan.program.eq_ignore_ascii_case("cargo") {
                Duration::from_secs(90)
            } else {
                Duration::from_secs(45)
            };
            if !wait_for_service(&health_url, wait_timeout) {
                report.warnings.push(format!(
                    "Engine was started in the background but did not answer `{health_url}` yet. The model may still be warming, or startup may have failed. Check `{}`.",
                    log_path(logs_dir, &plan.label).display()
                ));
            }
        }
        Err(error) => report.warnings.push(error),
    }
}

/// Check if Ollama CLI is available, either on PATH or at a known install location.
fn resolve_ollama_program() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("AEGIS_OLLAMA_PATH")
        .map(PathBuf::from)
        .filter(|path| path.is_file())
    {
        return Some(path);
    }
    if let Some(path) = which_program_path("ollama") {
        return Some(path);
    }

    let mut candidates = Vec::new();
    if cfg!(windows) {
        if let Some(local) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
            candidates.extend([
                local.join("Programs/Ollama/ollama.exe"),
                local.join("Ollama/ollama.exe"),
            ]);
        }
        if let Some(profile) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
            candidates.extend([
                profile.join("AppData/Local/Programs/Ollama/ollama.exe"),
                profile.join("scoop/apps/ollama/current/ollama.exe"),
                profile.join("Applications/Ollama/ollama.exe"),
            ]);
        }
        for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(variable).map(PathBuf::from) {
                candidates.push(root.join("Ollama/ollama.exe"));
            }
        }
        if let Some(chocolatey) = std::env::var_os("ChocolateyInstall").map(PathBuf::from) {
            candidates.push(chocolatey.join("bin/ollama.exe"));
        }
        if let Some(registry_path) = windows_app_path("ollama.exe") {
            candidates.push(registry_path);
        }
        // A recursive registry search can block for a long time on locked or
        // redirected Windows hives. Keep startup deterministic; users with a
        // non-standard moved installation can use AEGIS_OLLAMA_PATH.
        if std::env::var_os("AEGIS_SCAN_OLLAMA_REGISTRY").is_some() {
            candidates.extend(windows_uninstall_paths());
        }
    } else {
        candidates.extend([
            PathBuf::from("/usr/local/bin/ollama"),
            PathBuf::from("/usr/bin/ollama"),
            PathBuf::from("/opt/homebrew/bin/ollama"),
        ]);
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn which_program_path(name: &str) -> Option<PathBuf> {
    let cmd = if cfg!(windows) { "where" } else { "which" };
    let output = Command::new(cmd).arg(name).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

#[cfg(windows)]
fn windows_app_path(executable: &str) -> Option<PathBuf> {
    for hive in ["HKCU", "HKLM"] {
        let key =
            format!(r"{hive}\Software\Microsoft\Windows\CurrentVersion\App Paths\{executable}");
        let output = Command::new("reg")
            .args(["query", &key, "/ve"])
            .output()
            .ok()?;
        if !output.status.success() {
            continue;
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            if let Some(index) = line.find("REG_SZ") {
                let path = PathBuf::from(line[index + "REG_SZ".len()..].trim());
                if path.is_file() {
                    return Some(path);
                }
            }
        }
    }
    None
}

#[cfg(windows)]
fn windows_uninstall_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for key in [
        r"HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall",
        r"HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
    ] {
        let Ok(output) = Command::new("reg")
            .args(["query", key, "/s", "/f", "Ollama"])
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            continue;
        }
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let trimmed = line.trim();
            if let Some(index) = trimmed.find("REG_SZ") {
                let value = trimmed[index + "REG_SZ".len()..].trim().trim_matches('"');
                if value.is_empty() {
                    continue;
                }
                let candidate =
                    PathBuf::from(value.split(',').next().unwrap_or(value).trim_matches('"'));
                if candidate.is_file()
                    && candidate
                        .file_name()
                        .is_some_and(|name| name.eq_ignore_ascii_case("ollama.exe"))
                {
                    paths.push(candidate);
                } else if candidate.is_dir() {
                    paths.push(candidate.join("ollama.exe"));
                }
            }
        }
    }
    paths
}

#[cfg(not(windows))]
fn windows_app_path(_executable: &str) -> Option<PathBuf> {
    None
}

#[cfg(not(windows))]
fn windows_uninstall_paths() -> Vec<PathBuf> {
    Vec::new()
}

/// Check if Ollama has at least one model pulled. If not, pull the default model.
fn ensure_ollama_model(report: &mut RuntimeStartReport) -> bool {
    let default_model =
        std::env::var("AEGIS_DEFAULT_MODEL").unwrap_or_else(|_| "llama3.2:latest".to_string());

    if !uses_ollama_provider() {
        return true;
    }

    // First check if ollama is reachable at all
    let Some(ollama_program) = resolve_ollama_program() else {
        if !report
            .warnings
            .iter()
            .any(|warning| warning.starts_with("Ollama was not found"))
        {
            report.warnings.push(
                "Ollama executable was not found. Set `AEGIS_OLLAMA_PATH` if it was moved."
                    .to_string(),
            );
        }
        return false;
    };

    // Check if ollama serve is running
    if !service_reachable("http://127.0.0.1:11434/api/tags") {
        if !report
            .warnings
            .iter()
            .any(|warning| warning.contains("Ollama"))
        {
            report.warnings.push(
                "Ollama server is not running on http://127.0.0.1:11434. Start it with `ollama serve`."
                    .to_string(),
            );
        }
        return false;
    }

    // Check if any models exist
    let has_models = Command::new(&ollama_program)
        .args(["list"])
        .output()
        .ok()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.lines().count() > 1
        })
        .unwrap_or(false);

    if has_models {
        return true;
    }

    // No models — pull the default one
    println!(
        "  No Ollama models found. Pulling default model `{default_model}` (this may take a while)..."
    );

    let plan = model_pull_plan_for_name(&default_model, &ollama_program);
    match run_foreground(&plan) {
        Ok(()) => {
            println!("  Default model `{default_model}` pulled successfully.");
        }
        Err(e) => {
            report.warnings.push(format!(
                "Could not pull default model `{default_model}`: {e}. Run `ollama pull {default_model}` manually."
            ));
            return false;
        }
    }

    true
}

fn model_pull_plan_for_name(model_name: &str, ollama_program: &Path) -> LaunchPlan {
    LaunchPlan {
        label: format!("Model pull ({model_name})"),
        program: ollama_program.display().to_string(),
        args: vec!["pull".to_string(), model_name.to_string()],
        cwd: std::env::current_dir().unwrap_or_default(),
        env: Vec::new(),
    }
}

fn ensure_frontend_runtime(
    workspace: &Workspace,
    logs_dir: &Path,
    report: &mut RuntimeStartReport,
) {
    let web_url = workspace.web_ui_url();

    if service_reachable(&web_url) {
        report.already_running.push(format!("Web UI ({web_url})"));
        return;
    }

    if !workspace.frontend_manifest().exists() {
        report.warnings.push(format!(
            "Web UI was not started because `{}` was not found.",
            workspace.frontend_manifest().display()
        ));
        return;
    }

    if existing_background_process_running(logs_dir, "Web UI") {
        report.warnings.push(format!(
            "Web UI appears to already be starting, but `{web_url}` is not healthy yet. Check `{}`.",
            log_path(logs_dir, "Web UI").display()
        ));
        return;
    }

    let plan = LaunchPlan {
        label: "Web UI".to_string(),
        program: npm_program(),
        args: vec![
            "run".to_string(),
            "dev".to_string(),
            "--".to_string(),
            "--host".to_string(),
            "127.0.0.1".to_string(),
            "--port".to_string(),
            workspace.frontend_dev_port().unwrap_or(5173).to_string(),
            "--strictPort".to_string(),
        ],
        cwd: workspace.frontend_dir.clone(),
        env: Vec::new(),
    };

    match spawn_background(&plan, logs_dir) {
        Ok(()) => {
            report.started.push("Web UI".to_string());
            if !wait_for_service(&web_url, Duration::from_secs(12)) {
                report.warnings.push(format!(
                    "Web UI was started in the background but did not answer `{web_url}` yet. Check `{}`.",
                    log_path(logs_dir, &plan.label).display()
                ));
            }
        }
        Err(error) => report.warnings.push(error),
    }
}

pub fn engine_launch_plan(workspace: &Workspace) -> Option<LaunchPlan> {
    let running_from_install = is_packaged_install(workspace);

    // Keep the CLI and engine API in sync when running from a source checkout.
    if !running_from_install
        && workspace.root.join(".git").exists()
        && workspace.engine_manifest().exists()
    {
        return Some(LaunchPlan {
            label: "Engine".to_string(),
            program: "cargo".to_string(),
            args: vec!["run".to_string()],
            cwd: workspace.engine_dir.clone(),
            env: vec![
                (
                    "CARGO_TARGET_DIR".to_string(),
                    workspace.engine_target_dir(false).display().to_string(),
                ),
                (
                    "AEGIS_RAG_URL".to_string(),
                    std::env::var("AEGIS_RAG_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string()),
                ),
                (
                    "AEGIS_ENGINE_HOST".to_string(),
                    std::env::var("AEGIS_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                ),
                (
                    "AEGIS_ENGINE_PORT".to_string(),
                    std::env::var("AEGIS_ENGINE_PORT").unwrap_or_else(|_| "8080".to_string()),
                ),
            ],
        });
    }

    // Priority 1: compiled binary in install_root/bin/
    let install_binary = workspace.install_root.join("bin").join("aegis-engine.exe");
    if install_binary.exists() {
        return Some(LaunchPlan {
            label: "Engine".to_string(),
            program: install_binary.to_string_lossy().to_string(),
            args: Vec::new(),
            cwd: workspace.install_root.clone(),
            env: vec![
                (
                    "AEGIS_RAG_URL".to_string(),
                    std::env::var("AEGIS_RAG_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string()),
                ),
                (
                    "AEGIS_ENGINE_HOST".to_string(),
                    std::env::var("AEGIS_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
                ),
                (
                    "AEGIS_ENGINE_PORT".to_string(),
                    std::env::var("AEGIS_ENGINE_PORT").unwrap_or_else(|_| "8080".to_string()),
                ),
            ],
        });
    }

    // Priority 2: for developers — use cargo run from the engine directory
    if workspace.engine_manifest().exists() {
        Some(LaunchPlan {
            label: "Engine".to_string(),
            program: "cargo".to_string(),
            args: vec!["run".to_string()],
            cwd: workspace.engine_dir.clone(),
            env: vec![
                (
                    "CARGO_TARGET_DIR".to_string(),
                    workspace.engine_target_dir(false).display().to_string(),
                ),
                (
                    "AEGIS_RAG_URL".to_string(),
                    std::env::var("AEGIS_RAG_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:8000".to_string()),
                ),
            ],
        })
    } else {
        None
    }
}

pub fn model_pull_plan(workspace: &Workspace, model_name: &str) -> LaunchPlan {
    LaunchPlan {
        label: format!("Model pull ({model_name})"),
        program: "ollama".to_string(),
        args: vec!["pull".to_string(), model_name.to_string()],
        cwd: workspace.root.clone(),
        env: Vec::new(),
    }
}

pub fn run_foreground(plan: &LaunchPlan) -> AppResult<()> {
    let mut command = make_command(plan);
    command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = command.status().map_err(|error| {
        format!(
            "Could not start {}: {}",
            plan.label,
            render_spawn_error(&plan.program, error)
        )
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with status {status}.", plan.label))
    }
}

pub fn run_supervisor(ui: &Ui, plans: Vec<LaunchPlan>) -> AppResult<()> {
    let mut children = Vec::new();

    for plan in plans {
        let mut command = make_command(&plan);
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command.spawn().map_err(|error| {
            format!(
                "Could not start {}: {}",
                plan.label,
                render_spawn_error(&plan.program, error)
            )
        })?;

        let stdout_handle = child
            .stdout
            .take()
            .map(|stdout| spawn_reader(plan.label.clone(), false, stdout));
        let stderr_handle = child
            .stderr
            .take()
            .map(|stderr| spawn_reader(plan.label.clone(), true, stderr));

        children.push((plan.label, child, stdout_handle, stderr_handle));
    }

    let mut failures = 0usize;

    for (label, mut child, stdout_handle, stderr_handle) in children {
        let status = child
            .wait()
            .map_err(|error| format!("Could not wait for {label}: {error}"))?;

        if let Some(handle) = stdout_handle {
            let _ = handle.join();
        }
        if let Some(handle) = stderr_handle {
            let _ = handle.join();
        }

        if status.success() {
            println!(
                "{} {} exited cleanly.",
                ui.badge(crate::doctor::Health::Info),
                label
            );
        } else {
            failures += 1;
            eprintln!(
                "{} {} exited with status {status}.",
                ui.badge(crate::doctor::Health::Missing),
                label
            );
        }
    }

    if failures == 0 {
        Ok(())
    } else {
        Err(format!(
            "{failures} launched process(es) exited with errors."
        ))
    }
}

fn spawn_reader<R>(label: String, stderr: bool, reader: R) -> JoinHandle<()>
where
    R: io::Read + Send + 'static,
{
    thread::spawn(move || {
        let buffered = BufReader::new(reader);
        for line in buffered.lines().map_while(Result::ok) {
            if stderr {
                eprintln!("[{label}] {line}");
            } else {
                println!("[{label}] {line}");
            }
        }
    })
}

fn make_command(plan: &LaunchPlan) -> Command {
    let mut command = Command::new(&plan.program);
    command.args(&plan.args).current_dir(&plan.cwd);

    for (key, value) in &plan.env {
        command.env(key, value);
    }

    command
}

pub fn spawn_background(plan: &LaunchPlan, logs_dir: &Path) -> AppResult<()> {
    let log_path = log_path(logs_dir, &plan.label);
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| {
            format!(
                "Could not open `{}` for {} logs: {error}",
                log_path.display(),
                plan.label
            )
        })?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("Could not prepare {} stderr logging: {error}", plan.label))?;

    let mut command = make_command(plan);
    command
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));
    configure_background_command(&mut command);

    let child = command.spawn().map_err(|error| {
        format!(
            "Could not start {} using `{}`: {}",
            plan.label,
            plan.command_preview(),
            render_spawn_error(&plan.program, error)
        )
    })?;

    write_pid_file(logs_dir, &plan.label, child.id());
    Ok(())
}

#[cfg(windows)]
fn configure_background_command(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_background_command(_command: &mut Command) {}

fn write_pid_file(logs_dir: &Path, label: &str, pid: u32) {
    let _ = fs::write(pid_path(logs_dir, label), pid.to_string());
}

fn existing_background_process_running(logs_dir: &Path, label: &str) -> bool {
    let path = pid_path(logs_dir, label);
    let Ok(metadata) = fs::metadata(&path) else {
        return false;
    };

    // A PID can be reused after a service exits. Treat old markers as stale so
    // a dead service cannot block recovery forever, while leaving a generous
    // window for first-run Cargo compilation and model initialization.
    if metadata
        .modified()
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age > Duration::from_secs(10 * 60))
    {
        let _ = fs::remove_file(&path);
        return false;
    }

    let Ok(raw_pid) = fs::read_to_string(&path) else {
        return false;
    };

    let Ok(pid) = raw_pid.trim().parse::<u32>() else {
        return false;
    };

    process_running(pid)
}

#[cfg(windows)]
fn process_running(pid: u32) -> bool {
    Command::new("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .is_some_and(|stdout| stdout.contains(&pid.to_string()))
}

#[cfg(not(windows))]
fn process_running(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn pid_path(logs_dir: &Path, label: &str) -> PathBuf {
    logs_dir.join(format!("{}.pid", sanitize_label(label)))
}

fn log_path(logs_dir: &Path, label: &str) -> PathBuf {
    logs_dir.join(format!("{}.log", sanitize_label(label)))
}

fn sanitize_label(label: &str) -> String {
    label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

pub fn service_reachable(url: &str) -> bool {
    reqwest::blocking::Client::new()
        .get(url)
        .timeout(Duration::from_millis(800))
        .send()
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

pub fn wait_for_service(url: &str, timeout: Duration) -> bool {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if service_reachable(url) {
            return true;
        }
        thread::sleep(Duration::from_millis(500));
    }

    false
}

pub fn join_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn uses_ollama_provider() -> bool {
    configured_provider()
        .as_deref()
        .map(|provider| provider == "ollama")
        .unwrap_or(true)
}

fn configured_provider() -> Option<String> {
    if let Ok(provider) = std::env::var("AEGIS_INFERENCE_PROVIDER") {
        let provider = provider.trim().to_ascii_lowercase();
        if !provider.is_empty() {
            return Some(normalize_provider_name(&provider));
        }
    }
    let data_dir = std::env::var_os("AEGIS_DATA_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            if cfg!(windows) {
                std::env::var_os("APPDATA")
                    .map(PathBuf::from)
                    .map(|path| path.join("AEGIS"))
            } else {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|path| path.join(".local/share/AEGIS"))
            }
        })?;
    fs::read_to_string(data_dir.join("active_provider.txt"))
        .ok()
        .map(|value| normalize_provider_name(value.trim()))
}

fn normalize_provider_name(provider: &str) -> String {
    match provider.trim().to_ascii_lowercase().as_str() {
        "lm-studio" | "lm_studio" => "lmstudio".to_string(),
        "openai-compat" | "openai_compatible" => "openai-compatible".to_string(),
        other => other.to_string(),
    }
}

fn python_program_for_rag(rag_dir: &Path) -> String {
    let windows_venv = rag_dir.join("rag-env").join("Scripts").join("python.exe");
    if windows_venv.exists() {
        return windows_venv.display().to_string();
    }

    let unix_venv = rag_dir.join("rag-env").join("bin").join("python");
    if unix_venv.exists() {
        return unix_venv.display().to_string();
    }

    "python".to_string()
}

fn npm_program() -> String {
    if cfg!(windows) {
        "npm.cmd".to_string()
    } else {
        "npm".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_preview_includes_arguments() {
        let plan = LaunchPlan {
            label: "Engine".to_string(),
            program: "cargo".to_string(),
            args: vec![
                "run".to_string(),
                "-p".to_string(),
                "aegis-engine".to_string(),
            ],
            cwd: PathBuf::from("."),
            env: Vec::new(),
        };

        assert_eq!(plan.command_preview(), "cargo run -p aegis-engine");
    }

    #[test]
    fn command_preview_handles_no_arguments() {
        let plan = LaunchPlan {
            label: "AEGIS".to_string(),
            program: "aegis".to_string(),
            args: Vec::new(),
            cwd: PathBuf::from("."),
            env: Vec::new(),
        };

        assert_eq!(plan.command_preview(), "aegis");
    }

    #[test]
    fn join_url_handles_extra_slashes() {
        assert_eq!(
            join_url("http://127.0.0.1:8000/", "/health"),
            "http://127.0.0.1:8000/health"
        );
    }

    #[test]
    fn sanitize_label_makes_log_safe_names() {
        assert_eq!(sanitize_label("RAG Engine #1"), "rag-engine--1");
        assert_eq!(sanitize_label("  Engine  "), "engine");
    }

    #[test]
    fn source_checkout_prefers_matching_engine_source() {
        let workspace = Workspace::discover();
        if workspace.root.join(".git").exists() && workspace.engine_manifest().exists() {
            let plan = engine_launch_plan(&workspace).expect("source engine plan");
            assert_eq!(plan.program, "cargo");
            assert_eq!(plan.cwd, workspace.engine_dir);
        }
    }
}

fn render_runtime_report(ui: &Ui, report: &RuntimeStartReport) {
    if !report.started.is_empty() {
        println!(
            "{} {}",
            ui.success("Started local AEGIS services:"),
            report.started.join(", ")
        );
        println!("{}", ui.muted(&format!("Web UI: {}", report.web_url)));
    } else if report.warnings.is_empty() && report.starting.is_empty() {
        println!("{}", ui.success("AEGIS services are ready."));
    }

    if ui.verbose && !report.already_running.is_empty() {
        println!(
            "{}",
            ui.muted(&format!(
                "Already running: {}",
                report.already_running.join(", ")
            ))
        );
    }

    if !report.starting.is_empty() {
        println!(
            "{}",
            ui.muted(&format!(
                "Still starting: {}. AEGIS will keep waiting while the active model warms.",
                report.starting.join(", ")
            ))
        );
    }

    for warning in &report.warnings {
        println!("{}", ui.warning(&format!("Runtime auto-start: {warning}")));
    }
}

pub fn render_runtime_start_report(ui: &Ui, report: &RuntimeStartReport) {
    render_runtime_report(ui, report);
}

fn render_spawn_error(program: &str, error: io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        format!("`{program}` was not found on PATH.")
    } else {
        error.to_string()
    }
}

/// Services that can be managed (logs, stop, restart).
pub const SERVICE_NAMES: &[&str] = &["engine", "rag", "web-ui"];

/// Resolve the logs directory from workspace install root.
pub fn logs_dir_path(workspace: &crate::workspace::Workspace) -> PathBuf {
    if is_packaged_install(workspace) {
        // The installer runs elevated, so files under the install directory
        // may be owned by Administrators and unwritable to the normal user.
        // Keep packaged runtime logs in the guaranteed-writable temp area.
        return std::env::temp_dir().join("aegis").join("logs");
    }

    workspace.root.join(".aegis").join("logs")
}

fn is_packaged_install(workspace: &Workspace) -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .is_some_and(|bin_dir| {
            bin_dir
                .file_name()
                .is_some_and(|name| name.eq_ignore_ascii_case("bin"))
                && bin_dir == workspace.install_root.join("bin")
                && bin_dir.join("aegis-engine.exe").is_file()
        })
}

/// Read the last `n` lines from a service's log file.
pub fn read_service_log(logs_dir: &Path, service: &str, n: usize) -> AppResult<String> {
    let log_path = log_path(logs_dir, service);
    if !log_path.exists() {
        return Err(format!(
            "No log file found for `{service}` at `{}`.",
            log_path.display()
        ));
    }
    let content = fs::read_to_string(&log_path)
        .map_err(|e| format!("Could not read `{}`: {e}", log_path.display()))?;
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    let start = if total > n { total - n } else { 0 };
    Ok(lines[start..].join("\n"))
}

/// Stop a background service by reading its PID file and killing the process.
pub fn stop_service(logs_dir: &Path, service: &str) -> AppResult<bool> {
    let pid_path = pid_path(logs_dir, service);
    let Ok(raw_pid) = fs::read_to_string(&pid_path) else {
        return Ok(false);
    };
    let Ok(pid) = raw_pid.trim().parse::<u32>() else {
        return Ok(false);
    };

    let killed = if cfg!(windows) {
        Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        Command::new("kill")
            .args(["-9", &pid.to_string()])
            .output()
            .ok()
            .map(|o| o.status.success())
            .unwrap_or(false)
    };

    if killed {
        let _ = fs::remove_file(&pid_path);
    }
    Ok(killed)
}
