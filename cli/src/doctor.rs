//! Role: read-only dependency and workspace preflight checks for the CLI scaffold.
//! Called by: `commands.rs` for `status` and `doctor`.
//! Calls into: `workspace.rs` for component detection and the host environment for command probes.
//! Owns: the health model used to summarize missing dependencies and scaffolded subsystems.
//! Does not own: installation, engine startup, or any mutation of the local machine.
//! Next TODOs: feed these checks into the installer flow and compare them with live engine `/health` output.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use crate::engine_client::EngineClient;
use crate::runtime::{RuntimeLayout, DEFAULT_MODEL, DEFAULT_OLLAMA_URL};
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

    pub fn collect_installed(runtime: &RuntimeLayout, engine: &EngineClient) -> Self {
        let model_name = runtime
            .load_install_state()
            .map(|state| state.default_model)
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());
        let engine_health = engine.health();

        let dependencies = vec![
            runtime_path_check(
                "install state",
                &runtime.install_state_path,
                "Installed runtime metadata found.".to_string(),
                "Run `aegis install` to create install-state.json for the managed runtime."
                    .to_string(),
                true,
            ),
            runtime_path_check(
                "runtime bundle",
                &runtime.current_runtime_dir,
                format!(
                    "Active runtime directory present at `{}`.",
                    runtime.current_runtime_dir.display()
                ),
                "Run `aegis install` again so the runtime bundle is staged and promoted."
                    .to_string(),
                true,
            ),
            runtime_path_check(
                "CLI launcher",
                &runtime.user_cli_exe(),
                format!(
                    "CLI launcher found at `{}`.",
                    runtime.user_cli_exe().display()
                ),
                "Re-run `aegis install` so the per-user CLI launcher is synced into %LOCALAPPDATA%\\AEGIS\\bin."
                    .to_string(),
                true,
            ),
            runtime_path_check(
                "engine launcher",
                &runtime.user_engine_exe(),
                format!(
                    "Engine launcher found at `{}`.",
                    runtime.user_engine_exe().display()
                ),
                "Re-run `aegis install` so the engine launcher is synced into %LOCALAPPDATA%\\AEGIS\\bin."
                    .to_string(),
                true,
            ),
            runtime_path_check(
                "UI assets",
                &runtime.installed_ui_dir().join("index.html"),
                format!(
                    "Frontend bundle found at `{}`.",
                    runtime.installed_ui_dir().join("index.html").display()
                ),
                "Re-run `aegis install` so the built frontend is bundled under the managed runtime."
                    .to_string(),
                true,
            ),
            runtime_path_check(
                "engine env",
                &runtime.engine_env_path,
                format!("Engine env file found at `{}`.", runtime.engine_env_path.display()),
                "Run `aegis install` so the bootstrap flow writes engine.env with host, port, UI, and model settings."
                    .to_string(),
                true,
            ),
            runtime_dir_check(
                "data dir",
                &runtime.data_dir,
                "Future runtime data directory is present.".to_string(),
                "Run `aegis install` so the managed data directory is created.".to_string(),
                false,
            ),
            runtime_dir_check(
                "logs dir",
                &runtime.logs_dir,
                "Runtime log directory is present.".to_string(),
                "Run `aegis install` so the managed log directory is created.".to_string(),
                false,
            ),
            runtime_dir_check(
                "run dir",
                &runtime.run_dir,
                "Runtime PID and temp directory is present.".to_string(),
                "Run `aegis install` so the managed run directory is created.".to_string(),
                false,
            ),
            ollama_runtime_check(),
            model_runtime_check(&model_name),
            engine_runtime_check(&engine_health),
        ];

        let components = vec![
            CheckItem {
                name: "Bootstrap installer".to_string(),
                health: if runtime.is_installed() {
                    Health::Ok
                } else {
                    Health::Warn
                },
                detail: if runtime.is_installed() {
                    "Managed bootstrap layout detected under %LOCALAPPDATA%\\AEGIS."
                        .to_string()
                } else {
                    "The managed bootstrap layout is incomplete.".to_string()
                },
                guidance: if runtime.is_installed() {
                    None
                } else {
                    Some(
                        "Run `aegis install` from the bootstrap EXE to finish the per-user runtime setup."
                            .to_string(),
                    )
                },
                blocking: !runtime.is_installed(),
            },
            CheckItem {
                name: "Engine process".to_string(),
                health: if engine_health.reachable {
                    Health::Ok
                } else if runtime.user_engine_exe().exists() {
                    Health::Warn
                } else {
                    Health::Missing
                },
                detail: if engine_health.reachable {
                    "The installed engine responded to /health.".to_string()
                } else if runtime.user_engine_exe().exists() {
                    "The engine binary exists, but the localhost engine is not responding yet."
                        .to_string()
                } else {
                    "The engine binary is not installed.".to_string()
                },
                guidance: if engine_health.reachable {
                    None
                } else {
                    Some(
                        "Use `aegis start` to launch the managed engine, or re-run `aegis install` if the runtime looks incomplete."
                            .to_string(),
                    )
                },
                blocking: true,
            },
            CheckItem {
                name: "Static web UI".to_string(),
                health: if runtime.installed_ui_dir().join("index.html").exists() {
                    Health::Ok
                } else {
                    Health::Missing
                },
                detail: if runtime.installed_ui_dir().join("index.html").exists() {
                    "Static UI assets are ready to be served by the engine at `/`.".to_string()
                } else {
                    "No built UI assets were found under the installed runtime.".to_string()
                },
                guidance: if runtime.installed_ui_dir().join("index.html").exists() {
                    None
                } else {
                    Some(
                        "Rebuild the frontend bundle and republish the runtime zip so `ui/index.html` is included."
                            .to_string(),
                    )
                },
                blocking: true,
            },
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

fn runtime_path_check(
    name: &str,
    path: &Path,
    ok_detail: String,
    guidance: String,
    blocking: bool,
) -> CheckItem {
    if path.exists() {
        CheckItem {
            name: name.to_string(),
            health: Health::Ok,
            detail: ok_detail,
            guidance: None,
            blocking: false,
        }
    } else {
        CheckItem {
            name: name.to_string(),
            health: Health::Missing,
            detail: format!("`{}` is missing.", path.display()),
            guidance: Some(guidance),
            blocking,
        }
    }
}

fn runtime_dir_check(
    name: &str,
    path: &Path,
    ok_detail: String,
    guidance: String,
    blocking: bool,
) -> CheckItem {
    if path.is_dir() {
        let writable_hint = fs::metadata(path)
            .map(|metadata| {
                if metadata.permissions().readonly() {
                    " (directory is currently read-only)"
                } else {
                    ""
                }
            })
            .unwrap_or("");

        CheckItem {
            name: name.to_string(),
            health: if writable_hint.is_empty() {
                Health::Ok
            } else {
                Health::Warn
            },
            detail: format!("{ok_detail}{writable_hint}"),
            guidance: if writable_hint.is_empty() {
                None
            } else {
                Some(
                    "Adjust the directory permissions so the runtime can write logs, PID files, and future data."
                        .to_string(),
                )
            },
            blocking: !writable_hint.is_empty() && blocking,
        }
    } else {
        CheckItem {
            name: name.to_string(),
            health: Health::Missing,
            detail: format!("Directory `{}` is missing.", path.display()),
            guidance: Some(guidance),
            blocking,
        }
    }
}

fn ollama_runtime_check() -> CheckItem {
    let installed = find_ollama_exe().is_some();
    let reachable = probe_http_json(OLLAMA_TAGS_URL);

    match (installed, reachable) {
        (true, true) => CheckItem {
            name: "ollama".to_string(),
            health: Health::Ok,
            detail: format!("Ollama is installed and reachable at {DEFAULT_OLLAMA_URL}."),
            guidance: None,
            blocking: false,
        },
        (true, false) => CheckItem {
            name: "ollama".to_string(),
            health: Health::Warn,
            detail: format!(
                "Ollama appears to be installed, but `{}` is not responding.",
                OLLAMA_TAGS_URL
            ),
            guidance: Some(
                "Start Ollama or re-run `aegis install` so the bootstrap flow can repair the local provider setup."
                    .to_string(),
            ),
            blocking: true,
        },
        (false, _) => CheckItem {
            name: "ollama".to_string(),
            health: Health::Missing,
            detail: "Ollama is not installed or could not be located.".to_string(),
            guidance: Some(
                "Run `aegis install` so the bootstrap flow can download and install Ollama for this user."
                    .to_string(),
            ),
            blocking: true,
        },
    }
}

fn model_runtime_check(model_name: &str) -> CheckItem {
    if !probe_http_json(OLLAMA_TAGS_URL) {
        return CheckItem {
            name: "default model".to_string(),
            health: Health::Warn,
            detail: format!(
                "Could not verify whether `{model_name}` is present because Ollama is not reachable."
            ),
            guidance: Some(
                "Bring Ollama online first, then re-run `aegis doctor` or `aegis install`."
                    .to_string(),
            ),
            blocking: true,
        };
    }

    if ollama_has_model(model_name) {
        CheckItem {
            name: "default model".to_string(),
            health: Health::Ok,
            detail: format!("Ollama model `{model_name}` is available."),
            guidance: None,
            blocking: false,
        }
    } else {
        CheckItem {
            name: "default model".to_string(),
            health: Health::Missing,
            detail: format!("Ollama model `{model_name}` is not installed."),
            guidance: Some(
                "Run `aegis install` so the bootstrap flow can pull the default local model."
                    .to_string(),
            ),
            blocking: true,
        }
    }
}

fn engine_runtime_check(health: &crate::engine_client::EngineHealth) -> CheckItem {
    if health.reachable {
        CheckItem {
            name: "engine /health".to_string(),
            health: Health::Ok,
            detail: health.note.clone(),
            guidance: None,
            blocking: false,
        }
    } else {
        CheckItem {
            name: "engine /health".to_string(),
            health: Health::Warn,
            detail: health.note.clone(),
            guidance: Some(
                "Use `aegis start` to launch the managed engine, or inspect the runtime logs under %LOCALAPPDATA%\\AEGIS\\logs."
                    .to_string(),
            ),
            blocking: true,
        }
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
        ("npm.cmd", &["--version"]),
        ("pnpm", &["--version"]),
        ("pnpm.cmd", &["--version"]),
        ("yarn", &["--version"]),
        ("yarn.cmd", &["--version"]),
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

const OLLAMA_TAGS_URL: &str = "http://127.0.0.1:11434/api/tags";

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

fn probe_http_json(url: &str) -> bool {
    reqwest::blocking::get(url)
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

fn ollama_has_model(model_name: &str) -> bool {
    reqwest::blocking::get(OLLAMA_TAGS_URL)
        .ok()
        .and_then(|response| response.error_for_status().ok())
        .and_then(|response| response.json::<serde_json::Value>().ok())
        .and_then(|value| value.get("models").cloned())
        .and_then(|models| models.as_array().cloned())
        .map(|models| {
            models.iter().any(|model| {
                model.get("name")
                    .and_then(|name| name.as_str())
                    .map(|name| name.eq_ignore_ascii_case(model_name))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

fn find_ollama_exe() -> Option<String> {
    let path_lookup = probe_command("where", &["ollama.exe"]);
    if path_lookup.is_some() {
        return path_lookup;
    }

    env::var_os("LOCALAPPDATA").and_then(|local_app_data| {
        let candidate = Path::new(&local_app_data)
            .join("Programs")
            .join("Ollama")
            .join("ollama.exe");
        candidate.exists().then(|| candidate.display().to_string())
    })
}
