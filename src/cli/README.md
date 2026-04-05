# AEGIS CLI

The `src/cli` crate is the terminal control surface for AEGIS.
This pass intentionally keeps the CLI as a **TODO-first scaffold**: it compiles, exposes the target
command tree, prints friendly placeholder guidance, and documents how the CLI should connect to the
Rust engine without pretending the backend is fully wired.

## Current Direction

- The public command surface is centered on:
  - `aegis install`
  - `aegis chat "<prompt>"`
  - `aegis ask --stdin`
  - `aegis repl`
  - `aegis session new|list|show|use|reset`
  - `aegis provider list|select`
  - `aegis model list|select`
  - `aegis status`
  - `aegis doctor`
- The CLI must **not** own orchestration logic.
- Chat, session, provider, and model operations are planned around a **localhost HTTP boundary**
  into the Rust engine.
- The AEGIS ASCII banner is shown on:
  - bare `aegis`
  - `chat`
  - `ask --stdin`
  - `repl`
- Running bare `aegis` in an interactive terminal now opens a **live command shell** that keeps
  accepting commands until the user presses `Ctrl+C` or types `quit`.
- Interactive picking uses **numbered terminal menus** only when suitable and only in interactive
  terminals.

## Module Graph

- `main.rs`
  - builds `AppContext`
  - decides whether to show the banner
  - delegates into `commands.rs`
- `cli.rs`
  - defines the public Clap command tree
- `args.rs`
  - owns reusable argument structs shared by the command tree
- `commands.rs`
  - the only command-dispatch layer below `main.rs`
  - keeps handlers thin and scaffold-oriented
- `banner.rs`
  - owns AEGIS ASCII art and banner display policy
- `ui.rs`
  - owns terminal presentation helpers only
- `menu.rs`
  - owns numbered prompt scaffolding for interactive selection
- `engine_client.rs`
  - owns the future localhost HTTP client boundary to the engine
- `install.rs`
  - owns the staged dependency-install scaffold
- `doctor.rs`
  - owns read-only dependency and component checks
- `workspace.rs`
  - owns repository discovery and component path detection
- `runner.rs`
  - owns future subprocess launch plans for engine startup and installation steps

## Engine Boundary

The CLI is planned to talk to the Rust backend over localhost HTTP.

- Existing engine seam:
  - `/chat`
- Reserved future seams for the CLI scaffold:
  - `/health`
  - `/sessions`
  - `/providers`
  - `/models`

This keeps the CLI and Web UI on the same backend flow and avoids duplicating orchestration logic
inside the CLI crate.

## Installation Policy

`aegis install` is currently a **Windows-first staged installer scaffold**.

- Detailed TODO phases are written for Windows:
  - Rust toolchain
  - Ollama
  - Python
  - Node/npm
  - model pull
  - engine bootstrap
  - post-install verification
- Linux and macOS are included as placeholders only for now.

## Why It Is Scaffold-First

The goal of this pass is to make the intended architecture easy to continue implementing:

- every CLI file documents how it connects to the others
- the command tree matches the intended product direction
- the code stays compileable
- the remaining work is captured as explicit `TODO:` guidance rather than hidden assumptions
