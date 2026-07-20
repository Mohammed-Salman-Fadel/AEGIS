# AEGIS CLI Agent Architecture

## Trust Boundary

The model may explore through read-only engine tools and propose a unified diff. It cannot write files or execute model-authored shell commands. The CLI validates paths and patch applicability, applies only after the selected permission policy allows it, and separately controls inferred formatting and test commands.

Git safety is an invariant. Absolute paths, parent traversal, `.git` targets, invalid patches, and unsafe overlap with pre-existing changes are rejected locally.

## Task Lifecycle

Coding tasks persist these stages outside the repository:

1. Understand
2. Explore
3. Plan
4. Request permission
5. Edit
6. Format
7. Test
8. Review

The same task resumes an incomplete plan after a CLI or engine restart. `aegis code plan --path .` displays the latest plan without starting backend services.

## Repository Context

AEGIS detects the repository root, Git state, languages, frameworks, package managers, and build systems. Repository instruction files are injected as authoritative guidance before model exploration.

The incremental semantic index records paths, file kinds, symbols, compact previews, modification metadata, and recent Git history. Unchanged entries are reused. Relevance scoring selects a bounded set of excerpts for each task, preventing repeated full-file reads and limiting context overhead.

## Checkpoints

Before applying a patch, AEGIS stores snapshots outside the working tree. A checkpoint records whether each target existed and hashes its post-edit state. Restore is refused when a target changed after AEGIS edited it, preserving subsequent user work.

Use `aegis code checkpoints --path .` to list snapshots and `aegis code restore <id> --path .` to restore one.

## Capability Policy

Settings > Command Line writes a user-local JSON policy shared by the engine and CLI. Capabilities cover repository detection, instructions, indexing, context budgeting, persistent plans, checkpoints, patch application, command execution, verification, and default deep reasoning.

Disabling a capability removes that authority at the CLI enforcement boundary. Git safety remains enabled even if an old or manually edited settings file attempts to disable it.
