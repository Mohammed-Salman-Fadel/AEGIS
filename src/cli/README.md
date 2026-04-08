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
