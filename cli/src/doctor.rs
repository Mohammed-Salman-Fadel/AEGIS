use std::process::Command;

use crate::engine_client::{EngineClient, EngineHealth};
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
    pub system: Vec<CheckItem>,
    pub dependencies: Vec<CheckItem>,
    pub runtime: Vec<CheckItem>,
    pub components: Vec<CheckItem>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Health {
    Ok,
    Info,
    Warn,
    Missing,
}

#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os: String,
    pub architecture: String,
    pub cpu_cores: usize,
    pub total_memory_gb: f64,
    pub free_disk_gb: f64,
    pub hostname: String,
}

impl DoctorReport {
    pub fn collect(workspace: &Workspace) -> Self {
        Self::collect_with_runtime(workspace, Vec::new())
    }

    pub fn collect_live(workspace: &Workspace, engine: &EngineClient) -> Self {
        let runtime = vec![
            engine_runtime_check(engine.health()),
            rag_runtime_check(engine.rag_health()),
            ollama_runtime_check(engine.ollama_health()),
        ];

        Self::collect_with_runtime(workspace, runtime)
    }

    fn collect_with_runtime(workspace: &Workspace, runtime: Vec<CheckItem>) -> Self {
        let system = collect_system_info(workspace);

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
            git_check(),
        ];

        let components = vec![
            component_check(
                &engine,
                true,
                "Build and start the engine from its source directory.".to_string(),
            ),
            component_check(
                &frontend,
                false,
                "Initialize the frontend package manifest and install dependencies.".to_string(),
            ),
            component_check(
                &rag,
                false,
                "Start the RAG service or check `.aegis/logs/rag.log` if auto-start did not complete.".to_string(),
            ),
            component_check(
                &installer,
                false,
                "Replace install placeholders with real system setup steps when approved.".to_string(),
            ),
        ];

        Self {
            system,
            dependencies,
            runtime,
            components,
        }
    }

    pub fn blocking_issues(&self) -> usize {
        self.dependencies
            .iter()
            .chain(self.runtime.iter())
            .chain(self.components.iter())
            .filter(|item| item.blocking && matches!(item.health, Health::Warn | Health::Missing))
            .count()
    }

    pub fn warnings(&self) -> usize {
        self.dependencies
            .iter()
            .chain(self.runtime.iter())
            .chain(self.components.iter())
            .filter(|item| matches!(item.health, Health::Warn))
            .count()
    }

    pub fn missing(&self) -> usize {
        self.dependencies
            .iter()
            .chain(self.runtime.iter())
            .chain(self.components.iter())
            .filter(|item| matches!(item.health, Health::Missing))
            .count()
    }

}

// ── System info ──────────────────────────────────────────────────────────

fn collect_system_info(workspace: &Workspace) -> Vec<CheckItem> {
    let info = gather_system_info(workspace);
    vec![
        ok_item("OS", &info.os),
        ok_item("Architecture", &info.architecture),
        ok_item("CPU cores", &info.cpu_cores.to_string()),
        CheckItem {
            name: "Memory".to_string(),
            health: memory_health(info.total_memory_gb),
            detail: format!("{:.1} GB total", info.total_memory_gb),
            guidance: None,
            blocking: false,
        },
        disk_space_check(info.free_disk_gb),
        ok_item("Hostname", &info.hostname),
        ok_item("Workspace", &workspace.root.display().to_string()),
    ]
}

fn gather_system_info(workspace: &Workspace) -> SystemInfo {
    let os = if cfg!(target_os = "windows") {
        "Windows".to_string()
    } else if cfg!(target_os = "linux") {
        "Linux".to_string()
    } else if cfg!(target_os = "macos") {
        "macOS".to_string()
    } else {
        std::env::consts::OS.to_string()
    };

    let architecture = std::env::consts::ARCH.to_string();
    let cpu_cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);

    let total_memory_gb = probe_total_memory_gb();
    let free_disk_gb = probe_free_disk_gb(workspace);
    let hostname = probe_hostname();

    SystemInfo {
        os,
        architecture,
        cpu_cores,
        total_memory_gb,
        free_disk_gb,
        hostname,
    }
}

fn probe_total_memory_gb() -> f64 {
    if cfg!(target_os = "windows") {
        if let Some(output) = probe_command_raw("wmic", &["OS", "get", "TotalVisibleMemorySize", "/value"]) {
            for line in output.lines() {
                if let Some(value) = line.trim().strip_prefix("TotalVisibleMemorySize=") {
                    if let Ok(kb) = value.trim().parse::<f64>() {
                        return kb / 1_048_576.0;
                    }
                }
            }
        }
    } else if let Some(output) = read_file("/proc/meminfo") {
        for line in output.lines() {
            if let Some(rest) = line.trim().strip_prefix("MemTotal:") {
                let kb_str: String = rest.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
                if let Ok(kb) = kb_str.parse::<f64>() {
                    return kb / 1_048_576.0;
                }
            }
        }
    }
    0.0
}

fn probe_free_disk_gb(workspace: &Workspace) -> f64 {
    let drive = workspace.root.to_string_lossy().chars().next().unwrap_or('C');
    let drive_str = format!("{}:", drive);
    if let Some(output) = probe_command_raw("wmic", &["logicaldisk", "where", &format!("DeviceID='{}'", drive_str), "get", "FreeSpace", "/value"]) {
        for line in output.lines() {
            if let Some(value) = line.trim().strip_prefix("FreeSpace=") {
                if let Ok(bytes) = value.trim().parse::<f64>() {
                    return bytes / 1_073_741_824.0;
                }
            }
        }
    }
    0.0
}

fn memory_health(gb: f64) -> Health {
    if gb >= 7.5 {
        Health::Ok
    } else if gb >= 3.5 {
        Health::Warn
    } else {
        Health::Missing
    }
}

fn disk_space_check(free_gb: f64) -> CheckItem {
    let (health, detail) = if free_gb >= 10.0 {
        (Health::Ok, format!("{:.1} GB free", free_gb))
    } else if free_gb >= 1.0 {
        (Health::Warn, format!("{:.1} GB free — low disk space", free_gb))
    } else {
        (Health::Missing, format!("{:.1} GB free — critically low", free_gb))
    };
    CheckItem {
        name: "Free disk".to_string(),
        health,
        detail,
        guidance: if matches!(health, Health::Warn | Health::Missing) {
            Some("Free up disk space. Local models and build artifacts can consume 10+ GB.".to_string())
        } else {
            None
        },
        blocking: matches!(health, Health::Missing),
    }
}

// ── Runtime checks ───────────────────────────────────────────────────────

fn engine_runtime_check(health: EngineHealth) -> CheckItem {
    runtime_check(
        "engine /health",
        health,
        "Start the AEGIS engine so its `/health` endpoint responds before you try inference."
            .to_string(),
    )
}

fn ollama_runtime_check(health: EngineHealth) -> CheckItem {
    runtime_check(
        "ollama serve",
        health,
        "Run `ollama serve` and keep it available on the configured localhost URL before you try inference."
            .to_string(),
    )
}

fn rag_runtime_check(health: EngineHealth) -> CheckItem {
    runtime_check(
        "rag /health",
        health,
        "Start the AEGIS RAG service so document indexing and retrieval can run.".to_string(),
    )
}

fn runtime_check(name: &str, health: EngineHealth, guidance: String) -> CheckItem {
    let (item_health, detail) = if health.reachable {
        (Health::Ok, format!("{} ({})", health.note, health.request_path))
    } else if health.note.contains("HTTP") {
        (Health::Warn, format!("{} ({})", health.note, health.request_path))
    } else {
        (Health::Missing, format!("{} ({})", health.note, health.request_path))
    };

    CheckItem {
        name: name.to_string(),
        health: item_health,
        detail,
        guidance: if matches!(item_health, Health::Warn | Health::Missing) {
            Some(guidance)
        } else {
            None
        },
        blocking: !health.reachable,
    }
}

// ── Dependency checks ────────────────────────────────────────────────────

fn dependency_from_probe(name: &str, probe: Option<String>, blocking: bool, guidance: String) -> CheckItem {
    match probe {
        Some(detail) => ok_item(name, &detail),
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
    match probe_command("ollama", &["--version"]) {
        Some(version) => CheckItem {
            name: "ollama".to_string(),
            health: Health::Ok,
            detail: format!("Ollama {version}"),
            guidance: None,
            blocking: false,
        },
        None => CheckItem {
            name: "ollama".to_string(),
            health: Health::Missing,
            detail: "Ollama was not found on PATH.".to_string(),
            guidance: Some(
                "Install Ollama from https://ollama.com and download at least one model."
                    .to_string(),
            ),
            blocking: true,
        },
    }
}

fn python_check(rag_required: bool) -> CheckItem {
    let probe = probe_any(&[("python", &["--version"]), ("py", &["-3", "--version"])]);
    match (probe, rag_required) {
        (Some(detail), true) => CheckItem {
            name: "python".to_string(),
            health: Health::Ok,
            detail: format!("{detail} (needed for the RAG runtime)"),
            guidance: None,
            blocking: false,
        },
        (Some(detail), false) => CheckItem {
            name: "python".to_string(),
            health: Health::Info,
            detail: format!("{detail} (optional until the RAG service is needed)"),
            guidance: None,
            blocking: false,
        },
        (None, true) => CheckItem {
            name: "python".to_string(),
            health: Health::Missing,
            detail: "Python was not found on PATH, so the RAG service cannot start.".to_string(),
            guidance: Some(
                "Install Python 3.10+ so the RAG document-retrieval service can run.".to_string(),
            ),
            blocking: true,
        },
        (None, false) => CheckItem {
            name: "python".to_string(),
            health: Health::Info,
            detail: "Python not found (optional until the RAG service is implemented).".to_string(),
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
            detail: format!("{detail} (needed for the frontend dev server)"),
            guidance: None,
            blocking: false,
        },
        Some(detail) => CheckItem {
            name: "node".to_string(),
            health: Health::Info,
            detail: format!("{detail} (optional until the frontend is ready)"),
            guidance: None,
            blocking: false,
        },
        None if frontend_required => CheckItem {
            name: "node".to_string(),
            health: Health::Missing,
            detail: "Node.js was not found on PATH, so the frontend cannot be launched.".to_string(),
            guidance: Some(
                "Install Node.js 18+ via your package manager or https://nodejs.org.".to_string(),
            ),
            blocking: true,
        },
        None => CheckItem {
            name: "node".to_string(),
            health: Health::Info,
            detail: "Node.js not found (optional until the frontend is ready).".to_string(),
            guidance: None,
            blocking: false,
        },
    }
}

fn package_manager_check(frontend_required: bool) -> CheckItem {
    match probe_any(&[("npm", &["--version"]), ("pnpm", &["--version"]), ("yarn", &["--version"])]) {
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
            detail: format!("{detail} (optional until the frontend is ready)"),
            guidance: None,
            blocking: false,
        },
        None if frontend_required => CheckItem {
            name: "package manager".to_string(),
            health: Health::Missing,
            detail: "No supported frontend package manager was found on PATH.".to_string(),
            guidance: Some("Install npm, pnpm, or yarn to manage frontend dependencies.".to_string()),
            blocking: true,
        },
        None => CheckItem {
            name: "package manager".to_string(),
            health: Health::Info,
            detail: "No package manager found (optional until the frontend is ready).".to_string(),
            guidance: None,
            blocking: false,
        },
    }
}

fn git_check() -> CheckItem {
    match probe_command("git", &["--version"]) {
        Some(version) => CheckItem {
            name: "git".to_string(),
            health: Health::Ok,
            detail: format!("{version}"),
            guidance: None,
            blocking: false,
        },
        None => CheckItem {
            name: "git".to_string(),
            health: Health::Info,
            detail: "Git was not found on PATH (optional but recommended for version control).".to_string(),
            guidance: Some("Install Git from https://git-scm.com to track changes.".to_string()),
            blocking: false,
        },
    }
}

// ── Component checks ─────────────────────────────────────────────────────

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

// ── Helpers ──────────────────────────────────────────────────────────────

fn ok_item(name: &str, detail: &str) -> CheckItem {
    CheckItem {
        name: name.to_string(),
        health: Health::Ok,
        detail: detail.to_string(),
        guidance: None,
        blocking: false,
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

fn probe_command_raw(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Some(stdout + &stderr)
}

fn read_file(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

fn probe_hostname() -> String {
    if cfg!(windows) {
        probe_command("hostname", &[])
            .unwrap_or_else(|| "unknown".to_string())
    } else {
        std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("HOST"))
            .unwrap_or_else(|_| "unknown".to_string())
    }
}
