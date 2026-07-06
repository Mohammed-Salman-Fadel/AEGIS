# AEGIS Installer — Implementation Plan (Option A)

> Hybrid Pre-Compiled Binary + First-Run Auto-Setup
> Target: Windows 10 | Branch: `installer`

---

## Table of Contents

1. [Frontend Embedding in the Rust Engine](#1-frontend-embedding-in-the-rust-engine)
2. [Engine Binary Release Build](#2-engine-binary-release-build)
3. [CLI Binary Release Build](#3-cli-binary-release-build)
4. [Enhanced `aegis install` Command](#4-enhanced-aegis-install-command)
5. [NSIS Installer Script](#5-nsis-installer-script)
6. [First-Run `aegis open` Command](#6-first-run-aegis-open-command)
7. [Runner.rs Changes (Binary Startup)](#7-runnerrs-changes-binary-startup)
8. [GitHub Actions CI](#8-github-actions-ci)
9. [Verification Commands Summary](#9-verification-commands-summary)

---

## 1. Frontend Embedding in the Rust Engine

### 1.1 Add `rust-embed` dependency

**File:** `engine/Cargo.toml`

Add after line 44 (after `rand`):

```toml
# Embedded frontend (built dist/ comes along in the .exe)
rust-embed = "8"
```

Also add `mime_guess` for better MIME detection:

```toml
mime_guess = "2"
```

If the edition is `2024`, `rust-embed` v8 works fine.

### 1.2 Create `engine/build.rs`

**File:** `engine/build.rs` (new file)

This build script ensures the frontend is built before the engine compiles, and writes the embedded asset path so the engine binary includes the `frontend/dist/` output.

```rust
use std::env;
use std::path::Path;
use std::process::Command;

fn main() {
    let frontend_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("frontend");

    let dist_dir = frontend_dir.join("dist");

    // If dist/ already exists, skip rebuild (saves time during iterative dev).
    // To force a rebuild, delete dist/ or set env FORCE_FRONTEND_BUILD=1.
    let force = env::var("FORCE_FRONTEND_BUILD").is_ok();
    if !force && dist_dir.exists() {
        println!("cargo:warning=frontend/dist/ exists — skipping frontend build");
        println!("cargo:rerun-if-changed=build.rs");
        println!("cargo:rerun-if-env-changed=FORCE_FRONTEND_BUILD");
        return;
    }

    // Check for npm
    let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };

    let status = Command::new(npm)
        .args(["run", "build"])
        .current_dir(&frontend_dir)
        .status()
        .expect("Failed to run npm run build");

    if !status.success() {
        panic!("Frontend build failed (npm run build exited with {status})");
    }

    // Ensure dist/ exists
    assert!(
        dist_dir.exists(),
        "Frontend build completed but dist/ was not created"
    );

    // Tell cargo to rerun if any frontend source changes
    println!("cargo:rerun-if-changed=../frontend/src/");
    println!("cargo:rerun-if-changed=../frontend/package.json");
    println!("cargo:rerun-if-changed=../frontend/vite.config.ts");
    println!("cargo:rerun-if-changed=../frontend/tsconfig.json");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=FORCE_FRONTEND_BUILD");
}
```

### 1.3 Rewrite `handle_static` in `engine/src/network/router.rs`

**File:** `engine/src/network/router.rs`

**Changes required:**

a) **Add imports at the top (after line 28 or similar):**

```rust
use rust_embed::Embed;
```

b) **Define the embedded asset struct (anywhere before `create_router`):**

```rust
/// Embedded frontend dist/ using rust-embed.
/// The build.rs ensures dist/ is populated before engine compiles.
#[derive(Embed)]
#[folder = "../frontend/dist/"]
#[prefix = ""]
struct FrontendAssets;
```

c) **Replace the entire `handle_static` function** (lines 47–82) with:

```rust
/// Serves the embedded React frontend from the compiled binary.
/// SPA fallback: any unknown path returns index.html.
async fn handle_static(uri: axum::http::Uri) -> Response<Body> {
    let requested_path = uri.path().trim_start_matches('/');

    // Default to index.html for root or SPA routes
    let asset_path = if requested_path.is_empty() || !FrontendAssets::get(requested_path).is_some() {
        "index.html"
    } else {
        requested_path
    };

    match FrontendAssets::get(asset_path) {
        Some(content) => {
            let mime = mime_guess::from_path(asset_path)
                .first_or_octet_stream()
                .to_string();
            Response::builder()
                .header("Content-Type", mime)
                .header("Cache-Control", "no-cache, no-store, must-revalidate")
                .body(Body::from(content.data.to_vec()))
                .unwrap_or_else(|_| Response::new(Body::from("Internal error")))
        }
        None => {
            // Final SPA fallback: serve index.html
            match FrontendAssets::get("index.html") {
                Some(content) => Response::builder()
                    .header("Content-Type", "text/html; charset=utf-8")
                    .body(Body::from(content.data.to_vec()))
                    .unwrap(),
                None => Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from("Not found"))
                    .unwrap(),
            }
        }
    }
}
```

d) **Remove the old `mime_type` function** (lines 30–44) entirely — `mime_guess` replaces it. But if `mime_type` is used elsewhere, keep it and just remove its usage here. Actually it's only used in the old `handle_static`, so delete it.

e) **Update the CORS layer** (lines 537–546). Change from:

```rust
let cors = CorsLayer::new()
    .allow_origin("http://localhost:5173".parse::<HeaderValue>().unwrap())
    .allow_methods([
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
    ])
    .allow_headers(tower_http::cors::Any);
```

To:

```rust
let cors = CorsLayer::new()
    .allow_origin(tower_http::cors::Any)  // same-origin now; CORS no longer needs restriction
    .allow_methods([
        Method::GET,
        Method::POST,
        Method::PUT,
        Method::PATCH,
        Method::DELETE,
    ])
    .allow_headers(tower_http::cors::Any);
```

### 1.4 Frontend API_BASE — no change needed

**Current value** (`frontend/src/constants/api.ts` line 3):

```ts
export const API_BASE = '/api';
```

This is already a **relative path** (`/api`), which works perfectly for same-origin embedding where the engine serves both the API and the frontend on port 8080. **No change required.**

### 1.5 Verification commands for Step 1

```bash
# From repo root:
cd frontend && npm install && npm run build
ls frontend/dist/                       # should have index.html, assets/, etc.

cd engine
cargo build 2>&1 | head -20            # build.rs should trigger frontend build, then compile engine
# OR if dist/ already exists:
FORCE_FRONTEND_BUILD=1 cargo build     # force rebuild

# Verify embedded serve works:
# (run engine, then curl http://localhost:8080 — should return HTML)
```

---

## 2. Engine Binary Release Build

### 2.1 Build script/command

**File:** `scripts/build-engine-release.bat` (new, in repo root or `scripts/` dir)

```bat
@echo off
REM scripts/build-engine-release.bat — Build the AEGIS engine as a standalone .exe
REM Usage: scripts\build-engine-release.bat

setlocal enabledelayedexpansion

echo ===== AEGIS Engine Release Build =====
echo.

REM 1. Ensure frontend is fresh
echo [1/5] Building frontend...
cd /d "%~dp0..\frontend"
call npm install || exit /b 1
call npm run build || exit /b 1
echo Frontend built successfully.
echo.

REM 2. Build the engine binary in release mode
echo [2/5] Compiling engine (release)...
cd /d "%~dp0..\engine"
cargo build --release || exit /b 1
echo Engine compiled successfully.
echo.

REM 3. Locate the output binary
set ENGINE_EXE=%~dp0..\engine\target\release\aegis-engine.exe
if not exist "%ENGINE_EXE%" (
    echo ERROR: Expected engine binary at %ENGINE_EXE%
    exit /b 1
)

REM 4. Show binary info
echo [3/5] Binary created at:
echo   %ENGINE_EXE%
for %%I in ("%ENGINE_EXE%") do echo   Size: %%~zI bytes
echo.

REM 5. Copy to staging directory
set STAGING_DIR=%~dp0..\build\release
echo [4/5] Copying to staging: %STAGING_DIR%
mkdir "%STAGING_DIR%" 2>nul
copy /Y "%ENGINE_EXE%" "%STAGING_DIR%\" || exit /b 1
echo Done.
echo.

REM 6. Verify the binary
echo [5/5] Verifying binary...
"%STAGING_DIR%\aegis-engine.exe" --version 2>nul || (
    echo NOTE: --version flag may not exist; verify by checking the binary runs.
)
echo.
echo ===== Engine release build complete =====
echo Output: %STAGING_DIR%\aegis-engine.exe
```

**Also create:** `scripts/build-engine-release.sh` (Linux/macOS companion, for CI):

```bash
#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
echo "=== AEGIS Engine Release Build ==="

echo "[1/3] Building frontend..."
cd frontend && npm install && npm run build && cd ..

echo "[2/3] Compiling engine (release)..."
cd engine && cargo build --release && cd ..

ENGINE_EXE="engine/target/release/aegis-engine.exe"
if [ -f "$ENGINE_EXE" ]; then
    echo "[3/3] Binary: $ENGINE_EXE ($(stat -f%z "$ENGINE_EXE" 2>/dev/null || stat --format=%s "$ENGINE_EXE" 2>/dev/null) bytes)"
fi
echo "=== Engine release build complete ==="
```

### 2.2 Verification

```bash
scripts/build-engine-release.bat
# Expected output:
#   [1/5] Building frontend...
#   Frontend built successfully.
#   [2/5] Compiling engine (release)...
#   Engine compiled successfully.
#   [3/5] Binary created at: ...\engine\target\release\aegis-engine.exe
#   [4/5] Copying to staging: ...\build\release
#   [5/5] Verifying binary...

# Standalone test (without frontend directory):
copy build\release\aegis-engine.exe C:\temp\test-install\
cd C:\temp\test-install
aegis-engine.exe
# Should start on 127.0.0.1:8080; browse to http://localhost:8080 — should see the UI
```

---

## 3. CLI Binary Release Build

### 3.1 Build script/command

**File:** `scripts/build-cli-release.bat` (new)

```bat
@echo off
REM scripts/build-cli-release.bat — Build the AEGIS CLI as a standalone .exe
REM Usage: scripts\build-cli-release.bat

setlocal enabledelayedexpansion

echo ===== AEGIS CLI Release Build =====
echo.

REM 1. Build the CLI binary in release mode
echo [1/4] Compiling CLI (release)...
cd /d "%~dp0..\cli"
cargo build --release || exit /b 1
echo CLI compiled successfully.
echo.

REM 2. Locate the binary
set CLI_EXE=%~dp0..\cli\target\release\aegis.exe
if not exist "%CLI_EXE%" (
    echo ERROR: Expected CLI binary at %CLI_EXE%
    exit /b 1
)

REM 3. Show binary info
echo [2/4] Binary created at:
echo   %CLI_EXE%
for %%I in ("%CLI_EXE%") do echo   Size: %%~zI bytes
echo.

REM 4. Copy to staging
set STAGING_DIR=%~dp0..\build\release
echo [3/4] Copying to staging: %STAGING_DIR%
mkdir "%STAGING_DIR%" 2>nul
copy /Y "%CLI_EXE%" "%STAGING_DIR%\" || exit /b 1
echo Done.
echo.

REM 5. Verify
echo [4/4] Verifying binary...
"%STAGING_DIR%\aegis.exe" --help || exit /b 1
echo.
echo ===== CLI release build complete =====
echo Output: %STAGING_DIR%\aegis.exe
```

### 3.2 Verification

```bash
scripts\build-cli-release.bat
# Expected:
#   aegis.exe --help prints the command list
```

---

## 4. Enhanced `aegis install` Command

This is the largest change. The current `install.rs` is purely scaffold (just prints TODO steps). We replace it with real execution.

### 4.1 Rewrite `cli/src/install.rs`

**File:** `cli/src/install.rs` — complete rewrite

The new `build_install_plan` returns a plan with real _executable_ steps. We add a new function `execute_install_plan` that performs each step.

```rust
//! Real installer — performs local dependency detection, venv creation,
//! config writing, and default model download.
//!
//! Called by: `commands.rs` for the `aegis install` flow.
//! Owns: the install step list and the `execute_install_plan` function.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::ui::Ui;
use crate::workspace::Workspace;
use crate::runner::{run_foreground, LaunchPlan};

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub summary: String,
    pub workspace_root: PathBuf,
    pub default_install_root: PathBuf,
    pub install_root: PathBuf,
    pub install_root_source: String,
    pub steps: Vec<InstallStep>,
}

#[derive(Debug, Clone)]
pub struct InstallStep {
    pub name: String,
    pub description: String,
    pub action: InstallAction,
    pub verification_hint: String,
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
    /// User-visible info message
    Info { message: String },
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

    let mut steps: Vec<InstallStep> = Vec::new();

    // Step 1: Python check
    steps.push(InstallStep {
        name: "Python runtime check".into(),
        description: "Detect Python 3 on PATH for RAG service installation".into(),
        action: InstallAction::CheckProgram {
            name: "python".into(),
            optional: false,
        },
        verification_hint: "Run `python --version`".into(),
    });

    // Step 2: Create Python venv
    steps.push(InstallStep {
        name: "Create RAG virtual environment".into(),
        description: format!("Create venv at `{}`", rag_venv_dir.display()),
        action: InstallAction::RunCommand {
            program: if cfg!(windows) { "python" } else { "python3" }.into(),
            args: vec!["-m".into(), "venv".into(), rag_venv_dir.to_string_lossy().to_string()],
            cwd: Some(install_root.clone()),
        },
        verification_hint: format!("Check `{}` exists", rag_venv_dir.join(if cfg!(windows) { "Scripts\\python.exe" } else { "bin/python" }).display()),
    });

    // Step 3: Install RAG deps
    let python_pip = rag_venv_dir.join(if cfg!(windows) { "Scripts\\pip.exe" } else { "bin/pip" });
    let requirements_path = workspace.rag_dir.join("requirements.txt");

    steps.push(InstallStep {
        name: "Install Python RAG dependencies".into(),
        description: format!("pip install -r `{}`", requirements_path.display()),
        action: InstallAction::RunCommand {
            program: python_pip.to_string_lossy().to_string(),
            args: vec![
                "install".into(),
                "-r".into(),
                requirements_path.to_string_lossy().to_string(),
            ],
            cwd: Some(workspace.rag_dir.clone()),
        },
        verification_hint: "Verify pip list shows chromadb, fastapi, uvicorn".into(),
    });

    // Step 4: Node.js check
    steps.push(InstallStep {
        name: "Node.js / npm check".into(),
        description: "Detect Node.js for optional frontend development".into(),
        action: InstallAction::CheckProgram {
            name: "node".into(),
            optional: true,
        },
        verification_hint: "Run `node --version`".into(),
    });

    // Step 5: Ollama check
    steps.push(InstallStep {
        name: "Ollama check".into(),
        description: "Detect Ollama local LLM server on PATH".into(),
        action: InstallAction::CheckProgram {
            name: "ollama".into(),
            optional: false,
        },
        verification_hint: "Run `ollama --help`".into(),
    });

    // Step 6: Rust toolchain check
    steps.push(InstallStep {
        name: "Rust toolchain check".into(),
        description: "Detect Rust compiler (optional — only needed if building from source)".into(),
        action: InstallAction::CheckProgram {
            name: "cargo".into(),
            optional: true,
        },
        verification_hint: "Run `cargo --version`".into(),
    });

    // Step 7: Create .aegis directory structure
    steps.push(InstallStep {
        name: "Create AEGIS config directory".into(),
        description: format!("Create `{}` with config/, logs/, sessions/", aegis_dir.display()),
        action: InstallAction::CreateDir { path: config_dir.clone() },
        verification_hint: format!("Check `{}` exists", aegis_dir.display()),
    });

    steps.push(InstallStep {
        name: "Create AEGIS logs directory".into(),
        description: format!("Create `{}`", logs_dir.display()),
        action: InstallAction::CreateDir { path: logs_dir },
        verification_hint: format!("Check logs dir exists"),
    });

    steps.push(InstallStep {
        name: "Create AEGIS sessions directory".into(),
        description: format!("Create `{}`", sessions_dir.display()),
        action: InstallAction::CreateDir { path: sessions_dir },
        verification_hint: format!("Check sessions dir exists"),
    });

    // Step 8: Write default aegis.toml
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
            path: config_toml_path,
            content: config_toml_content,
        },
        verification_hint: format!("Check `{}` exists and is valid TOML", config_toml_path.display()),
    });

    // Step 9: Pull default model from Ollama
    steps.push(InstallStep {
        name: format!("Pull default model `{default_model}` from Ollama"),
        description: format!("Run `ollama pull {default_model}` to download the default local model"),
        action: InstallAction::RunCommand {
            program: "ollama".into(),
            args: vec!["pull".into(), default_model.clone()],
            cwd: None,
        },
        verification_hint: format!("Run `ollama list` and check `{default_model}` appears"),
    });

    InstallPlan {
        summary: format!(
            "Complete installation plan for AEGIS at `{}`. Performs dependency checks, creates RAG venv, writes config, and pulls default model.",
            install_root.display()
        ),
        workspace_root: workspace.root.clone(),
        default_install_root: workspace.default_install_root.clone(),
        install_root,
        install_root_source: install_root_source.into(),
        steps,
    }
}

/// Execute all steps in the install plan.
/// Returns Ok(()) if all steps succeed.
pub fn execute_install_plan(ui: &Ui, plan: &InstallPlan) -> Result<(), Vec<String>> {
    let mut errors: Vec<String> = Vec::new();

    for (index, step) in plan.steps.iter().enumerate() {
        let step_num = index + 1;
        let total = plan.steps.len();
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
                        // Continue with other steps for a full report
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
            InstallAction::Info { message } => {
                println!("  {}", ui.muted(message));
            }
        }

        println!();
    }

    if errors.is_empty() {
        println!("{}", ui.success("Installation complete!"));
        println!("{}", ui.muted(&format!(
            "Run `aegis open` to start the system, or `aegis doctor` to verify readiness."
        )));
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
        println!("{}. {}", index + 1, step.name);
        println!("   {}", step.description);
        println!("   Verify: {}", step.verification_hint);
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
```

### 4.2 Update `cli/src/commands.rs` — `handle_install`

Replace the current `handle_install` function (lines 132–179):

```rust
fn handle_install(ctx: &AppContext, args: crate::args::InstallArgs) -> AppResult<()> {
    let (install_root, install_root_source) = if let Some(path) = args.path.as_deref() {
        (
            crate::workspace::Workspace::normalize_install_root(path),
            "--path".to_string(),
        )
    } else if std::env::var_os("AEGIS_INSTALL_ROOT").is_some() {
        (
            ctx.workspace.install_root.clone(),
            "AEGIS_INSTALL_ROOT".to_string(),
        )
    } else if ctx.workspace.install_root != ctx.workspace.default_install_root {
        (
            ctx.workspace.install_root.clone(),
            "saved preference".to_string(),
        )
    } else {
        (ctx.workspace.install_root.clone(), "default".to_string())
    };

    let plan = install::build_install_plan(
        &ctx.workspace,
        install_root.clone(),
        install_root_source.clone(),
    );

    if args.plan_only {
        install::print_install_plan(&ctx.ui, &plan);
        return Ok(());
    }

    // Save the install root preference (even before execution, so future CLI runs know where things are)
    install::persist_install_root(&ctx.ui, &install_root)?;
    println!();

    if !args.yes {
        println!("{}", ctx.ui.warning("Dry-run mode: showing plan. Pass `--yes` to execute."));
        install::print_install_plan(&ctx.ui, &plan);
        return Ok(());
    }

    // Execute the install plan
    match install::execute_install_plan(&ctx.ui, &plan) {
        Ok(()) => Ok(()),
        Err(errors) => {
            // Print summary of what went wrong
            for e in &errors {
                eprintln!("{}", ctx.ui.error(e));
            }
            Err(format!(
                "Installation completed with {} error(s). Fix the issues above and re-run `aegis install --yes`.",
                errors.len()
            ))
        }
    }
}
```

### 4.3 Add `runner.rs` dependency to `install.rs`

The new `install.rs` uses `LaunchPlan` and `run_foreground` from `runner.rs`. Make sure `runner.rs` exports `run_foreground` (it already does — see line 282). The `InstallAction::RunCommand` usage is fine.

### 4.4 Verification

```bash
# Test plan-only mode (existing behavior plus new plan content)
aegis install --plan-only

# Test real execution
aegis install --yes

# Expected output:
#   1. Python runtime check ... OK
#   2. Create RAG virtual environment ... OK
#   3. Install Python RAG dependencies ... (takes a while, installs chromadb etc.)
#   4. Node.js / npm check ... OK (or WARN if not found)
#   5. Ollama check ... OK
#   6. Rust toolchain check ... OK (or WARN)
#   7-9. Create .aegis directories ... OK
#   10. Write default aegis.toml ... OK
#   11. Pull default model qwen3:4b from Ollama ... OK
```

---

## 5. NSIS Installer Script

### 5.1 Prerequisites

Install NSIS from: https://nsis.sourceforge.io/Download (or via `winget install NSIS.NSIS`).

### 5.2 NSIS script file

**File:** `installer/aegis-installer.nsi` (new)

```nsis
; AEGIS Windows Installer — NSIS script
; Bundles pre-compiled engine.exe, cli.exe, and frontend dist/

!define PRODUCT_NAME "AEGIS"
!define PRODUCT_VERSION "0.1.0"
!define PRODUCT_PUBLISHER "Nous Research"
!define PRODUCT_WEB_SITE "https://github.com/NousResearch/AEGIS"

Unicode true
RequestExecutionLevel admin

; --- Interface Settings ---
!include "MUI2.nsh"
!include "FileFunc.nsh"
!include "WinCore.nsh"

Name "${PRODUCT_NAME} ${PRODUCT_VERSION}"
OutFile "AEGIS-Windows-x64.exe"
InstallDir "$LOCALAPPDATA\AEGIS"

; Check for existing install; read previous InstallDir from registry
InstallDirRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS" "InstallLocation"

; --- Pages ---
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY

; Components page
!insertmacro MUI_PAGE_COMPONENTS

; Instfiles page
!insertmacro MUI_PAGE_INSTFILES

; Finish page with run checkbox
!define MUI_FINISHPAGE_RUN "$INSTDIR\bin\aegis.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Launch AEGIS after setup"
!define MUI_FINISHPAGE_LINK "View documentation"
!define MUI_FINISHPAGE_LINK_LOCATION "https://github.com/NousResearch/AEGIS"
!define MUI_FINISHPAGE_NOREBOOT_SUPPORT
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

; --- Languages ---
!insertmacro MUI_LANGUAGE "English"

; --- Install Sections ---

Section "AEGIS Core (required)" SecCore
    SectionIn RO
    SetOutPath "$INSTDIR"

    ; Create directory structure
    CreateDirectory "$INSTDIR\bin"
    CreateDirectory "$INSTDIR\frontend\dist"
    CreateDirectory "$INSTDIR\.aegis\config"
    CreateDirectory "$INSTDIR\.aegis\logs"
    CreateDirectory "$INSTDIR\.aegis\sessions"

    ; Copy binaries
    File /oname=bin\aegis-engine.exe "..\build\release\aegis-engine.exe"
    File /oname=bin\aegis.exe "..\build\release\aegis.exe"

    ; Copy frontend dist
    File /r /x *.map /x *.ts /x node_modules "..\frontend\dist\*.*" "frontend\dist\"

    ; Write a default aegis.toml (minimal, since `aegis install --yes` will overwrite it)
    FileOpen $0 "$INSTDIR\.aegis\config\aegis.toml" w
    FileWrite $0 "# AEGIS configuration$\r$\n"
    FileWrite $0 "# Generated by installer$\r$\n"
    FileWrite $0 "$\r$\n"
    FileWrite $0 "[server]$\r$\n"
    FileWrite $0 'host = "127.0.0.1"$\r$\n'
    FileWrite $0 'port = "8080"$\r$\n'
    FileWrite $0 "$\r$\n"
    FileWrite $0 "[inference]$\r$\n"
    FileWrite $0 'provider = "ollama"$\r$\n'
    FileWrite $0 'base_url = "http://127.0.0.1:11434"$\r$\n'
    FileWrite $0 "$\r$\n"
    FileWrite $0 "[rag]$\r$\n"
    FileWrite $0 'base_url = "http://127.0.0.1:8000"$\r$\n'
    FileWrite $0 'venv_path = "$INSTDIR\rag-env"$\r$\n'
    FileClose $0

    ; Write version info file
    FileOpen $0 "$INSTDIR\AEGIS_VERSION" w
    FileWrite $0 "${PRODUCT_VERSION}"
    FileClose $0

    ; Write uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"

    ; Create Start Menu shortcuts
    CreateDirectory "$SMPROGRAMS\AEGIS"
    CreateShortCut "$SMPROGRAMS\AEGIS\AEGIS Shell.lnk" "$INSTDIR\bin\aegis.exe" "" "$INSTDIR\bin\aegis.exe" 0
    CreateShortCut "$SMPROGRAMS\AEGIS\AEGIS Engine.lnk" "$INSTDIR\bin\aegis-engine.exe" "" "$INSTDIR\bin\aegis-engine.exe" 0
    CreateShortCut "$SMPROGRAMS\AEGIS\Uninstall AEGIS.lnk" "$INSTDIR\uninstall.exe" "" "$INSTDIR\uninstall.exe" 0
    CreateShortCut "$SMPROGRAMS\AEGIS\Documentation.lnk" "https://github.com/NousResearch/AEGIS"

    ; Registry for uninstall
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS" \
        "DisplayName" "AEGIS - Your Local AI Assistant"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS" \
        "DisplayVersion" "${PRODUCT_VERSION}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS" \
        "Publisher" "${PRODUCT_PUBLISHER}"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS" \
        "InstallLocation" "$INSTDIR"
    WriteRegStr HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS" \
        "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS" \
        "NoModify" 1
    WriteRegDWORD HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS" \
        "NoRepair" 1
SectionEnd

Section "Add AEGIS to PATH" SecPath
    ; Add bin directory to user PATH
    EnVar::SetHKCU
    EnVar::AddValue "PATH" "$INSTDIR\bin"
    Pop $0
    DetailPrint "PATH update result: $0"
SectionEnd

Section "Desktop shortcut" SecDesktop
    CreateShortCut "$DESKTOP\AEGIS.lnk" "$INSTDIR\bin\aegis.exe" "" "$INSTDIR\bin\aegis.exe" 0
SectionEnd

; --- Descriptions ---
LangString DESC_SecCore ${LANG_ENGLISH} "Core AEGIS files: engine, CLI, and web UI."
LangString DESC_SecPath ${LANG_ENGLISH} "Add the AEGIS bin folder to the user PATH so you can run `aegis` from any terminal."
LangString DESC_SecDesktop ${LANG_ENGLISH} "Create a desktop shortcut for the AEGIS CLI."

!insertmacro MUI_FUNCTION_DESCRIPTION_BEGIN
    !insertmacro MUI_DESCRIPTION_TEXT ${SecCore} $(DESC_SecCore)
    !insertmacro MUI_DESCRIPTION_TEXT ${SecPath} $(DESC_SecPath)
    !insertmacro MUI_DESCRIPTION_TEXT ${SecDesktop} $(DESC_SecDesktop)
!insertmacro MUI_FUNCTION_DESCRIPTION_END

; --- Uninstaller ---
Section "Uninstall"
    ; Remove shortcuts
    RMDir /r "$SMPROGRAMS\AEGIS"
    Delete "$DESKTOP\AEGIS.lnk"

    ; Remove PATH entry
    EnVar::SetHKCU
    EnVar::DeleteValue "PATH" "$INSTDIR\bin"
    Pop $0

    ; Remove install directory (preserve .aegis config if user wants? For clean uninstall, remove all)
    ; In a production installer, offer a checkbox. For now, clean removal.
    RMDir /r "$INSTDIR\bin"
    RMDir /r "$INSTDIR\frontend"
    Delete "$INSTDIR\uninstall.exe"
    Delete "$INSTDIR\AEGIS_VERSION"
    RMDir "$INSTDIR"

    ; Remove registry key
    DeleteRegKey HKLM "Software\Microsoft\Windows\CurrentVersion\Uninstall\AEGIS"
SectionEnd
```

### 5.3 Build script for the NSIS installer

**File:** `installer/build-installer.bat` (new)

```bat
@echo off
REM installer\build-installer.bat — Build the AEGIS Windows NSIS installer
REM Prerequisites: NSIS installed, `makensis` on PATH

setlocal enabledelayedexpansion

echo ===== AEGIS Windows Installer Build =====
echo.

REM 1. Build engine release binary
echo [1/5] Building engine release binary...
call "%~dp0..\scripts\build-engine-release.bat" || exit /b 1
echo.

REM 2. Build CLI release binary
echo [2/5] Building CLI release binary...
call "%~dp0..\scripts\build-cli-release.bat" || exit /b 1
echo.

REM 3. Ensure frontend dist is built
echo [3/5] Building frontend...
cd /d "%~dp0..\frontend"
call npm install && call npm run build || exit /b 1
echo.

REM 4. Run NSIS
echo [4/5] Compiling NSIS installer...
cd /d "%~dp0"
makensis aegis-installer.nsi || exit /b 1
echo.

REM 5. Show result
set INSTALLER_EXE=%~dp0AEGIS-Windows-x64.exe
echo [5/5] Installer created:
if exist "%INSTALLER_EXE%" (
    echo   %INSTALLER_EXE%
    for %%I in ("%INSTALLER_EXE%") do echo   Size: %%~zI bytes
    echo.
    echo ===== Installer build complete =====
) else (
    echo   ERROR: Installer not found at expected path!
    exit /b 1
)
```

### 5.4 Verification

```bash
installer\build-installer.bat
# Expected output:
#   AEGIS Windows Installer Build
#   [1/5] Building engine release binary... OK
#   [2/5] Building CLI release binary... OK
#   [3/5] Building frontend... OK
#   [4/5] Compiling NSIS installer... OK
#   [5/5] Installer created: ...\AEGIS-Windows-x64.exe (XX MB)

# Manual test: run the .exe and go through the installer wizard
```

---

## 6. First-Run `aegis open` Command

### 6.1 Add `Open` variant to CLI command tree

**File:** `cli/src/cli.rs`

Add the `Open` variant to the `CommandKind` enum:

```rust
#[derive(Debug, Clone, Subcommand)]
pub enum CommandKind {
    Install(InstallArgs),
    Open,                 // <-- ADD THIS
    Save(SaveArgs),
    // ... rest unchanged
}
```

### 6.2 Add `handle_open` in `cli/src/commands.rs`

Add after the `handle_install` function:

```rust
fn handle_open(ctx: &AppContext) -> AppResult<()> {
    println!("{}", ctx.ui.header("AEGIS Open"));
    println!(
        "{}",
        ctx.ui.muted("Starting local AEGIS services and opening the web interface...")
    );
    println!();

    let workspace = &ctx.workspace;
    let install_root = &workspace.install_root;
    let engine_binary = install_root.join("bin").join("aegis-engine.exe");
    let rag_venv_python = install_root
        .join("rag-env")
        .join("Scripts")
        .join("python.exe");
    let aegis_config = install_root.join(".aegis").join("config").join("aegis.toml");

    let engine_url =
        std::env::var("AEGIS_ENGINE_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let rag_url =
        std::env::var("AEGIS_RAG_URL").unwrap_or_else(|_| "http://127.0.0.1:8000".to_string());

    // 1. Start RAG service if not running
    println!("{} Checking RAG service...", ctx.ui.info("1/3"));
    if !crate::runner::service_reachable(&join_url(&rag_url, "/health")) {
        if rag_venv_python.exists() {
            let rag_dir = &workspace.rag_dir;
            let log_file = install_root.join(".aegis").join("logs").join("rag.log");

            println!("  Starting RAG from venv: {}", rag_venv_python.display());

            // Use spawn_background equivalent
            let plan = crate::runner::LaunchPlan {
                label: "RAG".to_string(),
                program: rag_venv_python.to_string_lossy().to_string(),
                args: vec![
                    "-m".to_string(),
                    "uvicorn".to_string(),
                    "app.main:app".to_string(),
                    "--host".to_string(),
                    "127.0.0.1".to_string(),
                    "--port".to_string(),
                    "8000".to_string(),
                ],
                cwd: rag_dir.clone(),
                env: vec![("PYTHONUNBUFFERED".to_string(), "1".to_string())],
            };

            crate::runner::spawn_background(&plan, &install_root.join(".aegis").join("logs"))?;

            if crate::runner::wait_for_service(&join_url(&rag_url, "/health"), std::time::Duration::from_secs(15)) {
                println!("  {} RAG is ready", ctx.ui.success("✓"));
            } else {
                println!("  {} RAG started but not yet healthy (check logs)", ctx.ui.warning("⚠"));
            }
        } else {
            println!(
                "  {} RAG venv not found at `{}`. Run `aegis install --yes` first.",
                ctx.ui.warning("⚠"),
                rag_venv_python.display()
            );
        }
    } else {
        println!("  {} RAG is already running", ctx.ui.success("✓"));
    }

    // 2. Start engine if not running
    println!("{} Checking engine...", ctx.ui.info("2/3"));
    if !crate::runner::service_reachable(&join_url(&engine_url, "/health")) {
        if engine_binary.exists() {
            println!("  Starting engine: {}", engine_binary.display());

            let plan = crate::runner::LaunchPlan {
                label: "Engine".to_string(),
                program: engine_binary.to_string_lossy().to_string(),
                args: Vec::new(),
                cwd: install_root.clone(),
                env: vec![
                    ("AEGIS_RAG_URL".to_string(), rag_url.clone()),
                    ("AEGIS_ENGINE_HOST".to_string(), "127.0.0.1".to_string()),
                    ("AEGIS_ENGINE_PORT".to_string(), "8080".to_string()),
                    ("AEGIS_CONFIG_PATH".to_string(), aegis_config.to_string_lossy().to_string()),
                ],
            };

            crate::runner::spawn_background(&plan, &install_root.join(".aegis").join("logs"))?;

            if crate::runner::wait_for_service(&join_url(&engine_url, "/health"), std::time::Duration::from_secs(30)) {
                println!("  {} Engine is ready", ctx.ui.success("✓"));
            } else {
                println!("  {} Engine started but not yet healthy (check logs)", ctx.ui.warning("⚠"));
            }
        } else {
            println!(
                "  {} Engine binary not found at `{}`. Re-run the installer or build from source.",
                ctx.ui.warning("⚠"),
                engine_binary.display()
            );
        }
    } else {
        println!("  {} Engine is already running", ctx.ui.success("✓"));
    }

    // 3. Open browser
    println!("{} Opening web UI...", ctx.ui.info("3/3"));
    let web_url = "http://localhost:8080";
    println!("  Web UI: {web_url}");

    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", web_url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(web_url).spawn();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(web_url).spawn();
    }

    println!();
    println!("{}", ctx.ui.success("AEGIS is running!"));
    println!("  Web UI    : {web_url}");
    println!("  API       : {engine_url}");
    println!("  RAG       : {rag_url}");
    println!();
    println!("{}", ctx.ui.muted("Press Ctrl+C to stop all services (if running from a terminal)."));

    Ok(())
}

/// Helper — join base URL and path
fn join_url(base_url: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}
```

### 6.3 Wire dispatch in `commands.rs`

In the `dispatch_command` function (line 62), add the `Open` case:

```rust
fn dispatch_command(
    ctx: &AppContext,
    command: CommandKind,
    invocation_mode: InvocationMode,
) -> AppResult<()> {
    match command {
        CommandKind::Install(args) => handle_install(ctx, args),
        CommandKind::Open => handle_open(ctx),          // <-- ADD THIS
        CommandKind::Save(args) => handle_save(ctx, &args.note),
        // ... rest unchanged
    }
}
```

### 6.4 Export `service_reachable`, `wait_for_service`, `spawn_background` from `runner.rs`

These are currently `pub(crate)` or not exported. Make them `pub`:

In `runner.rs`:
- Line 506: `fn service_reachable` → `pub fn service_reachable`
- Line 515: `fn wait_for_service` → `pub fn wait_for_service`
- Line 400: `fn spawn_background` → `pub fn spawn_background`

### 6.5 Verification

```bash
# Start everything with one command
aegis open

# Expected output:
#   1/3 Checking RAG service...
#     ✓ RAG is already running
#   2/3 Checking engine...
#     ✓ Engine is already running
#   3/3 Opening web UI...
#     Web UI: http://localhost:8080
# (Browser opens to localhost:8080)

# If nothing is running:
#   1/3 Checking RAG service...
#     Starting RAG from venv: ...
#     ✓ RAG is ready
#   2/3 Checking engine...
#     Starting engine: ...
#     ✓ Engine is ready
#   3/3 Opening web UI...
#     Web UI: http://localhost:8080
```

---

## 7. Runner.rs Changes (Binary Startup)

### 7.1 Update `engine_launch_plan` to use pre-compiled binary

**File:** `cli/src/runner.rs`

Replace the current `engine_launch_plan` function (lines 248–270):

```rust
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
                    std::env::var("AEGIS_ENGINE_HOST")
                        .unwrap_or_else(|_| "127.0.0.1".to_string()),
                ),
                (
                    "AEGIS_ENGINE_PORT".to_string(),
                    std::env::var("AEGIS_ENGINE_PORT")
                        .unwrap_or_else(|_| "8080".to_string()),
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
```

### 7.2 Update `ensure_frontend_runtime` — no longer needed

Since the frontend is now embedded in the engine binary, the `ensure_frontend_runtime` function (lines 192–246) should be **removed** or **disabled**. The web UI is served on the same port as the engine (8080). 

**Change:** Remove the call to `ensure_frontend_runtime` from `ensure_local_runtime` (line 83 becomes just RAG + Engine).

From:
```rust
ensure_rag_runtime(workspace, &logs_dir, &mut report);
ensure_engine_runtime(workspace, &logs_dir, &mut report);
ensure_frontend_runtime(workspace, &logs_dir, &mut report);
```

To:
```rust
ensure_rag_runtime(workspace, &logs_dir, &mut report);
ensure_engine_runtime(workspace, &logs_dir, &mut report);
```

Optionally, keep `ensure_frontend_runtime` as a no-op with a deprecation comment.

### 7.3 Update `web_ui_url` in `workspace.rs`

**File:** `cli/src/workspace.rs`

Update the `web_ui_url` method (lines 183–193) to default to the engine URL (port 8080) instead of the Vite dev server (port 5173):

```rust
pub fn web_ui_url(&self) -> String {
    if let Ok(url) = env::var("AEGIS_WEB_URL") {
        let url = url.trim();
        if !url.is_empty() {
            return url.to_string();
        }
    }

    // Default to engine URL — frontend is embedded, served on same port
    let host = env::var("AEGIS_ENGINE_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("AEGIS_ENGINE_PORT").unwrap_or_else(|_| "8080".to_string());
    format!("http://{host}:{port}")
}
```

This makes `aegis status`, `aegis doctor`, and all other places that call `web_ui_url()` point to the embedded frontend on the engine port.

### 7.4 Verification

```bash
# Verify engine_launch_plan picks up the compiled binary first:
# 1. Place a copy of engine.exe in install_root/bin/
# 2. Run aegis status (or aegis open)
# It should launch the binary, not cargo run

# Verify no frontend npm run dev is started:
# Check that the process list does not include npm
```

---

## 8. GitHub Actions CI

### 8.1 Workflow file

**File:** `.github/workflows/release.yml` (new)

```yaml
name: Build and Release

on:
  push:
    tags:
      - 'v*'  # Trigger on version tags like v0.1.0, v1.0.0
  workflow_dispatch:  # Allow manual trigger

jobs:
  build-windows:
    runs-on: windows-latest
    defaults:
      run:
        shell: bash

    steps:
      - name: Checkout
        uses: actions/checkout@v4

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
          cache: 'npm'
          cache-dependency-path: frontend/package-lock.json

      - name: Install frontend dependencies
        run: |
          cd frontend
          npm ci

      - name: Build frontend
        run: |
          cd frontend
          npm run build

      - name: Build engine (release)
        run: |
          cd engine
          cargo build --release
        env:
          CARGO_TARGET_DIR: ${{ github.workspace }}/.cargo-target-engine

      - name: Build CLI (release)
        run: |
          cd cli
          cargo build --release

      - name: Prepare staging directory
        run: |
          mkdir -p build/release
          cp engine/target/release/aegis-engine.exe build/release/
          cp cli/target/release/aegis.exe build/release/
          cp -r frontend/dist build/release/frontend-dist
          ls -la build/release/

      - name: Setup NSIS
        run: |
          # NSIS is pre-installed on windows-latest
          # Verify it's available
          makensis /VERSION

      - name: Build NSIS installer
        run: |
          cd installer
          makensis aegis-installer.nsi

      - name: Upload installer artifact
        uses: actions/upload-artifact@v4
        with:
          name: AEGIS-Windows-x64
          path: installer/AEGIS-Windows-x64.exe
          if-no-files-found: error

      - name: Upload binaries artifact
        uses: actions/upload-artifact@v4
        with:
          name: AEGIS-binaries
          path: |
            build/release/aegis-engine.exe
            build/release/aegis.exe
            build/release/frontend-dist/
          if-no-files-found: error

  create-release:
    needs: build-windows
    runs-on: ubuntu-latest
    permissions:
      contents: write
    if: startsWith(github.ref, 'refs/tags/v')

    steps:
      - name: Download installer
        uses: actions/download-artifact@v4
        with:
          name: AEGIS-Windows-x64
          path: artifacts/

      - name: Download binaries
        uses: actions/download-artifact@v4
        with:
          name: AEGIS-binaries
          path: artifacts/binaries/

      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          name: AEGIS ${{ github.ref_name }}
          body: |
            ## AEGIS ${{ github.ref_name }}

            ### Windows Installer
            - `AEGIS-Windows-x64.exe` — NSIS installer bundle (recommended)

            ### Standalone Binaries
            - `aegis-engine.exe` — AEGIS Rust engine
            - `aegis.exe` — AEGIS CLI
            - `frontend-dist/` — Pre-built frontend (embedded in engine binary, included here for reference)

            ### Installation
            1. Download `AEGIS-Windows-x64.exe`
            2. Run the installer
            3. Open a terminal and run `aegis install --yes` to configure dependencies
            4. Run `aegis open` to start
          files: |
            artifacts/AEGIS-Windows-x64.exe
          generate_release_notes: true
```

### 8.2 Verification

The workflow runs automatically when a tag matching `v*` is pushed:

```bash
git tag v0.1.0
git push origin v0.1.0
# → GitHub Actions triggers, builds everything, publishes release
```

Or triggered manually from the GitHub Actions UI.

---

## 9. Verification Commands Summary

After implementing all 8 steps, run these verification checks:

```bash
# 1. Frontend embedding
cd engine && cargo build --release
# → Should build frontend then compile engine

# 2. Engine binary standalone test
./target/release/aegis-engine.exe &
curl http://localhost:8080 | head -5
# → Should return HTML (not "Not found")
curl http://localhost:8080/api/health
# → Should return JSON health status
kill %1

# 3. CLI binary
cd ../cli && cargo build --release
./target/release/aegis.exe --help
# → Should show full command list

# 4. Install command (from repo root)
./target/release/aegis.exe install --plan-only
./target/release/aegis.exe install --yes
# → Should create venv, install deps, create directories, pull model

# 5. Open command
./target/release/aegis.exe open
# → Should start RAG + engine, open browser

# 6. Runner binary mode
# Move engine binary to install root:
cp engine/target/release/aegis-engine.exe %LOCALAPPDATA%/AEGIS/bin/
./target/release/aegis.exe open
# → Should launch engine binary (not cargo run)

# 7. NSIS installer
cd installer && makensis aegis-installer.nsi
# → Should produce AEGIS-Windows-x64.exe
```

---

## Appendix A: Files to Create

| # | File | Purpose |
|---|------|---------|
| 1 | `engine/build.rs` | Frontend build trigger |
| 2 | `scripts/build-engine-release.bat` | Engine binary build script (Windows) |
| 3 | `scripts/build-engine-release.sh` | Engine binary build script (Unix/CI) |
| 4 | `scripts/build-cli-release.bat` | CLI binary build script (Windows) |
| 5 | `installer/aegis-installer.nsi` | NSIS installer script |
| 6 | `installer/build-installer.bat` | Installer build orchestrator |
| 7 | `.github/workflows/release.yml` | CI release workflow |

## Appendix B: Files to Modify

| # | File | Change |
|---|------|--------|
| 1 | `engine/Cargo.toml` | Add `rust-embed`, `mime_guess` deps |
| 2 | `engine/src/network/router.rs` | Replace `handle_static` with embedded version; relax CORS; remove `mime_type` |
| 3 | `cli/src/install.rs` | Complete rewrite with real steps + `execute_install_plan` |
| 4 | `cli/src/commands.rs` | Update `handle_install`; add `handle_open` |
| 5 | `cli/src/cli.rs` | Add `Open` variant to `CommandKind` |
| 6 | `cli/src/runner.rs` | Update `engine_launch_plan` for binary; make `spawn_background`/`service_reachable`/`wait_for_service` pub; remove `ensure_frontend_runtime` call |
| 7 | `cli/src/workspace.rs` | Update `web_ui_url` to default to engine port 8080 |
| 8 | `.gitignore` | No change needed (already has `!landing page/public/downloads/AEGIS-Windows-x64.exe`) |
| 9 | `landing page/index.html` | Update download button to point to real installer (post-build step) |

## Appendix C: Files Requiring No Change

| File | Reason |
|------|--------|
| `frontend/src/constants/api.ts` | `API_BASE` is already `'/api'` — correct for same-origin |
| `cli/src/args.rs` | `InstallArgs` already has `--path`, `--plan-only`, `--yes` |
| `cli/src/main.rs` | Dispatches to commands.rs; no change needed |
| `engine/src/config.rs` | Stays env-var driven (config fallback added in install step) |
| `engine/src/main.rs` | No change — just calls `create_router` which handles everything |
| `pyproject.toml` | Empty — used by future RAG packaging |
| `requirements.txt` | Referenced by `install.rs` for pip install |
