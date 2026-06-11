use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ComponentState {
    Ready,
    Scaffolded,
    Missing,
}

#[derive(Debug, Clone)]
pub struct ComponentInfo {
    pub name: String,
    pub state: ComponentState,
    pub note: String,
    pub path: PathBuf,
    pub launchable: bool,
}

#[derive(Debug, Clone)]
pub struct Workspace {
    pub root: PathBuf,
    pub install_root: PathBuf,
    pub default_install_root: PathBuf,
    pub frontend_dir: PathBuf,
    pub engine_dir: PathBuf,
    pub rag_dir: PathBuf,
    pub cli_dir: PathBuf,
}

impl Workspace {
    pub fn discover() -> Self {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let project_root = Workspace::find_workspace_root(&cwd);
        let default_install_root = project_root.join("dist");

        Self {
            root: project_root.clone(),
            install_root: std::env::var("AEGIS_INSTALL_ROOT")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| default_install_root.clone()),
            default_install_root,
            frontend_dir: project_root.join("frontend"),
            engine_dir: project_root.join("engine"),
            rag_dir: project_root.join("rag-python"),
            cli_dir: project_root.join("cli"),
        }
    }

    /// Walk up from `start` looking for a directory that contains `.git`.
    /// If none found, fall back to the first ancestor with `Cargo.toml` containing `[workspace]`,
    /// then to the first ancestor with any `Cargo.toml`.
    fn find_workspace_root(start: &Path) -> PathBuf {
        let mut git_root: Option<PathBuf> = None;
        let mut cargo_root: Option<PathBuf> = None;
        let mut workspace_cargo_root: Option<PathBuf> = None;

        let mut current = Some(start.to_path_buf());
        while let Some(dir) = current {
            if dir.join(".git").exists() {
                git_root = Some(dir.clone());
            }
            let cargo_toml = dir.join("Cargo.toml");
            if cargo_toml.exists() {
                if cargo_toml_has_workspace(&cargo_toml) {
                    workspace_cargo_root = Some(dir.clone());
                }
                if cargo_root.is_none() {
                    cargo_root = Some(dir.clone());
                }
            }
            if git_root.is_some() && workspace_cargo_root.is_some() {
                break;
            }
            current = dir.parent().map(PathBuf::from);
        }

        git_root
            .or(workspace_cargo_root)
            .or(cargo_root)
            .unwrap_or_else(|| start.to_path_buf())
    }

    pub fn normalize_install_root(path: &Path) -> PathBuf {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if canonical.is_absolute() {
            canonical
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(&canonical)
        }
    }

    pub fn web_ui_url(&self) -> String {
        std::env::var("AEGIS_WEB_UI_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:5173".to_string())
    }

    pub fn components(&self) -> Vec<ComponentInfo> {
        vec![
            self.engine_component(),
            self.frontend_component(),
            self.rag_component(),
            self.installer_component(),
        ]
    }

    pub fn engine_component(&self) -> ComponentInfo {
        ComponentInfo {
            name: "Engine".to_string(),
            state: if self.engine_manifest().exists() {
                ComponentState::Scaffolded
            } else {
                ComponentState::Missing
            },
            note: format!("Engine source at {}", self.engine_dir.display()),
            path: self.engine_dir.clone(),
            launchable: self.engine_manifest().exists(),
        }
    }

    pub fn frontend_component(&self) -> ComponentInfo {
        ComponentInfo {
            name: "Frontend".to_string(),
            state: if self.frontend_manifest().exists() {
                ComponentState::Scaffolded
            } else {
                ComponentState::Missing
            },
            note: format!("Frontend source at {}", self.frontend_dir.display()),
            path: self.frontend_dir.clone(),
            launchable: self.frontend_manifest().exists(),
        }
    }

    pub fn rag_component(&self) -> ComponentInfo {
        ComponentInfo {
            name: "RAG".to_string(),
            state: if self.rag_runtime_defined() {
                ComponentState::Scaffolded
            } else {
                ComponentState::Missing
            },
            note: format!("RAG source at {}", self.rag_dir.display()),
            path: self.rag_dir.clone(),
            launchable: self.rag_runtime_defined(),
        }
    }

    pub fn installer_component(&self) -> ComponentInfo {
        ComponentInfo {
            name: "Installer".to_string(),
            state: ComponentState::Scaffolded,
            note: "Local install scaffold is ready.".to_string(),
            path: self.root.join("installer"),
            launchable: false,
        }
    }

    pub fn cli_build_target_dir(&self, release: bool) -> PathBuf {
        let base = self.cli_dir.join("target");
        if release {
            base.join("release")
        } else {
            base.join("debug")
        }
    }

    pub fn engine_manifest(&self) -> PathBuf {
        self.engine_dir.join("Cargo.toml")
    }

    pub fn frontend_manifest(&self) -> PathBuf {
        self.frontend_dir.join("package.json")
    }

    pub fn engine_target_dir(&self, release: bool) -> PathBuf {
        let base = self.engine_dir.join("target");
        if release {
            base.join("release")
        } else {
            base.join("debug")
        }
    }

    pub fn rag_runtime_defined(&self) -> bool {
        self.rag_dir.join("app").exists() || self.rag_dir.join("requirements.txt").exists()
    }
}

fn cargo_toml_has_workspace(path: &Path) -> bool {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return false,
    };
    content.lines().any(|line| {
        let trimmed = line.trim();
        trimmed == "[workspace]" || trimmed.starts_with("[workspace.")
    })
}
