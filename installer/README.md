# AEGIS Installer

The installer should support this first-run path:

1. The user downloads `AEGIS-Windows-x64.exe` from the landing page.
2. The user runs the executable.
3. Required local dependencies are installed or validated immediately.
4. The user runs `aegis open`.
5. AEGIS starts the local backend services, warms the active model, and opens the Web UI.

## Current Release Contract

The repository now verifies the parts of that path that are visible from source:

- `installer/AEGIS-Windows-x64.exe` is the source binary for the landing-page download.
- `landing page/public/downloads/AEGIS-Windows-x64.exe` is synchronized from the installer binary before builds.
- `landing page/dist/downloads/AEGIS-Windows-x64.exe` is verified to match the installer binary.
- The CLI exposes `aegis open`.
- `aegis open` participates in startup model warmup and opens the configured Web UI URL.

Run the full source-side verification from the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\verify-installation-pipeline.ps1
```

Use this stricter mode before public releases:

```powershell
powershell -ExecutionPolicy Bypass -File .\tools\verify-installation-pipeline.ps1 -RequireInstallerSource
```

## Important Gap

This directory currently contains a built Windows installer binary, but no installer source or build
recipe such as an NSIS `.nsi`, Inno Setup `.iss`, WiX project, or equivalent packaging manifest.

That means the repository can verify that users download the correct binary, but it cannot prove or
rebuild what the installer does after launch. In particular, source-side verification cannot confirm
that the installer downloads Python, Node, Ollama, models, service assets, or PATH entries.

For a fully reproducible release pipeline, add the installer source and make it responsible for:

- installing or validating the AEGIS CLI binary
- adding the CLI installation directory to PATH
- installing or validating Ollama
- pulling or validating the default local model
- installing or validating the Python runtime used by `python-services`
- installing Python dependencies for the RAG service
- installing or packaging the Web UI runtime/build
- installing or packaging the Rust engine runtime
- writing `AEGIS_INSTALL_ROOT` or the saved install-root preference
- running a post-install health check that confirms `aegis open` can start the local stack

## Local Runtime Expectations

At runtime, the CLI auto-starts local services unless `AEGIS_NO_AUTOSTART` is set to `1`, `true`,
`yes`, or `on`.

The current source-tree launcher expects these components:

- RAG service from `python-services`
- Rust engine from `engine`
- Web UI from `frontend`

The current launcher is developer-friendly because it can run those services from source. A packaged
installer should either install the same source layout plus dependencies, or update the launcher to
prefer packaged service binaries/assets before falling back to development commands.
