//! Real installer — performs local dependency detection, venv creation,
//! config writing, and default model download.
//!
//! Called by: `commands.rs` for the `aegis install` flow.
//! Owns: the install step list and the `execute_install_plan` function.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use crate::ui::Ui;
use crate::workspace::Workspace;
use crate::runner::{run_foreground, LaunchPlan};

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub summary: String,
    pub install_root: PathBuf,
    pub install_root_source: String,
    pub steps: Vec<InstallStep>,
}

#[derive(Debug, Clone)]
pub struct InstallStep {
    pub name: String,
    pub description: String,
    pub action: InstallAction,
    /// If non-None, this condition is checked first. When it's satisfied,
    /// the step is skipped cleanly instead of re-executed.
    pub skip_if: Option<SkipCondition>,
}

#[derive(Debug, Clone)]
pub enum SkipCondition {
    /// Skip if path exists (file or directory)
    PathExists(PathBuf),
    /// Skip if running `<program> <args>` produces stdout/stderr containing `contains`
    CmdOutputContains { program: String, args: Vec<String>, contains: String },
}

#[derive(Debug, Clone)]
pub enum InstallAction {
    /// Run a Command and check exit code
    RunCommand {
        program: String,
        args: Vec<String>,
        cwd: Option<PathBuf>,
    },
    /// Create a directory
    CreateDir { path: PathBuf },
    /// Write a file with content
    WriteFile { path: PathBuf, content: String },
    /// Check if a program is available on PATH, warn if not
    CheckProgram { name: String, optional: bool },
}

pub fn build_install_plan(
    workspace: &Workspace,
    install_root: PathBuf,
    install_root_source: impl Into<String>,
) -> InstallPlan {
    let default_model = std::env::var("AEGIS_DEFAULT_MODEL")
        .unwrap_or_else(|_| "qwen3:4b".to_string());

    let aegis_dir = install_root.join(".aegis");
    let config_dir = aegis_dir.join("config");
    let logs_dir = aegis_dir.join("logs");
    let sessions_dir = aegis_dir.join("sessions");
    let rag_venv_dir = install_root.join("rag-env");
    let config_toml_path = config_dir.join("aegis.toml");

    let rag_venv_python = rag_venv_dir.join(if cfg!(windows) { "Scripts\\python.exe" } else { "bin/python" });
    let rag_venv_pip = rag_venv_dir.join(if cfg!(windows) { "Scripts\\pip.exe" } else { "bin/pip" });
    let python_pip = rag_venv_pip.clone();
    let requirements_exist = workspace.rag_dir.join("requirements.txt").exists();

    let mut steps: Vec<InstallStep> = Vec::new();

    // Step 1: Python check
    steps.push(InstallStep {
        name: "Python runtime check".into(),
        description: "Detect Python 3 on PATH for RAG service installation".into(),
        action: InstallAction::CheckProgram {
            name: "python".into(),
            optional: false,
        },
        skip_if: None,
    });

    // Step 2: Create Python venv (skip if already exists)
    let python_cmd = if cfg!(windows) { "python" } else { "python3" };
    steps.push(InstallStep {
        name: "Create RAG virtual environment".into(),
        description: format!("Create venv at `{}`", rag_venv_dir.display()),
        action: InstallAction::RunCommand {
            program: python_cmd.into(),
            args: vec!["-m".into(), "venv".into(), rag_venv_dir.to_string_lossy().to_string()],
            cwd: Some(install_root.clone()),
        },
        skip_if: Some(SkipCondition::PathExists(rag_venv_python.clone())),
    });

    // Step 3: Install RAG deps (skip if already installed)
    if requirements_exist {
        let req_path = workspace.rag_dir.join("requirements.txt");
        steps.push(InstallStep {
            name: "Install Python RAG dependencies".into(),
            description: format!("pip install -r `{}`", req_path.display()),
            action: InstallAction::RunCommand {
                program: python_pip.to_string_lossy().to_string(),
                args: vec![
                    "install".into(),
                    "-r".into(),
                    req_path.to_string_lossy().to_string(),
                ],
                cwd: Some(rag_venv_dir.clone()),
            },
            // Skip if chromadb can already be imported in the venv
            skip_if: Some(SkipCondition::CmdOutputContains {
                program: rag_venv_python.to_string_lossy().to_string(),
                args: vec!["-c".into(), "import chromadb; print('ok')".into()],
                contains: "ok".into(),
            }),
        });
    }

    // Step 4: Node.js check
    steps.push(InstallStep {
        name: "Node.js / npm check".into(),
        description: "Detect Node.js for optional frontend development".into(),
        action: InstallAction::CheckProgram {
            name: "node".into(),
            optional: true,
        },
        skip_if: None,
    });

    // Step 5: Ollama check
    steps.push(InstallStep {
        name: "Ollama check".into(),
        description: "Detect Ollama local LLM server on PATH".into(),
        action: InstallAction::CheckProgram {
            name: "ollama".into(),
            optional: false,
        },
        skip_if: None,
    });

    // Step 6: Rust toolchain check
    steps.push(InstallStep {
        name: "Rust toolchain check".into(),
        description: "Detect Rust compiler (optional — only needed if building from source)".into(),
        action: InstallAction::CheckProgram {
            name: "cargo".into(),
            optional: true,
        },
        skip_if: None,
    });

    // Step 7-9: Create .aegis directory structure (create_dir_all is already safe)
    steps.push(InstallStep {
        name: "Create AEGIS config directory".into(),
        description: format!("Create `{}` with config/, logs/, sessions/", aegis_dir.display()),
        action: InstallAction::CreateDir { path: config_dir.clone() },
        skip_if: Some(SkipCondition::PathExists(config_dir.join("aegis.toml"))),
    });

    steps.push(InstallStep {
        name: "Create AEGIS logs directory".into(),
        description: format!("Create `{}`", logs_dir.display()),
        action: InstallAction::CreateDir { path: logs_dir },
        skip_if: None,
    });

    steps.push(InstallStep {
        name: "Create AEGIS sessions directory".into(),
        description: format!("Create `{}`", sessions_dir.display()),
        action: InstallAction::CreateDir { path: sessions_dir },
        skip_if: None,
    });

    // Step 10: Write default aegis.toml (skip if already exists)
    let config_toml_content = format!(
        r#"# AEGIS configuration
# This file is auto-generated by `aegis install`.

[server]
host = "127.0.0.1"
port = "8080"

[inference]
provider = "ollama"
base_url = "http://127.0.0.1:11434"
# api_key = ""

[rag]
base_url = "http://127.0.0.1:8000"
venv_path = "{}"

[defaults]
model = "{}"
"#,
        rag_venv_dir.to_string_lossy().replace('\\', "\\\\"),
        default_model,
    );

    steps.push(InstallStep {
        name: "Write default aegis.toml".into(),
        description: format!("Write configuration to `{}`", config_toml_path.display()),
        action: InstallAction::WriteFile {
            path: config_toml_path.clone(),
            content: config_toml_content,
        },
        skip_if: Some(SkipCondition::PathExists(config_toml_path.clone())),
    });

    // Step 11: Pull default model from Ollama (skip if already pulled)
    steps.push(InstallStep {
        name: format!("Pull default model `{default_model}` from Ollama"),
        description: format!("Run `ollama pull {default_model}` to download the default local model"),
        action: InstallAction::RunCommand {
            program: "ollama".into(),
            args: vec!["pull".into(), default_model.clone()],
            cwd: None,
        },
        skip_if: Some(SkipCondition::CmdOutputContains {
            program: "ollama".into(),
            args: vec!["list".into()],
            contains: default_model.clone(),
        }),
    });

    InstallPlan {
        summary: format!(
            "Complete installation plan for AEGIS at `{}`. Performs dependency checks, creates RAG venv, writes config, and pulls default model.",
            install_root.display()
        ),
        install_root,
        install_root_source: install_root_source.into(),
        steps,
    }
}

/// Check whether a SkipCondition is currently satisfied.
fn check_skip(condition: &SkipCondition) -> bool {
    match condition {
        SkipCondition::PathExists(path) => path.exists(),
        SkipCondition::CmdOutputContains { program, args, contains } => {
            Command::new(program)
                .args(args)
                .output()
                .ok()
                .map(|o| {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    let stderr = String::from_utf8_lossy(&o.stderr);
                    stdout.contains(contains) || stderr.contains(contains)
                })
                .unwrap_or(false)
        }
    }
}

/// Execute all steps in the install plan.
/// Returns Ok(()) if all steps succeed.
pub fn execute_install_plan(ui: &Ui, plan: &InstallPlan) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    for (index, step) in plan.steps.iter().enumerate() {
        let step_num = index + 1;
        let total = plan.steps.len();

        // Check skip condition before executing
        if let Some(ref condition) = step.skip_if {
            if check_skip(condition) {
                println!(
                    "{} [{step_num}/{total}] {} — {}",
                    ui.header(&step.name),
                    ui.muted(&step.description),
                    ui.success("already satisfied, skipping"),
                );
                println!();
                continue;
            }
        }

        println!(
            "{} [{step_num}/{total}] {}",
            ui.header(&step.name),
            ui.muted(&step.description),
        );

        match &step.action {
            InstallAction::RunCommand { program, args, cwd } => {
                let lp = LaunchPlan {
                    label: step.name.clone(),
                    program: program.clone(),
                    args: args.clone(),
                    cwd: cwd.clone().unwrap_or_else(|| plan.install_root.clone()),
                    env: Vec::new(),
                };
                match run_foreground(&lp) {
                    Ok(()) => {
                        println!("  {}", ui.success("OK"));
                    }
                    Err(e) => {
                        let msg = format!("Step {step_num} failed: {e}");
                        println!("  {}", ui.error(&msg));
                        errors.push(msg);
                    }
                }
            }
            InstallAction::CreateDir { path } => {
                match fs::create_dir_all(path) {
                    Ok(()) => println!("  {} Created", ui.success("OK")),
                    Err(e) => {
                        let msg = format!("Could not create `{}`: {e}", path.display());
                        println!("  {}", ui.error(&msg));
                        errors.push(msg);
                    }
                }
            }
            InstallAction::WriteFile { path, content } => {
                if let Some(parent) = path.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                match fs::write(path, content) {
                    Ok(()) => println!("  {} Written", ui.success("OK")),
                    Err(e) => {
                        let msg = format!("Could not write `{}`: {e}", path.display());
                        println!("  {}", ui.error(&msg));
                        errors.push(msg);
                    }
                }
            }
            InstallAction::CheckProgram { name, optional } => {
                let found = which_program(name);
                if found {
                    println!("  {} Found `{name}` on PATH", ui.success("OK"));
                } else if *optional {
                    println!("  {} `{name}` not found (optional)", ui.warning("WARN"));
                } else {
                    let msg = format!("`{name}` is required but was not found on PATH.");
                    println!("  {}", ui.error(&msg));
                    errors.push(msg);
                }
            }
        }

        println!();
    }

    if errors.is_empty() {
        println!("{}", ui.success("Installation complete!"));
        println!("{}", ui.muted("Run `aegis open` to start the system, or `aegis doctor` to verify readiness."));
        Ok(())
    } else {
        println!("{}", ui.error(&format!(
            "Installation finished with {} error(s):",
            errors.len()
        )));
        for e in &errors {
            println!("  • {e}");
        }
        Err(errors)
    }
}

/// Check if a program exists on PATH.
fn which_program(name: &str) -> bool {
    let cmd = if cfg!(windows) { "where" } else { "which" };
    Command::new(cmd)
        .arg(name)
        .output()
        .ok()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// Keep existing helpers

pub fn print_install_plan(ui: &Ui, plan: &InstallPlan) {
    println!("{}", ui.header("Install Plan"));
    println!("{}", plan.summary);
    println!();
    println!("Install root: {}", plan.install_root.display());
    println!("Source: {}", plan.install_root_source);
    println!();

    for (index, step) in plan.steps.iter().enumerate() {
        let skip_hint = match &step.skip_if {
            Some(SkipCondition::PathExists(p)) => format!(" [skip if `{}` exists]", p.display()),
            Some(SkipCondition::CmdOutputContains { contains, .. }) => {
                format!(" [skip if already contains `{contains}`]")
            }
            None => String::new(),
        };
        println!("{}. {}{}", index + 1, step.name, skip_hint);
        println!("   {}", step.description);
    }
}

pub fn persist_install_root(ui: &Ui, install_root: &std::path::Path) -> Result<(), String> {
    let preference_path =
        Workspace::save_install_root_preference(install_root).map_err(|error| {
            format!(
                "Could not save install path preference `{}`: {error}",
                install_root.display()
            )
        })?;

    println!(
        "{}",
        ui.success(&format!(
            "Installation path preference saved: {}",
            install_root.display()
        ))
    );
    println!(
        "{}",
        ui.muted(&format!(
            "Future CLI runs will read this from `{}` unless AEGIS_INSTALL_ROOT is set.",
            preference_path.display()
        ))
    );

    Ok(())
}
