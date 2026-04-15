//! Role: read-only dependency and workspace preflight checks for the CLI scaffold.
//! Called by: `commands.rs` for `status` and `doctor`.
//! Calls into: `workspace.rs` for component detection and the host environment for command probes.
//! Owns: the health model used to summarize missing dependencies and scaffolded subsystems.
//! Does not own: installation, engine startup, or any mutation of the local machine.
//! Next TODOs: feed these checks into the installer flow and compare them with live engine `/health` output.

use std::process::Command;

use crate::workspace::{ComponentInfo, ComponentState, Workspace};

#[derive(Debug, Clone)]
pub struct CheckItem {
    pub name: String,
    pub health: Health,
    pub detail: String,
    pub guidance: Option<String>,
    pub blocking: bool,
}

#[derive(Debug, Clone)]
pub struct DoctorReport {
    pub dependencies: Vec<CheckItem>,
    pub components: Vec<CheckItem>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Health {
    Ok,
    Info,
    Warn,
    Missing,
}

impl DoctorReport {
    pub fn collect(workspace: &Workspace) -> Self {
        let engine = workspace.engine_component();
        let frontend = workspace.frontend_component();
        let rag = workspace.rag_component();
        let installer = workspace.installer_component();

        let dependencies = vec![
            dependency_from_probe(
                "cargo",
                probe_command("cargo", &["--version"]),
                true,
                "Install the Rust toolchain so the CLI and engine can compile.".to_string(),
            ),
            dependency_from_probe(
                "rustc",
                probe_command("rustc", &["--version"]),
                true,
                "Install the Rust compiler so the scaffold can be built locally.".to_string(),
            ),
            ollama_check(),
            python_check(workspace.rag_runtime_defined()),
            node_check(frontend.launchable),
            package_manager_check(frontend.launchable),
        ];

        let components = vec![
            component_check(
                &engine,
                true,
                "Finish the engine entrypoint and expose the planned localhost HTTP endpoints."
                    .to_string(),
            ),
            component_check(
                &frontend,
                false,
                "Add a real frontend app manifest once the web UI is ready to be launched."
                    .to_string(),
            ),
            component_check(
                &rag,
                false,
                "Add Python project files when you are ready to enable the RAG runtime."
                    .to_string(),
            ),
            component_check(
                &installer,
                false,
                "Replace install placeholders with real system setup steps when approved."
                    .to_string(),
            ),
        ];

        Self {
            dependencies,
            components,
        }
    }

    pub fn blocking_issues(&self) -> usize {
        self.dependencies
            .iter()
            .chain(self.components.iter())
            .filter(|item| item.blocking && matches!(item.health, Health::Warn | Health::Missing))
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.dependencies
            .iter()
            .chain(self.components.iter())
            .filter(|item| matches!(item.health, Health::Warn))
            .count()
    }

    pub fn missing(&self) -> usize {
        self.dependencies
            .iter()
            .chain(self.components.iter())
            .filter(|item| matches!(item.health, Health::Missing))
            .count()
    }

    pub fn setup_actions(&self) -> Vec<String> {
        let mut actions = Vec::new();

        for item in self
            .dependencies
            .iter()
            .chain(self.components.iter())
            .filter(|item| matches!(item.health, Health::Warn | Health::Missing))
        {
            if let Some(guidance) = &item.guidance {
                if !actions.contains(guidance) {
                    actions.push(guidance.clone());
                }
            }
        }

        if actions.is_empty() {
            actions.push(
                "Everything needed for the currently scaffolded CLI flows looks available."
                    .to_string(),
            );
        }

        actions
    }
}

fn dependency_from_probe(
    name: &str,
    probe: Option<String>,
    blocking: bool,
    guidance: String,
) -> CheckItem {
    match probe {
        Some(detail) => CheckItem {
            name: name.to_string(),
            health: Health::Ok,
            detail,
            guidance: None,
            blocking: false,
        },
        None => CheckItem {
            name: name.to_string(),
            health: Health::Missing,
            detail: format!("{name} was not found on PATH."),
            guidance: Some(guidance),
            blocking,
        },
    }
}

fn ollama_check() -> CheckItem {
    match probe_command("ollama", &["--help"]) {
        Some(_) => CheckItem {
            name: "ollama".to_string(),
            health: Health::Ok,
            detail: "Ollama CLI detected on PATH.".to_string(),
            guidance: None,
            blocking: false,
        },
        None => CheckItem {
            name: "ollama".to_string(),
            health: Health::Missing,
            detail: "Ollama was not found on PATH.".to_string(),
            guidance: Some(
                "Install Ollama and download at least one local model before enabling inference."
                    .to_string(),
            ),
            blocking: true,
        },
    }
}

fn component_check(component: &ComponentInfo, blocking: bool, guidance: String) -> CheckItem {
    let (health, item_blocking) = match component.state {
        ComponentState::Ready => (Health::Ok, false),
        ComponentState::Scaffolded => {
            if component.name == "Installer" {
                (Health::Info, false)
            } else {
                (Health::Warn, blocking)
            }
        }
        ComponentState::Missing => {
            if component.name == "Installer" {
                (Health::Info, false)
            } else {
                (Health::Missing, blocking)
            }
        }
    };

    CheckItem {
        name: format!("{} component", component.name),
        health,
        detail: component.note.clone(),
        guidance: if matches!(health, Health::Warn | Health::Missing) {
            Some(guidance)
        } else {
            None
        },
        blocking: item_blocking,
    }
}

fn python_check(rag_runtime_required: bool) -> CheckItem {
    let probe = probe_any(&[("python", &["--version"]), ("py", &["-3", "--version"])]);

    match (probe, rag_runtime_required) {
        (Some(detail), true) => CheckItem {
            name: "python".to_string(),
            health: Health::Ok,
            detail: format!("{detail} (needed for the future RAG runtime)"),
            guidance: None,
            blocking: false,
        },
        (Some(detail), false) => CheckItem {
            name: "python".to_string(),
            health: Health::Info,
            detail: format!("{detail} (optional until the RAG runtime is implemented)"),
            guidance: None,
            blocking: false,
        },
        (None, true) => CheckItem {
            name: "python".to_string(),
            health: Health::Missing,
            detail: "Python was not found on PATH, so the future RAG service cannot start."
                .to_string(),
            guidance: Some(
                "Install Python so the retrieval service can run once its project files are in place."
                    .to_string(),
            ),
            blocking: true,
        },
        (None, false) => CheckItem {
            name: "python".to_string(),
            health: Health::Info,
            detail: "Python is not installed yet, but the current RAG folder is still documentation-only."
                .to_string(),
            guidance: None,
            blocking: false,
        },
    }
}

fn node_check(frontend_required: bool) -> CheckItem {
    match probe_command("node", &["--version"]) {
        Some(detail) if frontend_required => CheckItem {
            name: "node".to_string(),
            health: Health::Ok,
            detail: format!("{detail} (needed for the future frontend dev server)"),
            guidance: None,
            blocking: false,
        },
        Some(detail) => CheckItem {
            name: "node".to_string(),
            health: Health::Info,
            detail: format!("{detail} (optional until the frontend gains a real app manifest)"),
            guidance: None,
            blocking: false,
        },
        None if frontend_required => CheckItem {
            name: "node".to_string(),
            health: Health::Missing,
            detail: "Node.js was not found on PATH, so the frontend cannot be launched."
                .to_string(),
            guidance: Some(
                "Install Node.js so the CLI can support the frontend once a package.json exists."
                    .to_string(),
            ),
            blocking: true,
        },
        None => CheckItem {
            name: "node".to_string(),
            health: Health::Info,
            detail: "Node.js is not installed yet, but the frontend folder does not have a package.json right now."
                .to_string(),
            guidance: None,
            blocking: false,
        },
    }
}

fn package_manager_check(frontend_required: bool) -> CheckItem {
    match probe_any(&[
        ("npm", &["--version"]),
        ("pnpm", &["--version"]),
        ("yarn", &["--version"]),
    ]) {
        Some(detail) if frontend_required => CheckItem {
            name: "package manager".to_string(),
            health: Health::Ok,
            detail: format!("{detail} (frontend command runner detected)"),
            guidance: None,
            blocking: false,
        },
        Some(detail) => CheckItem {
            name: "package manager".to_string(),
            health: Health::Info,
            detail: format!("{detail} (optional until the frontend gains a package manifest)"),
            guidance: None,
            blocking: false,
        },
        None if frontend_required => CheckItem {
            name: "package manager".to_string(),
            health: Health::Missing,
            detail: "No supported frontend package manager was found on PATH.".to_string(),
            guidance: Some(
                "Install npm, pnpm, or yarn so the CLI can launch the frontend when it is ready."
                    .to_string(),
            ),
            blocking: true,
        },
        None => CheckItem {
            name: "package manager".to_string(),
            health: Health::Info,
            detail: "No frontend package manager was found, but the current frontend folder is still a placeholder."
                .to_string(),
            guidance: None,
            blocking: false,
        },
    }
}

fn probe_any(candidates: &[(&str, &[&str])]) -> Option<String> {
    candidates.iter().find_map(|(program, args)| {
        probe_command(program, args).map(|detail| format!("{detail} via {program}"))
    })
}

fn probe_command(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let line = stdout
        .lines()
        .chain(stderr.lines())
        .map(str::trim)
        .find(|line| !line.is_empty())?;

    Some(line.to_string())
}
