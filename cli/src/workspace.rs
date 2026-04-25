//! Role: repository and component path discovery for the CLI scaffold.
//! Called by: `main.rs`, `doctor.rs`, `install.rs`, and `runner.rs`.
//! Calls into: the local filesystem only.
//! Owns: workspace root detection, component location, and component scaffold/readiness notes.
//! Does not own: launching processes, backend networking, or command interpretation.
//! Next TODOs: load path overrides from shared config and expose richer engine/runtime discovery rules.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub cli_dir: PathBuf,
    pub engine_dir: PathBuf,
    pub frontend_dir: PathBuf,
    pub installer_dir: PathBuf,
    pub rag_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: &'static str,
    pub path: PathBuf,
    pub state: ComponentState,
    pub launchable: bool,
    pub note: String,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ComponentState {
    Ready,
    Scaffolded,
    Missing,
}

impl Workspace {
    pub fn discover() -> Self {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let current_dir = env::current_dir().unwrap_or_else(|_| manifest_dir.clone());
        let root = Self::locate_root(&current_dir)
            .or_else(|| Self::locate_root(&manifest_dir))
            .unwrap_or_else(|| {
                manifest_dir
                    .parent()
                    .and_then(Path::parent)
                    .map(Path::to_path_buf)
                    .unwrap_or(manifest_dir.clone())
            });

        let cli_dir = Self::resolve_existing(&root, &["src/cli", "cli"])
            .unwrap_or_else(|| root.join("src").join("cli"));
        let engine_dir = Self::resolve_existing(&root, &["engine-rust"])
            .unwrap_or_else(|| root.join("engine-rust"));
        let frontend_dir = Self::resolve_existing(&root, &["frontend", "web-ui"])
            .unwrap_or_else(|| root.join("frontend"));
        let installer_dir =
            Self::resolve_existing(&root, &["installer"]).unwrap_or_else(|| root.join("installer"));
        let rag_dir = Self::resolve_existing(&root, &["src/rag-python", "rag-python"])
            .unwrap_or_else(|| root.join("src").join("rag-python"));

        Self {
            root,
            cli_dir,
            engine_dir,
            frontend_dir,
            installer_dir,
            rag_dir,
        }
    }

    fn locate_root(start: &Path) -> Option<PathBuf> {
        for candidate in start.ancestors() {
            let has_engine = candidate.join("engine-rust").exists();
            let has_cli =
                candidate.join("src").join("cli").exists() || candidate.join("cli").exists();

            if has_engine && has_cli {
                return Some(candidate.to_path_buf());
            }
        }

        None
    }

    fn resolve_existing(root: &Path, candidates: &[&str]) -> Option<PathBuf> {
        candidates
            .iter()
            .map(|relative| root.join(relative))
            .find(|path| path.exists())
    }

    pub fn engine_manifest(&self) -> PathBuf {
        self.engine_dir.join("Cargo.toml")
    }

    pub fn engine_entrypoint(&self) -> PathBuf {
        self.engine_dir.join("src").join("main.rs")
    }

    pub fn frontend_manifest(&self) -> PathBuf {
        self.frontend_dir.join("package.json")
    }

    pub fn frontend_vite_config(&self) -> PathBuf {
        self.frontend_dir.join("vite.config.ts")
    }

    pub fn web_ui_url(&self) -> String {
        if let Ok(url) = env::var("AEGIS_WEB_URL") {
            let url = url.trim();
            if !url.is_empty() {
                return url.to_string();
            }
        }

        let port = self.frontend_dev_port().unwrap_or(5173);

        format!("http://localhost:{port}")
    }

    pub fn installer_readme(&self) -> PathBuf {
        self.installer_dir.join("README.md")
    }

    pub fn rag_runtime_defined(&self) -> bool {
        self.rag_dir.join("pyproject.toml").exists()
            || self.rag_dir.join("requirements.txt").exists()
            || self.rag_dir.join("Pipfile").exists()
            || self.rag_dir.join("poetry.lock").exists()
    }

    pub fn engine_target_dir(&self, release: bool) -> PathBuf {
        let profile = if release { "release" } else { "debug" };
        self.cli_dir
            .join("target")
            .join("runtime")
            .join("engine")
            .join(profile)
    }

    pub fn cli_build_target_dir(&self, release: bool) -> PathBuf {
        let profile = if release { "release" } else { "debug" };
        self.cli_dir
            .join("target")
            .join("runtime")
            .join("cli")
            .join(profile)
    }

    pub fn engine_component(&self) -> ComponentInfo {
        let manifest = self.engine_manifest();
        let entrypoint = self.engine_entrypoint();

        if manifest.exists() && entrypoint.exists() {
            let source = fs::read_to_string(&entrypoint).unwrap_or_default();
            let scaffolded = source.contains("TODO:");
            let note = if scaffolded {
                "Rust engine crate found, but the main entrypoint is still scaffolded.".to_string()
            } else {
                "Rust engine entrypoint detected and ready to expose the planned HTTP surface."
                    .to_string()
            };

            return ComponentInfo {
                name: "Engine",
                path: self.engine_dir.clone(),
                state: if scaffolded {
                    ComponentState::Scaffolded
                } else {
                    ComponentState::Ready
                },
                launchable: true,
                note,
            };
        }

        if self.engine_dir.exists() {
            return ComponentInfo {
                name: "Engine",
                path: self.engine_dir.clone(),
                state: ComponentState::Missing,
                launchable: false,
                note: "The engine folder exists, but its Cargo entrypoint is incomplete."
                    .to_string(),
            };
        }

        ComponentInfo {
            name: "Engine",
            path: self.engine_dir.clone(),
            state: ComponentState::Missing,
            launchable: false,
            note: "The engine-rust folder could not be found from the current workspace."
                .to_string(),
        }
    }

    pub fn frontend_component(&self) -> ComponentInfo {
        if self.frontend_manifest().exists() {
            return ComponentInfo {
                name: "Frontend",
                path: self.frontend_dir.clone(),
                state: ComponentState::Ready,
                launchable: true,
                note: "Frontend app manifest found, so the CLI can eventually delegate startup."
                    .to_string(),
            };
        }

        if self.frontend_dir.exists() {
            return ComponentInfo {
                name: "Frontend",
                path: self.frontend_dir.clone(),
                state: ComponentState::Scaffolded,
                launchable: false,
                note: "The frontend folder exists, but no package.json was found yet.".to_string(),
            };
        }

        ComponentInfo {
            name: "Frontend",
            path: self.frontend_dir.clone(),
            state: ComponentState::Missing,
            launchable: false,
            note: "The frontend folder could not be located from the current workspace."
                .to_string(),
        }
    }

    pub fn rag_component(&self) -> ComponentInfo {
        if self.rag_runtime_defined() {
            return ComponentInfo {
                name: "RAG",
                path: self.rag_dir.clone(),
                state: ComponentState::Ready,
                launchable: true,
                note: "Python runtime files were found for the future retrieval service."
                    .to_string(),
            };
        }

        if self.rag_dir.exists() {
            return ComponentInfo {
                name: "RAG",
                path: self.rag_dir.clone(),
                state: ComponentState::Scaffolded,
                launchable: false,
                note: "The RAG folder exists, but it is still documentation-only right now."
                    .to_string(),
            };
        }

        ComponentInfo {
            name: "RAG",
            path: self.rag_dir.clone(),
            state: ComponentState::Missing,
            launchable: false,
            note: "The RAG subsystem folder could not be located from the current workspace."
                .to_string(),
        }
    }

    pub fn installer_component(&self) -> ComponentInfo {
        if self.installer_readme().exists() {
            return ComponentInfo {
                name: "Installer",
                path: self.installer_dir.clone(),
                state: ComponentState::Scaffolded,
                launchable: false,
                note: "Installer documentation exists, but the real automation flow is still TODO-only."
                    .to_string(),
            };
        }

        if self.installer_dir.exists() {
            return ComponentInfo {
                name: "Installer",
                path: self.installer_dir.clone(),
                state: ComponentState::Missing,
                launchable: false,
                note: "Installer folder found without a clear runnable entrypoint.".to_string(),
            };
        }

        ComponentInfo {
            name: "Installer",
            path: self.installer_dir.clone(),
            state: ComponentState::Missing,
            launchable: false,
            note: "The installer folder could not be located from the current workspace."
                .to_string(),
        }
    }

    pub fn components(&self) -> Vec<ComponentInfo> {
        vec![
            self.engine_component(),
            self.frontend_component(),
            self.rag_component(),
            self.installer_component(),
        ]
    }

    fn frontend_dev_port(&self) -> Option<u16> {
        let vite_config = self.frontend_vite_config();
        let source = fs::read_to_string(vite_config).ok()?;
        let marker = "port:";
        let start = source.find(marker)? + marker.len();
        let digits: String = source[start..]
            .chars()
            .skip_while(|ch| ch.is_whitespace())
            .take_while(|ch| ch.is_ascii_digit())
            .collect();

        digits.parse().ok()
    }
}
