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
const RAG_INIT_PATH: &str = "/init";
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
    pub warnings: Vec<String>,
    pub web_url: String,
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

    let logs_dir = workspace.root.join(".aegis").join("logs");
    if let Err(error) = fs::create_dir_all(&logs_dir) {
        report.warnings.push(format!(
            "Could not create AEGIS log directory `{}`: {error}",
            logs_dir.display()
        ));
    }

    ensure_rag_runtime(workspace, &logs_dir, &mut report);
    ensure_ollama_runtime(workspace, &logs_dir, &mut report);
    ensure_engine_runtime(workspace, &logs_dir, &mut report);
    render_runtime_report(ui, &report);

    report
}

fn ensure_rag_runtime(workspace: &Workspace, logs_dir: &Path, report: &mut RuntimeStartReport) {
    let base_url =
        std::env::var("AEGIS_RAG_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());
    let health_url = join_url(&base_url, RAG_HEALTH_PATH);

    if service_reachable(&health_url) {
        report.already_running.push(format!("RAG ({base_url})"));
        let _ = initialize_rag(&base_url);
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
        report.warnings.push(format!(
            "RAG appears to already be starting, but `{health_url}` is not healthy yet. Check `{}`.",
            log_path(logs_dir, "RAG").display()
        ));
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
            if wait_for_service(&health_url, Duration::from_secs(8)) {
                if let Err(error) = initialize_rag(&base_url) {
                    report.warnings.push(format!(
                        "RAG started, but `/init` did not complete yet: {error}"
                    ));
                }
            } else {
                report.warnings.push(format!(
                    "RAG was started in the background but did not answer `{health_url}` yet. Check `{}`.",
                    log_path(logs_dir, &plan.label).display()
                ));
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
        report.warnings.push(format!(
            "Ollama appears to already be starting, but `{health_url}` is not healthy yet. Check `{}`.",
            log_path(logs_dir, "Ollama").display()
        ));
        return;
    }

    let plan = LaunchPlan {
        label: "Ollama".to_string(),
        program: "ollama".to_string(),
        args: vec!["serve".to_string()],
        cwd: workspace.root.clone(),
        env: Vec::new(),
    };

    match spawn_background(&plan, logs_dir) {
        Ok(()) => {
            report.started.push("Ollama".to_string());
            if !wait_for_service(&health_url, Duration::from_secs(25)) {
                report.warnings.push(format!(
                    "Ollama was started in the background but did not answer `{health_url}` yet. Model warmup may fail until Ollama is ready. Check `{}`.",
                    log_path(logs_dir, &plan.label).display()
                ));
            }
        }
        Err(error) => report.warnings.push(error),
    }
}

fn ensure_engine_runtime(workspace: &Workspace, logs_dir: &Path, report: &mut RuntimeStartReport) {
    let base_url =
        std::env::var("AEGIS_ENGINE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let health_url = join_url(&base_url, ENGINE_HEALTH_PATH);

    if service_reachable(&health_url) {
        report.already_running.push(format!("Engine ({base_url})"));
        return;
    }

    // Preflight: ensure at least one Ollama model is available before starting the engine.
    // The engine aborts startup if warm_active_model() finds nothing, so we pull
    // a default model here if Ollama is empty.
    ensure_ollama_model(report);

    let Some(plan) = engine_launch_plan(workspace) else {
        report.warnings.push(
            "Engine was not started because no engine binary or Cargo.toml was found.".to_string(),
        );
        return;
    };

    if existing_background_process_running(logs_dir, "Engine") {
        report.warnings.push(format!(
            "Engine appears to already be starting, but `{health_url}` is not healthy yet. Check `{}`.",
            log_path(logs_dir, "Engine").display()
        ));
        return;
    }

    match spawn_background(&plan, logs_dir) {
        Ok(()) => {
            report.started.push("Engine".to_string());
            if !wait_for_service(&health_url, Duration::from_secs(30)) {
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
fn probe_ollama_cli() -> bool {
    // First try PATH
    if which_program("ollama") {
        return true;
    }

    // Fallback: check known install paths (PATH may have been corrupted)
    let known_paths = if cfg!(windows) {
        vec![
            format!(
                r"{}\AppData\Local\Programs\Ollama\ollama.exe",
                std::env::var("USERPROFILE").unwrap_or_default()
            ),
            r"C:\Program Files\Ollama\ollama.exe".to_string(),
            r"C:\Program Files (x86)\Ollama\ollama.exe".to_string(),
        ]
    } else {
        vec![
            "/usr/local/bin/ollama".to_string(),
            "/usr/bin/ollama".to_string(),
        ]
    };

    for path in &known_paths {
        if std::path::Path::new(path).exists() {
            // Temporarily add to PATH so subsequent commands work
            if let Ok(current) = std::env::var("PATH") {
                if let Some(parent) = std::path::Path::new(path).parent() {
                    let dir = parent.to_string_lossy();
                    if !current.contains(dir.as_ref()) {
                        // SAFETY: We're modifying our own process's PATH to find Ollama.
                        // This is safe because it only affects the current process, not the
                        // system or other processes, and it only appends to the existing PATH.
                        unsafe {
                            std::env::set_var("PATH", format!("{};{}", dir, current));
                        }
                    }
                }
            }
            return true;
        }
    }

    false
}
fn which_program(name: &str) -> bool {
    let cmd = if cfg!(windows) { "where" } else { "which" };
    Command::new(cmd)
        .arg(name)
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if Ollama has at least one model pulled. If not, pull the default model.
fn ensure_ollama_model(report: &mut RuntimeStartReport) {
    let default_model =
        std::env::var("AEGIS_DEFAULT_MODEL").unwrap_or_else(|_| "qwen3:4b".to_string());

    // First check if ollama is reachable at all
    if !probe_ollama_cli() {
        report.warnings.push(
            "Ollama CLI not found on PATH. Install Ollama and pull a model before using AEGIS."
                .to_string(),
        );
        return;
    }

    // Check if ollama serve is running
    if !service_reachable("http://127.0.0.1:11434/api/tags") {
        report.warnings.push(
            "Ollama server is not running on http://127.0.0.1:11434. Start it with `ollama serve`."
                .to_string(),
        );
        return;
    }

    // Check if any models exist
    let has_models = Command::new("ollama")
        .args(["list"])
        .output()
        .ok()
        .map(|o| {
            let stdout = String::from_utf8_lossy(&o.stdout);
            stdout.lines().count() > 1
        })
        .unwrap_or(false);

    if has_models {
        return;
    }

    // No models — pull the default one
    println!(
        "  No Ollama models found. Pulling default model `{default_model}` (this may take a while)..."
    );

    let plan = model_pull_plan_for_name(&default_model);
    match run_foreground(&plan) {
        Ok(()) => {
            println!("  Default model `{default_model}` pulled successfully.");
        }
        Err(e) => {
            report.warnings.push(format!(
                "Could not pull default model `{default_model}`: {e}. Run `ollama pull {default_model}` manually."
            ));
        }
    }
}

fn model_pull_plan_for_name(model_name: &str) -> LaunchPlan {
    LaunchPlan {
        label: format!("Model pull ({model_name})"),
        program: "ollama".to_string(),
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
    let Ok(raw_pid) = fs::read_to_string(pid_path(logs_dir, label)) else {
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

fn initialize_rag(base_url: &str) -> AppResult<()> {
    let init_url = join_url(base_url, RAG_INIT_PATH);
    let response = reqwest::blocking::Client::new()
        .post(&init_url)
        .timeout(Duration::from_secs(20))
        .send()
        .map_err(|error| format!("Could not call `{init_url}`: {error}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("`{init_url}` returned HTTP {}.", response.status()))
    }
}

pub fn join_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

fn uses_ollama_provider() -> bool {
    std::env::var("AEGIS_INFERENCE_PROVIDER")
        .map(|provider| matches!(provider.trim().to_ascii_lowercase().as_str(), "" | "ollama"))
        .unwrap_or(true)
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
}

fn render_runtime_report(ui: &Ui, report: &RuntimeStartReport) {
    if !report.started.is_empty() {
        println!(
            "{} {}",
            ui.success("Started local AEGIS services:"),
            report.started.join(", ")
        );
        println!("{}", ui.muted(&format!("Web UI: {}", report.web_url)));
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

    for warning in &report.warnings {
        println!("{}", ui.warning(&format!("Runtime auto-start: {warning}")));
    }
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
    workspace.install_root.join(".aegis").join("logs")
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
