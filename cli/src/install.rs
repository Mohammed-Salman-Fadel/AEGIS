//! Role: Windows-first installation scaffold and staged dependency plan.
//! Called by: `commands.rs` for the `aegis install` flow.
//! Calls into: `workspace.rs` for paths, `ui.rs` for rendering, and eventually `runner.rs` for subprocess execution.
//! Owns: the install step list, platform notes, and TODO guidance for dependency setup.
//! Does not own: actual package downloads, backend startup, or doctor validation logic.
//! Next TODOs: map each step to real subprocess plans and persist install progress once the workflow is approved.

use crate::ui::Ui;
use crate::workspace::Workspace;

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub summary: String,
    pub steps: Vec<InstallStep>,
}

#[derive(Debug, Clone)]
pub struct InstallStep {
    pub name: String,
    pub platform: String,
    pub description: String,
    pub verification_hint: String,
}

pub fn build_install_plan(workspace: &Workspace) -> InstallPlan {
    let root = workspace.root.display().to_string();

    InstallPlan {
        summary: format!(
            "Scaffold-only install plan for {root}. Windows is the primary target for the first real implementation."
        ),
        // The install plan is intentionally data-first so `commands.rs` can render it now and
        // `runner.rs` can execute the same stages later without duplicating the order.
        steps: vec![
            InstallStep {
                name: "Install Rust toolchain".to_string(),
                platform: "Windows (primary)".to_string(),
                description: "TODO: use runner.rs to bootstrap Rust so the CLI and engine crates can build."
                    .to_string(),
                verification_hint: "Verify `cargo --version` and `rustc --version`.".to_string(),
            },
            InstallStep {
                name: "Install Ollama".to_string(),
                platform: "Windows (primary)".to_string(),
                description: "TODO: add a Windows installer flow for Ollama and confirm the service is reachable."
                    .to_string(),
                verification_hint: "Verify `ollama --help` and later `ollama list`.".to_string(),
            },
            InstallStep {
                name: "Install Python runtime".to_string(),
                platform: "Windows (primary)".to_string(),
                description: "TODO: scaffold Python installation for the future RAG service.".to_string(),
                verification_hint: "Verify `python --version` or `py -3 --version`.".to_string(),
            },
            InstallStep {
                name: "Install Node.js and npm".to_string(),
                platform: "Windows (primary)".to_string(),
                description: "TODO: add a frontend dependency installer once the frontend gets a real app manifest."
                    .to_string(),
                verification_hint: "Verify `node --version` and `npm --version`.".to_string(),
            },
            InstallStep {
                name: "Pull a local model".to_string(),
                platform: "Windows (primary)".to_string(),
                description: "TODO: use runner.rs to execute `ollama pull <model>` once model selection is finalized."
                    .to_string(),
                verification_hint: "Verify the model appears in `ollama list`.".to_string(),
            },
            InstallStep {
                name: "Bootstrap the Rust engine".to_string(),
                platform: "Windows (primary)".to_string(),
                description: "TODO: use runner.rs to build or start the engine after dependencies are present."
                    .to_string(),
                verification_hint: "Verify the future engine `/health` endpoint responds.".to_string(),
            },
            InstallStep {
                name: "Post-install verification".to_string(),
                platform: "Windows (primary)".to_string(),
                description: "TODO: run `aegis doctor` automatically after install succeeds.".to_string(),
                verification_hint: "Verify doctor reports no blocking issues for supported flows.".to_string(),
            },
            InstallStep {
                name: "Linux support placeholder".to_string(),
                platform: "Linux".to_string(),
                description: "TODO: translate the Windows-first flow into package-manager-friendly Linux steps."
                    .to_string(),
                verification_hint: "Document distro-specific verification commands later.".to_string(),
            },
            InstallStep {
                name: "macOS support placeholder".to_string(),
                platform: "macOS".to_string(),
                description: "TODO: translate the Windows-first flow into Homebrew or installer-based macOS steps."
                    .to_string(),
                verification_hint: "Document macOS verification commands later.".to_string(),
            },
        ],
    }
}

pub fn print_install_plan(ui: &Ui, plan: &InstallPlan) {
    println!("{}", ui.header("Install Scaffold"));
    println!("{}", plan.summary);
    println!(
        "{}",
        ui.muted("This command intentionally documents the staged installer instead of performing the real setup today.")
    );
    println!();

    for (index, step) in plan.steps.iter().enumerate() {
        println!("{}. {} [{}]", index + 1, step.name, step.platform);
        println!("   {}", step.description);
        println!("   Verify: {}", step.verification_hint);
    }
}
