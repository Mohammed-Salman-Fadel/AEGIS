# AEGIS Installer

The installer folder is the source of truth for the Windows v1 bootstrap release. The user-facing flow is intentionally simple: download `aegis-bootstrap-windows-x64.exe`, run `aegis install`, and let the bootstrapper prepare the local runtime under `%LOCALAPPDATA%\AEGIS`.

## Release Artifacts

The Windows release produces exactly three artifacts:

- `aegis-bootstrap-windows-x64.exe`
- `aegis-runtime-windows-x64.zip`
- `installer-manifest.json`

The bootstrap executable downloads `installer-manifest.json`, verifies the runtime zip SHA-256, promotes the runtime into a staged per-user install, prepares RAG, starts the Rust engine, and opens the web UI at `http://localhost:8080`.

## Runtime Layout

The installed runtime is per-user and does not require Rust, Node, or system Python:

- `%LOCALAPPDATA%\AEGIS\bin`: user-facing `aegis.exe` and `aegis-engine.exe`
- `%LOCALAPPDATA%\AEGIS\runtime\current`: active promoted runtime bundle
- `%LOCALAPPDATA%\AEGIS\runtime\rag-venv`: local RAG virtual environment created from bundled Python
- `%LOCALAPPDATA%\AEGIS\config`: install state, cached manifest, and engine env
- `%LOCALAPPDATA%\AEGIS\data`: sessions, uploads, RAG indexes, and future user-local data
- `%LOCALAPPDATA%\AEGIS\logs`: engine, RAG, and Ollama startup logs
- `%LOCALAPPDATA%\AEGIS\run`: PID records for managed AEGIS processes

## Runtime Zip Contents

`aegis-runtime-windows-x64.zip` must contain:

- `bin/aegis.exe`
- `bin/aegis-engine.exe`
- `ui/` built frontend assets
- `rag/` Python RAG app, `requirements.txt`, and offline `wheels/`
- `python/` portable Python runtime with `python.exe`
- `config/default.env`
- `version.txt`

## Installer Behavior

`aegis install` performs a staged install:

1. Validate Windows x64 support.
2. Download and parse the installer manifest.
3. Download and SHA-256 verify the runtime zip.
4. Extract into a staged runtime directory.
5. Validate required runtime files, including RAG and bundled Python.
6. Promote staged runtime to `runtime/current`.
7. Sync launchers into `%LOCALAPPDATA%\AEGIS\bin`.
8. Add the launcher directory to the user PATH.
9. Detect Ollama and ask before installing it if missing.
10. Start Ollama when it is installed but not serving.
11. Pull the manifest default model, currently `llama3.2:3b`.
12. Create the RAG venv from bundled Python and offline wheels.
13. Write engine/RAG config.
14. Start the RAG sidecar and Rust engine.
15. Wait for health checks and open the UI.

Re-running `aegis install` is intended to be idempotent. Already satisfied steps are skipped where possible.

## Lifecycle Commands

- `aegis start`: starts Ollama if present, then managed RAG and engine.
- `aegis stop`: stops only AEGIS-managed RAG and engine processes.
- `aegis open`: opens `http://localhost:8080`.
- `aegis status`: reports installed version, engine state, RAG state, Ollama reachability, model presence, and URLs.
- `aegis doctor`: validates runtime files, writable folders, Ollama, model, RAG health/init, engine health, and UI assets.

## Packaging

Use `installer/windows/build_release.ps1` for a full release build. It builds the Rust CLI, Rust engine, frontend, packages the Python RAG runtime, downloads/prepares portable Python, creates the offline wheelhouse, and emits the three release artifacts in `dist/`.

Use `installer/windows/package_runtime.ps1` only when the CLI, engine, and frontend have already been built.
