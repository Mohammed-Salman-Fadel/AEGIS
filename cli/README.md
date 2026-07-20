# AEGIS CLI

## Coding Workflows

The developer CLI treats a repository as a workspace, not as chat context. Inspection is offline and read-only:

```powershell
aegis code inspect --path D:\projects\my-app
```

Coding tasks use read-only model tools, then return a unified diff for local validation and review:

```powershell
# Default: validate the patch and ask before editing files.
aegis code task "Fix the failing authentication tests" --path .

# Explore and propose changes without touching the workspace.
aegis code task "Explain and review the cache layer" --permission read-only

# Apply a validated workspace-local patch without the final prompt.
# This still rejects absolute paths, parent traversal, .git targets, and invalid patches.
aegis code task "Add input validation" --permission workspace-write
```

After an approved patch, AEGIS displays detected verification commands and asks separately before running them. Model-authored shell commands are never executed as part of this flow.

Each task follows and persists `Understand -> Explore -> Plan -> Request permission -> Edit -> Format -> Test -> Review`. Inspect the latest state or restore a guarded pre-edit snapshot without starting the model runtime:

```powershell
aegis code plan --path .
aegis code checkpoints --path .
aegis code restore <checkpoint-id> --path .
```

The repository index caches file metadata, symbols, documentation, configuration, tests, and recent Git history. Unchanged files are reused on later tasks, while only the most relevant excerpts enter the model context. Repository guidance is loaded from `AGENTS.md`, `CONTRIBUTING.md`, `.github/CONTRIBUTING.md`, `CLAUDE.md`, `.aegis.md`, and local `.aegis` instruction files when present.

### Developer commands

```powershell
aegis explain src/auth --path .
aegis find "where sessions are persisted" --path .
aegis fix "model switching fails" --path .
aegis test --path .
aegis review --path .
```

- `explain` uses repository-scoped read-only tools to trace files, symbols, callers, and imports.
- `find` performs a fast offline text/symbol search and never starts the model runtime.
- `fix` uses the same validated patch workflow as `code task`.
- `test` selects checks from modified files and the nearest Cargo crate, Node package, or Python tests.
- `review` evaluates current changes for bugs, regressions, security risks, and missing coverage.

Coding tasks support `--quiet`, `--json`, `--diff-only`, `--explain`, `--reason`, and four permission modes: `read-only`, `ask-before-edit`, `workspace-write`, and `unattended-safe`. Unattended-safe requires a clean working tree, rejects deletion/rename/binary or oversized patches, requires affected checks, and rolls the patch back if verification fails.

CLI capabilities can be enabled or disabled in the web UI under **Settings > Command Line**. The CLI reads that policy before every invocation. Disabling file editing or the agentic loop forces coding tasks into read-only mode; disabling command execution also disables automatic verification. Git boundary and dirty-worktree protections cannot be disabled.

The `src/cli` crate is the terminal control surface for AEGIS. Coding workflows are connected to the local engine while repository discovery, indexing, patch validation, permission checks, checkpoints, and command execution remain enforced locally by the CLI.

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
