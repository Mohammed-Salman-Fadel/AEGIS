#![allow(dead_code)]

//! Role: subprocess scaffolding for future engine startup, installer steps, and model downloads.
//! Called by: `commands.rs` for status previews and later by `install.rs` when real execution is approved.
//! Calls into: the host operating system through `std::process::Command`.
//! Owns: generic launch-plan descriptions and process execution helpers.
//! Does not own: command routing, dependency decisions, or CLI argument parsing.
//! Next TODOs: map installer and engine flows onto these helpers once the approved OS-specific commands are finalized.

use std::io::{self, BufRead, BufReader};
use std::fs::{self, File};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread::{self, JoinHandle};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use crate::AppResult;
use crate::ui::Ui;
use crate::workspace::Workspace;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: std::path::PathBuf,
    pub env: Vec<(String, String)>,
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

pub fn engine_launch_plan(workspace: &Workspace) -> Option<LaunchPlan> {
    if !workspace.engine_manifest().exists() {
        return None;
    }

    Some(LaunchPlan {
        label: "Engine".to_string(),
        program: "cargo".to_string(),
        args: vec!["run".to_string()],
        cwd: workspace.engine_dir.clone(),
        env: vec![(
            "CARGO_TARGET_DIR".to_string(),
            workspace.engine_target_dir(false).display().to_string(),
        )],
    })
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

fn render_spawn_error(program: &str, error: io::Error) -> String {
    if error.kind() == io::ErrorKind::NotFound {
        format!("`{program}` was not found on PATH.")
    } else {
        error.to_string()
    }
}

pub fn spawn_detached_process(
    program: &Path,
    args: &[String],
    cwd: &Path,
    env: &[(String, String)],
    stdout_log: &Path,
    stderr_log: &Path,
) -> AppResult<u32> {
    if let Some(parent) = stdout_log.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create process log directory `{}`: {error}",
                parent.display()
            )
        })?;
    }
    if let Some(parent) = stderr_log.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create process log directory `{}`: {error}",
                parent.display()
            )
        })?;
    }

    let stdout = File::create(stdout_log).map_err(|error| {
        format!(
            "Could not create stdout log `{}`: {error}",
            stdout_log.display()
        )
    })?;
    let stderr = File::create(stderr_log).map_err(|error| {
        format!(
            "Could not create stderr log `{}`: {error}",
            stderr_log.display()
        )
    })?;

    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr));

    for (key, value) in env {
        command.env(key, value);
    }

    #[cfg(windows)]
    command.creation_flags(CREATE_NO_WINDOW);

    let child = command.spawn().map_err(|error| {
        format!(
            "Could not start `{}` in the background: {}",
            program.display(),
            render_spawn_error(&program.display().to_string(), error)
        )
    })?;

    Ok(child.id())
}

pub fn stop_process(pid: u32) -> AppResult<()> {
    #[cfg(windows)]
    {
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Could not stop process {pid}: {error}"))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("taskkill could not stop process {pid}."))
        }
    }

    #[cfg(not(windows))]
    {
        let status = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .map_err(|error| format!("Could not stop process {pid}: {error}"))?;

        if status.success() {
            Ok(())
        } else {
            Err(format!("kill could not stop process {pid}."))
        }
    }
}

pub fn is_process_running(pid: u32) -> bool {
    #[cfg(windows)]
    {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("$p = Get-Process -Id {pid} -ErrorAction SilentlyContinue; if ($p) {{ exit 0 }} else {{ exit 1 }}"),
            ])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

pub fn open_in_browser(url: &str) -> AppResult<()> {
    #[cfg(windows)]
    {
        Command::new("explorer")
            .arg(url)
            .spawn()
            .map_err(|error| format!("Could not open browser for `{url}`: {error}"))?;
        Ok(())
    }

    #[cfg(not(windows))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|error| format!("Could not open browser for `{url}`: {error}"))?;
        Ok(())
    }
}

pub fn expand_zip_archive(zip_path: &Path, destination: &Path) -> AppResult<()> {
    let destination = destination.display().to_string();
    let zip_path = zip_path.display().to_string();

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
                zip_path.replace('\'', "''"),
                destination.replace('\'', "''")
            ),
        ])
        .status()
        .map_err(|error| format!("Could not extract `{zip_path}`: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("PowerShell could not extract `{zip_path}`."))
    }
}

pub fn sha256_file(path: &Path) -> AppResult<String> {
    let path = path.display().to_string();
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            &format!(
                "(Get-FileHash -LiteralPath '{}' -Algorithm SHA256).Hash",
                path.replace('\'', "''")
            ),
        ])
        .output()
        .map_err(|error| format!("Could not hash `{path}`: {error}"))?;

    if !output.status.success() {
        return Err(format!("Could not hash `{path}` with PowerShell."));
    }

    let hash = String::from_utf8_lossy(&output.stdout).trim().to_lowercase();
    if hash.is_empty() {
        Err(format!("Hash output for `{path}` was empty."))
    } else {
        Ok(hash)
    }
}

pub fn run_program_and_wait(program: &str, args: &[String], cwd: &Path) -> AppResult<()> {
    let status = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| format!("Could not start `{program}`: {}", render_spawn_error(program, error)))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("`{program}` exited with status {status}."))
    }
}
