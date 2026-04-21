//! Role: Windows bootstrap installer, runtime promotion flow, and installed-process lifecycle helpers.
//! Called by: `commands.rs` for `aegis install`, `aegis start`, `aegis stop`, and `aegis open`.
//! Calls into: `runtime.rs` for layout/state, `runner.rs` for subprocess helpers, and `ui.rs` for rendering.
//! Owns: manifest download, runtime staging/promoting, Ollama/model setup, and managed engine startup behavior.
//! Does not own: command parsing, terminal menus, or engine HTTP behavior itself.
//! Next TODOs: add runtime updates/rollback commands and integrate the future Python RAG sidecar into the same install root.

use std::env;
use std::fs;
use std::io::copy;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::args::InstallArgs;
use crate::runtime::{
    now_epoch_seconds, EnginePidRecord, InstallState, InstallerManifest, RuntimeLayout,
    DEFAULT_ENGINE_HOST, DEFAULT_ENGINE_PORT, DEFAULT_MODEL, DEFAULT_UI_URL,
};
use crate::runner;
use crate::ui::Ui;
use crate::{AppContext, AppResult};

const OLLAMA_HEALTH_URL: &str = "http://127.0.0.1:11434/api/tags";

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

#[derive(Debug, Clone)]
pub struct RuntimeStatus {
    pub installed: bool,
    pub install_root: PathBuf,
    pub version: Option<String>,
    pub engine_url: String,
    pub ui_url: String,
    pub engine_pid: Option<u32>,
    pub engine_running: bool,
    pub ollama_reachable: bool,
    pub model_name: String,
    pub model_present: bool,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StartOutcome {
    Started,
    AlreadyRunning,
}

#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

pub fn build_install_plan(_workspace: &crate::workspace::Workspace) -> InstallPlan {
    InstallPlan {
        summary: "Windows-first bootstrap install for the packaged AEGIS runtime.".to_string(),
        steps: vec![
            InstallStep {
                name: "Validate Windows x64 host".to_string(),
                platform: "Windows x64".to_string(),
                description:
                    "Checks the host platform before attempting a per-user runtime install."
                        .to_string(),
                verification_hint: "The installer should report Windows x64 support.".to_string(),
            },
            InstallStep {
                name: "Download installer manifest".to_string(),
                platform: "Windows x64".to_string(),
                description:
                    "Fetches installer-manifest.json from the configured release host."
                        .to_string(),
                verification_hint: "Manifest version and runtime URL should parse successfully."
                    .to_string(),
            },
            InstallStep {
                name: "Stage runtime bundle".to_string(),
                platform: "Windows x64".to_string(),
                description:
                    "Downloads the runtime zip, verifies SHA-256, extracts it to a staged directory, and promotes it only after validation."
                        .to_string(),
                verification_hint:
                    "The staged runtime must contain bin/aegis.exe, bin/aegis-engine.exe, ui/index.html, config/default.env, and version.txt."
                        .to_string(),
            },
            InstallStep {
                name: "Configure user environment".to_string(),
                platform: "Windows x64".to_string(),
                description:
                    "Creates %LOCALAPPDATA%\\AEGIS directories and adds the per-user bin directory to PATH."
                        .to_string(),
                verification_hint:
                    "The user PATH should contain %LOCALAPPDATA%\\AEGIS\\bin.".to_string(),
            },
            InstallStep {
                name: "Provision Ollama + model".to_string(),
                platform: "Windows x64".to_string(),
                description:
                    "Installs Ollama if missing, starts the local Ollama service when needed, and ensures qwen3:4b is present."
                        .to_string(),
                verification_hint: "Ollama /api/tags should respond and include qwen3:4b."
                    .to_string(),
            },
            InstallStep {
                name: "Write engine config and start localhost runtime".to_string(),
                platform: "Windows x64".to_string(),
                description:
                    "Writes engine.env, launches aegis-engine.exe in the background, waits for /health, and opens the browser."
                        .to_string(),
                verification_hint:
                    "http://localhost:8080/health should return success and the UI should open."
                        .to_string(),
            },
        ],
    }
}

pub fn print_install_plan(ui: &Ui, plan: &InstallPlan) {
    println!("{}", ui.header("Install Plan"));
    println!("{}", plan.summary);
    println!();

    for (index, step) in plan.steps.iter().enumerate() {
        println!("{}. {} [{}]", index + 1, step.name, step.platform);
        println!("   {}", step.description);
        println!("   Verify: {}", step.verification_hint);
    }
}

pub fn run_install(ctx: &AppContext, args: &InstallArgs) -> AppResult<RuntimeStatus> {
    if args.plan_only {
        let plan = build_install_plan(&ctx.workspace);
        print_install_plan(&ctx.ui, &plan);
        return Ok(runtime_status(&ctx.runtime));
    }

    install_runtime(ctx)
}

pub fn runtime_status(layout: &RuntimeLayout) -> RuntimeStatus {
    let install_state = layout.load_install_state();
    let version = install_state.as_ref().map(|state| state.version.clone());
    let engine_url = install_state
        .as_ref()
        .map(|state| format!("http://{}:{}", state.engine_host, state.engine_port))
        .unwrap_or_else(|| format!("http://{DEFAULT_ENGINE_HOST}:{DEFAULT_ENGINE_PORT}"));
    let ui_url = install_state
        .as_ref()
        .map(|state| state.ui_url.clone())
        .unwrap_or_else(|| DEFAULT_UI_URL.to_string());
    let model_name = install_state
        .as_ref()
        .map(|state| state.default_model.clone())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string());
    let pid_record = layout.load_pid_record();
    let pid_running = pid_record
        .as_ref()
        .map(|record| runner::is_process_running(record.pid))
        .unwrap_or(false);
    let engine_health = url_reachable(&format!("{engine_url}/health"));

    RuntimeStatus {
        installed: layout.is_installed(),
        install_root: layout.root.clone(),
        version,
        engine_url,
        ui_url,
        engine_pid: pid_record.as_ref().map(|record| record.pid),
        engine_running: engine_health || pid_running,
        ollama_reachable: ollama_reachable(),
        model_present: ollama_has_model(&model_name),
        model_name,
    }
}

pub fn start_engine(layout: &RuntimeLayout) -> AppResult<StartOutcome> {
    if !layout.is_installed() {
        return Err(format!(
            "AEGIS is not installed in `{}` yet. Run `aegis install` first.",
            layout.root.display()
        ));
    }

    let status = runtime_status(layout);
    if status.engine_running {
        return Ok(StartOutcome::AlreadyRunning);
    }

    layout.ensure_base_dirs()?;

    let program = layout.user_engine_exe();
    if !program.exists() {
        return Err(format!(
            "The installed engine binary was not found at `{}`.",
            program.display()
        ));
    }

    let env_pairs = layout.load_engine_env_pairs()?;
    let pid = runner::spawn_detached_process(
        &program,
        &[],
        &layout.current_runtime_dir,
        &env_pairs,
        &layout.engine_stdout_log(),
        &layout.engine_stderr_log(),
    )?;

    let record = EnginePidRecord {
        pid,
        started_at_epoch_seconds: now_epoch_seconds(),
        version: layout
            .load_install_state()
            .map(|state| state.version)
            .unwrap_or_else(|| "unknown".to_string()),
        url: status.engine_url.clone(),
    };
    layout.save_pid_record(&record)?;

    wait_for_url(&format!("{}/health", status.engine_url), Duration::from_secs(20))?;

    Ok(StartOutcome::Started)
}

pub fn stop_engine(layout: &RuntimeLayout) -> AppResult<bool> {
    let Some(record) = layout.load_pid_record() else {
        return Ok(false);
    };

    if !runner::is_process_running(record.pid) {
        layout.clear_pid_record()?;
        return Ok(false);
    }

    runner::stop_process(record.pid)?;
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if !runner::is_process_running(record.pid) {
            break;
        }
        thread::sleep(Duration::from_millis(300));
    }

    layout.clear_pid_record()?;
    Ok(true)
}

pub fn open_ui(layout: &RuntimeLayout) -> AppResult<()> {
    let url = layout
        .load_install_state()
        .map(|state| state.ui_url)
        .unwrap_or_else(|| DEFAULT_UI_URL.to_string());
    runner::open_in_browser(&url)
}

fn install_runtime(ctx: &AppContext) -> AppResult<RuntimeStatus> {
    validate_windows_x64()?;
    ctx.runtime.ensure_base_dirs()?;

    println!("{}", ctx.ui.header("AEGIS Install"));
    println!("Install root : {}", ctx.runtime.root.display());
    println!("Manifest URL : {}", ctx.runtime.manifest_url());
    println!();

    let manifest_source = ctx.runtime.manifest_url();
    let manifest = fetch_manifest(&manifest_source)?;
    ctx.runtime.save_manifest_cache(&manifest)?;
    println!("{}", ctx.ui.success("Manifest downloaded."));

    let current_state = ctx.runtime.load_install_state();
    let runtime_needs_update = current_state
        .as_ref()
        .map(|state| state.version != manifest.version || !ctx.runtime.current_runtime_dir.exists())
        .unwrap_or(true);

    if runtime_needs_update {
        println!("{}", ctx.ui.info("Staging runtime bundle..."));
        stage_and_promote_runtime(&ctx.runtime, &manifest)?;
        println!("{}", ctx.ui.success("Runtime bundle promoted."));
    } else {
        println!(
            "{}",
            ctx.ui.muted("Installed runtime already matches the manifest version.")
        );
        sync_user_facing_binaries(&ctx.runtime)?;
    }

    ensure_user_path_contains(&ctx.runtime.bin_dir)?;
    println!("{}", ctx.ui.success("User PATH verified."));

    let ollama_exe = ensure_ollama_installed_and_running(&ctx.runtime, &manifest)?;
    ensure_model_available(&ollama_exe, &manifest.default_model, &ctx.runtime)?;
    println!(
        "{}",
        ctx.ui.success(&format!("Model `{}` is ready.", manifest.default_model))
    );

    ctx.runtime.write_engine_env(&manifest)?;
    let install_state = InstallState {
        version: manifest.version.clone(),
        installed_at_epoch_seconds: now_epoch_seconds(),
        manifest_url: manifest_source,
        runtime_sha256: manifest.runtime_sha256.clone(),
        default_model: manifest.default_model.clone(),
        engine_host: manifest.engine_host.clone(),
        engine_port: manifest.engine_port,
        ui_url: manifest.ui_url.clone(),
    };
    ctx.runtime.save_install_state(&install_state)?;

    if ctx.runtime.load_pid_record().is_some() {
        let _ = stop_engine(&ctx.runtime);
    }

    match start_engine(&ctx.runtime)? {
        StartOutcome::Started => println!("{}", ctx.ui.success("AEGIS engine started.")),
        StartOutcome::AlreadyRunning => {
            println!("{}", ctx.ui.muted("AEGIS engine was already running."))
        }
    }

    open_ui(&ctx.runtime)?;
    println!("{}", ctx.ui.success(&format!("Opened {}.", manifest.ui_url)));

    Ok(runtime_status(&ctx.runtime))
}

fn validate_windows_x64() -> AppResult<()> {
    if !cfg!(windows) {
        return Err("The v1 bootstrap installer currently supports Windows only.".to_string());
    }

    let arch = env::var("PROCESSOR_ARCHITECTURE").unwrap_or_default().to_lowercase();
    let wow64 = env::var("PROCESSOR_ARCHITEW6432").unwrap_or_default().to_lowercase();
    let is_x64 = arch.contains("amd64") || arch.contains("x86_64") || wow64.contains("amd64");

    if is_x64 {
        Ok(())
    } else {
        Err("The v1 bootstrap installer currently supports Windows x64 only.".to_string())
    }
}

fn fetch_manifest(source: &str) -> AppResult<InstallerManifest> {
    if Path::new(source).exists() {
        let raw = fs::read_to_string(source)
            .map_err(|error| format!("Could not read installer manifest `{source}`: {error}"))?;
        serde_json::from_str(&raw)
            .map_err(|error| format!("Could not parse installer manifest `{source}`: {error}"))
    } else {
        Client::new()
            .get(source)
            .send()
            .map_err(|error| format!("Could not download installer manifest `{source}`: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Manifest request failed for `{source}`: {error}"))?
            .json::<InstallerManifest>()
            .map_err(|error| format!("Could not parse installer manifest `{source}`: {error}"))
    }
}

fn stage_and_promote_runtime(layout: &RuntimeLayout, manifest: &InstallerManifest) -> AppResult<()> {
    let download_dir = env::temp_dir().join(format!("aegis-install-{}", now_epoch_seconds()));
    fs::create_dir_all(&download_dir).map_err(|error| {
        format!(
            "Could not create download directory `{}`: {error}",
            download_dir.display()
        )
    })?;

    let archive_path = download_dir.join("aegis-runtime-windows-x64.zip");
    download_file(&manifest.runtime_url, &archive_path)?;
    verify_sha256(&archive_path, &manifest.runtime_sha256)?;

    let stage_dir = layout.stage_dir_for(&manifest.version);
    if stage_dir.exists() {
        fs::remove_dir_all(&stage_dir).map_err(|error| {
            format!(
                "Could not clean staged runtime `{}`: {error}",
                stage_dir.display()
            )
        })?;
    }
    fs::create_dir_all(&stage_dir).map_err(|error| {
        format!(
            "Could not create staged runtime `{}`: {error}",
            stage_dir.display()
        )
    })?;

    runner::expand_zip_archive(&archive_path, &stage_dir)?;
    validate_runtime_bundle(&stage_dir)?;

    if layout.current_runtime_dir.exists() {
        let backup_version = layout
            .load_install_state()
            .map(|state| state.version)
            .unwrap_or_else(|| format!("backup-{}", now_epoch_seconds()));
        let backup_dir = layout.backup_dir_for(&backup_version);

        if backup_dir.exists() {
            let _ = fs::remove_dir_all(&backup_dir);
        }

        fs::rename(&layout.current_runtime_dir, &backup_dir).map_err(|error| {
            format!(
                "Could not move the current runtime to `{}`: {error}",
                backup_dir.display()
            )
        })?;
    }

    if layout.current_runtime_dir.exists() {
        let _ = fs::remove_dir_all(&layout.current_runtime_dir);
    }

    if let Some(parent) = layout.current_runtime_dir.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create active runtime parent `{}`: {error}",
                parent.display()
            )
        })?;
    }

    fs::rename(&stage_dir, &layout.current_runtime_dir).map_err(|error| {
        format!(
            "Could not promote the staged runtime to `{}`: {error}",
            layout.current_runtime_dir.display()
        )
    })?;

    sync_user_facing_binaries(layout)?;

    let _ = fs::remove_file(&archive_path);
    let _ = fs::remove_dir_all(&download_dir);

    Ok(())
}

fn validate_runtime_bundle(stage_dir: &Path) -> AppResult<()> {
    let required = [
        stage_dir.join("bin").join("aegis.exe"),
        stage_dir.join("bin").join("aegis-engine.exe"),
        stage_dir.join("ui").join("index.html"),
        stage_dir.join("config").join("default.env"),
        stage_dir.join("version.txt"),
    ];

    for path in required {
        if !path.exists() {
            return Err(format!(
                "The runtime bundle is missing `{}` after extraction.",
                path.display()
            ));
        }
    }

    Ok(())
}

fn sync_user_facing_binaries(layout: &RuntimeLayout) -> AppResult<()> {
    fs::create_dir_all(&layout.bin_dir).map_err(|error| {
        format!(
            "Could not create launcher directory `{}`: {error}",
            layout.bin_dir.display()
        )
    })?;

    copy_runtime_binary(
        &layout.installed_cli_exe(),
        &layout.user_cli_exe(),
        "CLI launcher",
    )?;
    copy_runtime_binary(
        &layout.installed_engine_exe(),
        &layout.user_engine_exe(),
        "engine launcher",
    )?;

    Ok(())
}

fn copy_runtime_binary(source: &Path, destination: &Path, label: &str) -> AppResult<()> {
    let running_from_destination = env::current_exe()
        .ok()
        .map(|current| current.eq(destination))
        .unwrap_or(false);

    if running_from_destination {
        return Ok(());
    }

    fs::copy(source, destination).map_err(|error| {
        format!(
            "Could not copy the {label} from `{}` to `{}`: {error}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(())
}

fn download_file(url: &str, destination: &Path) -> AppResult<()> {
    let mut response = Client::new()
        .get(url)
        .send()
        .map_err(|error| format!("Could not download `{url}`: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Download failed for `{url}`: {error}"))?;

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create `{}`: {error}", parent.display()))?;
    }

    let mut file = fs::File::create(destination).map_err(|error| {
        format!(
            "Could not create download target `{}`: {error}",
            destination.display()
        )
    })?;

    copy(&mut response, &mut file).map_err(|error| {
        format!(
            "Could not write downloaded file `{}`: {error}",
            destination.display()
        )
    })?;

    Ok(())
}

fn verify_sha256(path: &Path, expected_hash: &str) -> AppResult<()> {
    let actual = runner::sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected_hash.trim()) {
        Ok(())
    } else {
        Err(format!(
            "SHA-256 mismatch for `{}`. Expected {}, got {}.",
            path.display(),
            expected_hash,
            actual
        ))
    }
}

fn ensure_user_path_contains(bin_dir: &Path) -> AppResult<()> {
    let bin_dir_display = bin_dir.display().to_string();
    let script = format!(
        "$target = '{}'; \
         $current = [Environment]::GetEnvironmentVariable('Path', 'User'); \
         $parts = @(); \
         if ($current) {{ $parts = $current -split ';' | Where-Object {{ $_ -and $_.Trim() -ne '' }} }}; \
         if (-not ($parts | Where-Object {{ $_ -ieq $target }})) {{ \
             $updated = if ($parts.Count -gt 0) {{ ($parts + $target) -join ';' }} else {{ $target }}; \
             [Environment]::SetEnvironmentVariable('Path', $updated, 'User'); \
         }}",
        bin_dir_display.replace('\'', "''")
    );

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .status()
        .map_err(|error| format!("Could not update the user PATH: {error}"))?;

    if !status.success() {
        return Err("PowerShell could not update the user PATH.".to_string());
    }

    let current_path = env::var("PATH").unwrap_or_default();
    let already_present = current_path
        .split(';')
        .any(|entry| entry.eq_ignore_ascii_case(&bin_dir_display));

    if !already_present {
        let next_path = if current_path.is_empty() {
            bin_dir_display.clone()
        } else {
            format!("{bin_dir_display};{current_path}")
        };
        unsafe {
            env::set_var("PATH", next_path);
        }
    }

    Ok(())
}

fn ensure_ollama_installed_and_running(
    layout: &RuntimeLayout,
    manifest: &InstallerManifest,
) -> AppResult<PathBuf> {
    if let Some(path) = find_ollama_exe() {
        if !ollama_reachable() {
            start_ollama_service(&path, layout)?;
        }
        wait_for_url(OLLAMA_HEALTH_URL, Duration::from_secs(30))?;
        return Ok(path);
    }

    let installer_path = env::temp_dir().join("aegis-ollama-installer.exe");
    download_file(&manifest.ollama_installer_url, &installer_path)?;
    verify_sha256(&installer_path, &manifest.ollama_installer_sha256)?;
    run_ollama_installer(&installer_path)?;

    let ollama_exe = find_ollama_exe().ok_or_else(|| {
        "Ollama installation completed, but `ollama.exe` could not be located afterwards."
            .to_string()
    })?;

    start_ollama_service(&ollama_exe, layout)?;
    wait_for_url(OLLAMA_HEALTH_URL, Duration::from_secs(30))?;
    Ok(ollama_exe)
}

fn run_ollama_installer(installer_path: &Path) -> AppResult<()> {
    let script = format!(
        "Start-Process -FilePath '{}' -Verb RunAs -Wait",
        installer_path.display().to_string().replace('\'', "''")
    );

    let status = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &script])
        .status()
        .map_err(|error| format!("Could not launch the Ollama installer: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err("The Ollama installer did not complete successfully.".to_string())
    }
}

fn start_ollama_service(ollama_exe: &Path, layout: &RuntimeLayout) -> AppResult<()> {
    let args = vec!["serve".to_string()];
    let _ = runner::spawn_detached_process(
        ollama_exe,
        &args,
        &layout.root,
        &[],
        &layout.logs_dir.join("ollama.stdout.log"),
        &layout.logs_dir.join("ollama.stderr.log"),
    )?;

    Ok(())
}

fn ensure_model_available(ollama_exe: &Path, model: &str, layout: &RuntimeLayout) -> AppResult<()> {
    if ollama_has_model(model) {
        return Ok(());
    }

    let args = vec!["pull".to_string(), model.to_string()];
    runner::run_program_and_wait(&ollama_exe.display().to_string(), &args, &layout.root)
}

fn ollama_reachable() -> bool {
    url_reachable(OLLAMA_HEALTH_URL)
}

fn ollama_has_model(model: &str) -> bool {
    Client::new()
        .get(OLLAMA_HEALTH_URL)
        .send()
        .ok()
        .and_then(|response| response.error_for_status().ok())
        .and_then(|response| response.json::<OllamaTagsResponse>().ok())
        .map(|response| {
            response
                .models
                .iter()
                .any(|candidate| candidate.name.eq_ignore_ascii_case(model))
        })
        .unwrap_or(false)
}

fn wait_for_url(url: &str, timeout: Duration) -> AppResult<()> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if url_reachable(url) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(500));
    }

    Err(format!("Timed out waiting for `{url}` to become reachable."))
}

fn url_reachable(url: &str) -> bool {
    Client::new()
        .get(url)
        .timeout(Duration::from_secs(2))
        .send()
        .map(|response| response.status().is_success())
        .unwrap_or(false)
}

fn find_ollama_exe() -> Option<PathBuf> {
    let path_lookup = std::process::Command::new("where")
        .args(["ollama.exe"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| {
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(PathBuf::from)
        });

    if path_lookup.is_some() {
        return path_lookup;
    }

    env::var_os("LOCALAPPDATA").and_then(|local_app_data| {
        let candidate = PathBuf::from(local_app_data)
            .join("Programs")
            .join("Ollama")
            .join("ollama.exe");
        candidate.exists().then_some(candidate)
    })
}
