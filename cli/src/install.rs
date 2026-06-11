use std::path::PathBuf;

use crate::ui::Ui;
use crate::workspace::Workspace;
use crate::AppResult;

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub root: PathBuf,
    pub root_source: String,
    pub steps: Vec<InstallStep>,
}

#[derive(Debug, Clone)]
pub struct InstallStep {
    pub label: String,
    pub description: String,
}

pub fn build_install_plan(
    workspace: &Workspace,
    install_root: PathBuf,
    install_root_source: String,
) -> InstallPlan {
    InstallPlan {
        root: install_root,
        root_source: install_root_source,
        steps: vec![
            InstallStep {
                label: "Engine binary".to_string(),
                description: format!(
                    "Build the Rust engine from {}",
                    workspace.engine_dir.display()
                ),
            },
            InstallStep {
                label: "CLI binary".to_string(),
                description: "Build the AEGIS CLI for the current platform.".to_string(),
            },
            InstallStep {
                label: "RAG runtime".to_string(),
                description: format!(
                    "Copy Python RAG files from {}",
                    workspace.rag_dir.display()
                ),
            },
            InstallStep {
                label: "Frontend assets".to_string(),
                description: "Build and copy the Web UI frontend.".to_string(),
            },
        ],
    }
}

pub fn print_install_plan(ui: &Ui, plan: &InstallPlan) {
    println!("{}", ui.header("Install Plan"));
    println!("Install root : {}", plan.root.display());
    println!("Root source  : {}", plan.root_source);
    println!();
    for (index, step) in plan.steps.iter().enumerate() {
        println!("{}. {}: {}", index + 1, step.label, step.description);
    }
}

pub fn persist_install_root(ui: &Ui, install_root: &PathBuf) -> AppResult<()> {
    let config_dir = dirs_or_default();
    std::fs::create_dir_all(&config_dir)
        .map_err(|error| format!("Could not create config directory `{}`: {error}", config_dir.display()))?;

    let config_path = config_dir.join("install-root");
    std::fs::write(&config_path, install_root.display().to_string())
        .map_err(|error| format!("Could not write install root to `{}`: {error}", config_path.display()))?;

    if ui.verbose {
        println!(
            "{}",
            ui.muted(&format!(
                "Saved install root preference to `{}`.",
                config_path.display()
            ))
        );
    }

    Ok(())
}

fn dirs_or_default() -> PathBuf {
    let config_dir = std::env::var("APPDATA")
        .or_else(|_| std::env::var("XDG_CONFIG_HOME"))
        .or_else(|_| std::env::var("HOME").map(|home| format!("{home}/.config")))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(config_dir).join("aegis")
}
