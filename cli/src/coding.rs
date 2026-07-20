//! Developer-focused repository inspection and permission-controlled patch application.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::Serialize;

use crate::args::{
    CodeCheckpointArgs, CodeFindArgs, CodeQueryArgs, CodeTaskArgs, CodeTestArgs, CodeWorkspaceArgs,
    PermissionMode,
};
use crate::command_line::CommandLinePolicy;
use crate::developer_agent::{
    TaskPlan, capability_summary, create_checkpoint, current_plan, finalize_checkpoint,
    list_checkpoints, prepare_repository_context, restore_checkpoint as restore_saved_checkpoint,
    semantic_search,
};
use crate::{AppContext, AppResult};

const MAX_DISCOVERY_FILES: usize = 20_000;
const SKIPPED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
];

#[derive(Debug, Clone, Serialize)]
pub struct RepositoryContext {
    pub root: PathBuf,
    pub name: String,
    pub is_git: bool,
    pub branch: Option<String>,
    pub dirty_entries: usize,
    pub file_count: usize,
    pub truncated: bool,
    pub languages: Vec<(String, usize)>,
    pub build_systems: Vec<String>,
    pub frameworks: Vec<String>,
    pub package_managers: Vec<String>,
    pub dirty_files: Vec<String>,
    pub instruction_files: Vec<PathBuf>,
}

pub fn inspect(ctx: &AppContext, args: CodeWorkspaceArgs) -> AppResult<()> {
    let policy = CommandLinePolicy::load();
    require_capability(policy.repository_detection, "Repository detection")?;
    let repository = RepositoryContext::discover(&args.path)?;
    print_context(ctx, &repository);
    println!("Capabilities: {}", enabled_capabilities(&policy));
    Ok(())
}

pub fn checkpoints(ctx: &AppContext, args: CodeWorkspaceArgs) -> AppResult<()> {
    let policy = CommandLinePolicy::load();
    require_capability(policy.task_checkpoints, "Task checkpoints")?;
    let repository = RepositoryContext::discover(&args.path)?;
    let checkpoints = list_checkpoints(&repository.root)?;
    println!("{}", ctx.ui.section("Task Checkpoints"));
    if checkpoints.is_empty() {
        println!(
            "{}",
            ctx.ui
                .muted("No AEGIS checkpoints exist for this repository.")
        );
        return Ok(());
    }
    for checkpoint in checkpoints {
        println!("{}  {}", ctx.ui.header(&checkpoint.id), checkpoint.task);
        println!("  Files: {}", checkpoint.files.len());
    }
    println!();
    println!(
        "{}",
        ctx.ui
            .muted("Restore with `aegis code restore <checkpoint-id>`.")
    );
    Ok(())
}

pub fn show_plan(ctx: &AppContext, args: CodeWorkspaceArgs) -> AppResult<()> {
    let policy = CommandLinePolicy::load();
    require_capability(policy.persistent_task_plan, "Persistent task plan")?;
    require_capability(policy.repository_detection, "Repository detection")?;
    let repository = RepositoryContext::discover(&args.path)?;
    println!("{}", ctx.ui.section("Task Plan"));
    let Some(plan) = current_plan(&repository.root)? else {
        println!(
            "{}",
            ctx.ui
                .muted("No persisted task plan exists for this repository.")
        );
        return Ok(());
    };
    println!("Task       : {}", plan.task);
    println!("Repository : {}", plan.repository);
    for stage in plan.stages {
        let marker = match stage.status.as_str() {
            "completed" => "DONE",
            "in_progress" => "ACTIVE",
            _ => "WAITING",
        };
        println!("{marker:>7}  {}", stage.name);
    }
    Ok(())
}

pub fn restore_checkpoint(ctx: &AppContext, args: CodeCheckpointArgs) -> AppResult<()> {
    let policy = CommandLinePolicy::load();
    require_capability(policy.task_checkpoints, "Task checkpoints")?;
    require_capability(policy.patch_application, "Patch application")?;
    let repository = RepositoryContext::discover(&args.path)?;
    let restored = restore_saved_checkpoint(&repository.root, &args.id)?;
    println!(
        "{}",
        ctx.ui
            .success(&format!("Restored checkpoint `{}`.", args.id))
    );
    println!("Files: {}", restored.join(", "));
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct FindMatch {
    path: String,
    line: usize,
    text: String,
}

pub fn find(ctx: &AppContext, args: CodeFindArgs) -> AppResult<()> {
    let policy = CommandLinePolicy::load();
    require_capability(policy.repository_detection, "Repository detection")?;
    let repository = RepositoryContext::discover(&args.path)?;
    if !(1..=500).contains(&args.limit) {
        return Err("--limit must be between 1 and 500.".to_string());
    }
    let mut matches = find_in_repository(&repository.root, &args.query, args.limit)?;
    if policy.semantic_index && matches.len() < args.limit {
        let existing = matches
            .iter()
            .map(|item| item.path.clone())
            .collect::<BTreeSet<_>>();
        for hit in semantic_search(&repository.root, &args.query, args.limit - matches.len())? {
            if existing.contains(&hit.path) {
                continue;
            }
            matches.push(FindMatch {
                path: hit.path,
                line: 1,
                text: format!(
                    "[semantic score {}] {}{}",
                    hit.score,
                    if hit.symbols.is_empty() {
                        String::new()
                    } else {
                        format!("symbols: {} | ", hit.symbols.join(", "))
                    },
                    hit.preview
                ),
            });
        }
    }
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&matches).map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    println!("{}", ctx.ui.section("Find in Workspace"));
    println!("Query      : {}", args.query);
    println!("Repository : {}", repository.root.display());
    println!("Matches    : {}", matches.len());
    println!();
    if matches.is_empty() {
        println!(
            "{}",
            ctx.ui
                .muted("No matching lines found. Try a shorter symbol or phrase.")
        );
    } else {
        for item in matches {
            println!("{}:{}  {}", item.path, item.line, item.text);
        }
    }
    Ok(())
}

pub fn explain(ctx: &AppContext, args: CodeQueryArgs) -> AppResult<()> {
    let query = args
        .query
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Explain requires a file, symbol, subsystem, or question.".to_string())?;
    read_only_task(
        ctx,
        &args.path,
        &format!(
            "Explain this project topic clearly, tracing definitions, callers, imports, and architecture where relevant: {}",
            query
        ),
        args.json,
    )
}

pub fn review(ctx: &AppContext, args: CodeQueryArgs) -> AppResult<()> {
    let focus = if args
        .query
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .is_empty()
    {
        "all current working-tree changes"
    } else {
        args.query.as_deref().unwrap().trim()
    };
    read_only_task(
        ctx,
        &args.path,
        &format!(
            "Review {focus}. Prioritize concrete bugs, behavioral regressions, security risks, and missing tests. Cite file paths and lines. Do not propose edits unless needed to demonstrate a finding."
        ),
        args.json,
    )
}

pub fn test_affected(ctx: &AppContext, args: CodeTestArgs) -> AppResult<()> {
    let repository = RepositoryContext::discover(&args.path)?;
    let changed = repository.dirty_files.clone();
    let commands = affected_test_commands(&repository, &changed);
    if args.json {
        let value = serde_json::json!({"repository": repository.root, "changed_files": changed, "commands": commands.iter().map(VerificationCommand::display).collect::<Vec<_>>()});
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    println!("{}", ctx.ui.section("Affected Tests"));
    println!("Changed files: {}", changed.len());
    if commands.is_empty() {
        println!(
            "{}",
            ctx.ui.muted("No affected test command could be inferred.")
        );
        return Ok(());
    }
    for command in &commands {
        print_command_approval(ctx, &repository.root, command);
    }
    if !args.yes && !confirm(ctx, "Run these affected checks? [y/N]")? {
        println!("{}", ctx.ui.muted("Tests skipped."));
        return Ok(());
    }
    for command in commands {
        run_verification_command(ctx, &repository.root, &command)?;
    }
    println!("{}", ctx.ui.success("Affected checks passed."));
    Ok(())
}

pub fn task(ctx: &AppContext, args: CodeTaskArgs) -> AppResult<()> {
    let json = args.json;
    let task_name = args.task.clone();
    match task_inner(ctx, args) {
        Ok(()) => Ok(()),
        Err(error) => {
            if json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &serde_json::json!({"status": "failed", "task": task_name, "error": error})
                    )
                    .unwrap_or_else(|_| "{\"status\":\"failed\"}".to_string())
                );
            } else {
                println!();
                println!("{}", ctx.ui.section("Change Summary"));
                println!("Status     : failed");
                println!(
                    "Files      : inspect git status; a failure may occur after patch application"
                );
                println!("Checks     : incomplete");
                println!("Error      : {error}");
            }
            Err(error)
        }
    }
}

fn task_inner(ctx: &AppContext, args: CodeTaskArgs) -> AppResult<()> {
    let policy = CommandLinePolicy::load();
    require_capability(policy.repository_detection, "Repository detection")?;
    let repository = RepositoryContext::discover(&args.path)?;
    let effective_permission = if policy.agentic_loop && policy.patch_application {
        args.permission
    } else {
        PermissionMode::ReadOnly
    };
    let prepared = prepare_repository_context(
        &repository.root,
        &repository.instruction_files,
        &args.task,
        &policy,
    )?;
    let mut plan = TaskPlan::start(&repository.root, &args.task, policy.persistent_task_plan)?;
    advance_plan(
        ctx,
        &mut plan,
        &repository.root,
        "Understand",
        should_announce_progress(&args),
    )?;
    if !args.quiet && !args.json && !args.diff_only {
        print_context(ctx, &repository);
        println!(
            "Index      : {} cached file entries",
            prepared.indexed_files
        );
        println!("Capabilities: {}", enabled_capabilities(&policy));
        if let Some(plan) = plan.as_ref() {
            println!("Plan       : {}", plan.summary());
        }
        if effective_permission != args.permission {
            println!(
                "{}",
                ctx.ui.warning(
                    "Editing is disabled by Command Line settings; this task is read-only."
                )
            );
        }
    }
    let existing = repository
        .dirty_files
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut request = args.task.trim().to_string();
    let mut revision = 0usize;

    loop {
        advance_plan(
            ctx,
            &mut plan,
            &repository.root,
            "Explore",
            should_announce_progress(&args),
        )?;
        if !args.quiet && !args.json && !args.diff_only {
            println!();
            println!(
                "{}",
                ctx.ui
                    .section(if revision == 0 { "Task" } else { "Revision" })
            );
            println!("Request    : {}", request);
            println!("Permission : {}", effective_permission.label());
            println!("{}", ctx.ui.muted("Exploration tools are read-only; file changes must pass local patch validation."));
        }
        advance_plan(
            ctx,
            &mut plan,
            &repository.root,
            "Plan",
            should_announce_progress(&args),
        )?;
        let prompt = coding_prompt(
            &repository,
            &request,
            effective_permission,
            &prepared.instructions,
            &prepared.ranked_context,
        );
        let reply = if args.quiet || args.json || args.diff_only {
            ctx.engine.code_task(
                &prompt,
                &repository.name,
                &repository.root,
                args.reason || policy.deep_reasoning,
                |_| Ok(()),
            )?
        } else {
            crate::commands::stream_llm_response(ctx, |on_token| {
                ctx.engine.code_task(
                    &prompt,
                    &repository.name,
                    &repository.root,
                    args.reason || policy.deep_reasoning,
                    on_token,
                )
            })?
        };
        let Some(mut patch) = extract_unified_diff(&reply.message) else {
            complete_plan(&mut plan, &repository.root)?;
            let summary =
                TaskSummary::unchanged(&repository, &args.task, "No patch proposed", reply.message);
            print_task_summary(ctx, &summary, &args)?;
            return Ok(());
        };
        validate_patch_paths(&patch)?;
        verify_patch(&repository.root, &patch)?;
        if args.diff_only {
            complete_plan(&mut plan, &repository.root)?;
            print!("{patch}");
            return Ok(());
        }
        if !args.quiet && !args.json {
            println!();
            println!("{}", ctx.ui.section("Proposed Patch"));
            println!("{patch}");
        }
        if args.explain || effective_permission == PermissionMode::ReadOnly {
            complete_plan(&mut plan, &repository.root)?;
            let summary =
                TaskSummary::previewed(&repository, &args.task, patch_files(&patch), reply.message);
            print_task_summary(ctx, &summary, &args)?;
            return Ok(());
        }

        advance_plan(
            ctx,
            &mut plan,
            &repository.root,
            "Request permission",
            should_announce_progress(&args),
        )?;
        match effective_permission {
            PermissionMode::AskBeforeEdit => match review_patch(ctx, &patch)? {
                PatchDecision::ApplyAll => {}
                PatchDecision::ApplySelected(selected) => patch = selected,
                PatchDecision::Revise(feedback) => {
                    revision += 1;
                    if revision >= 3 {
                        return Err("Revision limit reached without an approved patch.".to_string());
                    }
                    request = format!(
                        "{}\n\nRevise the previous proposal using this feedback: {}",
                        args.task.trim(),
                        feedback
                    );
                    continue;
                }
                PatchDecision::Reject => {
                    let summary = TaskSummary::unchanged(
                        &repository,
                        &args.task,
                        "Patch rejected",
                        reply.message,
                    );
                    print_task_summary(ctx, &summary, &args)?;
                    complete_plan(&mut plan, &repository.root)?;
                    return Ok(());
                }
            },
            PermissionMode::UnattendedSafe => {
                require_capability(policy.command_execution, "Command execution")?;
                validate_unattended_patch(&repository, &patch)?
            }
            PermissionMode::WorkspaceWrite => {}
            PermissionMode::ReadOnly => unreachable!(),
        }

        let touched = patch_files(&patch);
        let overlaps = touched
            .iter()
            .filter(|path| existing.contains(*path))
            .cloned()
            .collect::<Vec<_>>();
        if policy.git_safety
            && !overlaps.is_empty()
            && effective_permission != PermissionMode::AskBeforeEdit
        {
            return Err(format!(
                "Patch overlaps pre-existing user changes in: {}. Re-run with --permission ask-before-edit for explicit review.",
                overlaps.join(", ")
            ));
        }
        let unattended_commands = if effective_permission == PermissionMode::UnattendedSafe {
            let commands = affected_test_commands(&repository, &touched);
            if commands.is_empty() {
                return Err("Unattended-safe mode requires an affected verification command; none could be inferred for this patch.".to_string());
            }
            Some(commands)
        } else {
            None
        };
        let mut checkpoint = if policy.task_checkpoints {
            Some(create_checkpoint(&repository.root, &args.task, &touched)?)
        } else {
            None
        };
        advance_plan(
            ctx,
            &mut plan,
            &repository.root,
            "Edit",
            should_announce_progress(&args),
        )?;
        verify_patch(&repository.root, &patch)?;
        apply_patch(&repository.root, &patch)?;
        if let Some(checkpoint) = checkpoint.as_mut() {
            finalize_checkpoint(&repository.root, checkpoint)?;
        }
        advance_plan(
            ctx,
            &mut plan,
            &repository.root,
            "Format",
            should_announce_progress(&args),
        )?;
        let tests = if !policy.command_execution || !policy.automatic_verification {
            Vec::new()
        } else if let Some(commands) = unattended_commands {
            for command in &commands {
                print_command_approval(ctx, &repository.root, command);
            }
            match run_commands(ctx, &repository.root, commands, true) {
                Ok(passed) => passed,
                Err(error) => {
                    reverse_patch(&repository.root, &patch).map_err(|rollback| format!("Verification failed: {error}. Automatic rollback also failed: {rollback}"))?;
                    return Err(format!(
                        "Verification failed and the unattended patch was rolled back: {error}"
                    ));
                }
            }
        } else {
            offer_verification(ctx, &repository, &touched)?
        };
        advance_plan(
            ctx,
            &mut plan,
            &repository.root,
            "Test",
            should_announce_progress(&args),
        )?;
        let after = git_status_files(&repository.root)
            .into_iter()
            .collect::<BTreeSet<_>>();
        let aegis_changes = after.difference(&existing).cloned().collect::<Vec<_>>();
        let summary = TaskSummary {
            status: "applied".into(),
            task: args.task.clone(),
            repository: repository.root.display().to_string(),
            permission: effective_permission.label().into(),
            files_changed: touched,
            pre_existing_changes: existing.into_iter().collect(),
            aegis_changes,
            tests,
            warnings: vec![format!(
                "Review the resulting diff before committing.{}",
                checkpoint
                    .as_ref()
                    .map(|value| format!(" Checkpoint: {}.", value.id))
                    .unwrap_or_default()
            )],
            answer: None,
        };
        advance_plan(
            ctx,
            &mut plan,
            &repository.root,
            "Review",
            should_announce_progress(&args),
        )?;
        print_task_summary(ctx, &summary, &args)?;
        complete_plan(&mut plan, &repository.root)?;
        return Ok(());
    }
}

fn read_only_task(ctx: &AppContext, path: &Path, request: &str, json: bool) -> AppResult<()> {
    let policy = CommandLinePolicy::load();
    require_capability(policy.repository_detection, "Repository detection")?;
    let repository = RepositoryContext::discover(path)?;
    let prepared = prepare_repository_context(
        &repository.root,
        &repository.instruction_files,
        request,
        &policy,
    )?;
    if !json {
        print_context(ctx, &repository);
        println!();
    }
    let prompt = coding_prompt(
        &repository,
        request,
        PermissionMode::ReadOnly,
        &prepared.instructions,
        &prepared.ranked_context,
    );
    let reply = if json {
        ctx.engine.code_task(
            &prompt,
            &repository.name,
            &repository.root,
            policy.deep_reasoning,
            |_| Ok(()),
        )?
    } else {
        crate::commands::stream_llm_response(ctx, |on_token| {
            ctx.engine.code_task(
                &prompt,
                &repository.name,
                &repository.root,
                policy.deep_reasoning,
                on_token,
            )
        })?
    };
    if json {
        let value = serde_json::json!({"repository": repository.root, "query": request, "answer": reply.message});
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?
        );
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct TaskSummary {
    status: String,
    task: String,
    repository: String,
    permission: String,
    files_changed: Vec<String>,
    pre_existing_changes: Vec<String>,
    aegis_changes: Vec<String>,
    tests: Vec<String>,
    warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    answer: Option<String>,
}

impl TaskSummary {
    fn unchanged(repository: &RepositoryContext, task: &str, reason: &str, answer: String) -> Self {
        Self {
            status: reason.to_string(),
            task: task.to_string(),
            repository: repository.root.display().to_string(),
            permission: "no-write".into(),
            files_changed: Vec::new(),
            pre_existing_changes: repository.dirty_files.clone(),
            aegis_changes: Vec::new(),
            tests: Vec::new(),
            warnings: Vec::new(),
            answer: Some(answer),
        }
    }

    fn previewed(
        repository: &RepositoryContext,
        task: &str,
        files: Vec<String>,
        answer: String,
    ) -> Self {
        Self {
            status: "previewed".into(),
            task: task.to_string(),
            repository: repository.root.display().to_string(),
            permission: "read-only".into(),
            files_changed: files,
            pre_existing_changes: repository.dirty_files.clone(),
            aegis_changes: Vec::new(),
            tests: Vec::new(),
            warnings: vec!["No files were modified.".into()],
            answer: Some(answer),
        }
    }
}

fn print_task_summary(
    ctx: &AppContext,
    summary: &TaskSummary,
    args: &CodeTaskArgs,
) -> AppResult<()> {
    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(summary).map_err(|error| error.to_string())?
        );
        return Ok(());
    }
    println!();
    println!("{}", ctx.ui.section("Change Summary"));
    println!("Status     : {}", summary.status);
    println!(
        "Files      : {}",
        if summary.files_changed.is_empty() {
            "none".into()
        } else {
            summary.files_changed.join(", ")
        }
    );
    println!(
        "AEGIS new  : {}",
        if summary.aegis_changes.is_empty() {
            "none".into()
        } else {
            summary.aegis_changes.join(", ")
        }
    );
    println!(
        "Preserved  : {} pre-existing change{}",
        summary.pre_existing_changes.len(),
        if summary.pre_existing_changes.len() == 1 {
            ""
        } else {
            "s"
        }
    );
    println!(
        "Checks     : {}",
        if summary.tests.is_empty() {
            "not run".into()
        } else {
            summary.tests.join(", ")
        }
    );
    if !summary.warnings.is_empty() {
        println!("Warnings   : {}", summary.warnings.join(" "));
    }
    Ok(())
}

fn find_in_repository(root: &Path, query: &str, limit: usize) -> AppResult<Vec<FindMatch>> {
    let needle = query.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return Err("Search query cannot be empty.".to_string());
    }
    let mut results = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("Could not search `{}`: {error}", directory.display()))?
            .flatten()
        {
            let path = entry.path();
            if path.is_dir() {
                if !path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(should_skip_dir)
                {
                    pending.push(path);
                }
                continue;
            }
            if !is_searchable_file(&path) {
                continue;
            }
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            for (index, line) in content.lines().enumerate() {
                if line.to_ascii_lowercase().contains(&needle) {
                    results.push(FindMatch {
                        path: path
                            .strip_prefix(root)
                            .unwrap_or(&path)
                            .display()
                            .to_string(),
                        line: index + 1,
                        text: line.trim().chars().take(240).collect(),
                    });
                    if results.len() >= limit {
                        return Ok(results);
                    }
                }
            }
        }
    }
    results.sort_by(|left, right| left.path.cmp(&right.path).then(left.line.cmp(&right.line)));
    Ok(results)
}

fn is_searchable_file(path: &Path) -> bool {
    const EXTENSIONS: &[&str] = &[
        "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "cs", "c", "h", "cpp", "hpp",
        "rb", "php", "swift", "vue", "svelte", "html", "css", "scss", "json", "toml", "yaml",
        "yml", "md", "txt", "xml",
    ];
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| {
            EXTENSIONS
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

impl RepositoryContext {
    pub fn discover(requested: &Path) -> AppResult<Self> {
        let requested = if requested.is_absolute() {
            requested.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|error| format!("Could not read the current directory: {error}"))?
                .join(requested)
        };
        let start = requested.canonicalize().map_err(|error| {
            format!("Could not open project `{}`: {error}", requested.display())
        })?;
        if !start.is_dir() {
            return Err(format!(
                "Project path `{}` is not a directory.",
                start.display()
            ));
        }

        let root = git_root(&start).unwrap_or(start);
        let is_git = root.join(".git").exists();
        let branch = is_git
            .then(|| git_text(&root, &["branch", "--show-current"]))
            .flatten();
        let dirty_files = if is_git {
            git_status_files(&root)
        } else {
            Vec::new()
        };
        let dirty_entries = dirty_files.len();

        let mut extensions = BTreeMap::new();
        let mut file_count = 0usize;
        let mut truncated = false;
        scan_files(&root, &mut file_count, &mut truncated, &mut extensions)?;
        let mut language_counts = BTreeMap::<String, usize>::new();
        for (extension, count) in extensions {
            if let Some(name) = language_name(&extension) {
                *language_counts.entry(name.to_string()).or_default() += count;
            }
        }
        let mut languages = language_counts.into_iter().collect::<Vec<_>>();
        languages.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        languages.truncate(8);

        let build_systems = detect_build_systems(&root);
        let package_managers = detect_package_managers(&root);
        let frameworks = detect_frameworks(&root);
        let instruction_files = [
            "AGENTS.md",
            "CONTRIBUTING.md",
            ".github/CONTRIBUTING.md",
            "CLAUDE.md",
            ".aegis.md",
            ".aegis/AGENTS.md",
            ".aegis/instructions.md",
        ]
        .into_iter()
        .map(|name| root.join(name))
        .filter(|path| path.is_file())
        .collect();
        let name = root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_string();

        Ok(Self {
            root,
            name,
            is_git,
            branch,
            dirty_entries,
            file_count,
            truncated,
            languages,
            build_systems,
            frameworks,
            package_managers,
            dirty_files,
            instruction_files,
        })
    }
}

fn print_context(ctx: &AppContext, repository: &RepositoryContext) {
    println!("{}", ctx.ui.section("Coding Workspace"));
    println!("Repository : {}", repository.root.display());
    println!(
        "Git        : {}",
        if repository.is_git {
            "detected"
        } else {
            "not detected"
        }
    );
    if let Some(branch) = repository.branch.as_deref() {
        println!("Branch     : {branch}");
    }
    println!(
        "Changes    : {} existing working-tree entr{}",
        repository.dirty_entries,
        if repository.dirty_entries == 1 {
            "y"
        } else {
            "ies"
        }
    );
    println!(
        "Files      : {}{}",
        repository.file_count,
        if repository.truncated {
            "+ (scan capped)"
        } else {
            ""
        }
    );
    println!(
        "Languages  : {}",
        if repository.languages.is_empty() {
            "unknown".to_string()
        } else {
            repository
                .languages
                .iter()
                .map(|(name, count)| format!("{name} ({count})"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!(
        "Build      : {}",
        if repository.build_systems.is_empty() {
            "not detected".to_string()
        } else {
            repository.build_systems.join(", ")
        }
    );
    println!(
        "Packages   : {}",
        if repository.package_managers.is_empty() {
            "not detected".to_string()
        } else {
            repository.package_managers.join(", ")
        }
    );
    println!(
        "Frameworks : {}",
        if repository.frameworks.is_empty() {
            "not detected".to_string()
        } else {
            repository.frameworks.join(", ")
        }
    );
    if !repository.instruction_files.is_empty() {
        println!(
            "Guidance   : {}",
            repository
                .instruction_files
                .iter()
                .filter_map(|path| path.file_name()?.to_str())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

fn coding_prompt(
    repository: &RepositoryContext,
    task: &str,
    permission: PermissionMode,
    instructions: &str,
    ranked_context: &str,
) -> String {
    format!(
        "You are operating as a careful coding agent inside the repository at `{}`.\n\
         User task: {}\n\
         Permission policy: {}.\n\n\
         Follow this controlled loop: Understand -> Explore -> Plan -> Request permission -> Edit -> Format -> Test -> Review. \
         Inspect the repository with read-only tools before answering. Treat the supplied repository instructions as authoritative. \
         Preserve existing user changes and do not claim that files were edited by tool calls. Terminal tools are read-only. \
         If code changes are appropriate, finish with one valid unified Git diff in a ```diff fenced block. \
         Keep every patch path relative to the repository and never target .git or a path outside the repository. \
         If the request is informational or evidence is insufficient, explain the result without inventing a patch. \
         Include the verification commands that should be run after an applied patch.\n\n\
         REPOSITORY INSTRUCTIONS:\n{}\n\n\
         RANKED REPOSITORY CONTEXT:\n{}",
        repository.root.display(),
        task.trim(),
        permission.label(),
        if instructions.is_empty() {
            "No repository instruction files were enabled or found."
        } else {
            instructions
        },
        if ranked_context.is_empty() {
            "No semantic repository context was enabled or available. Use read-only tools to explore."
        } else {
            ranked_context
        },
    )
}

fn require_capability(enabled: bool, name: &str) -> AppResult<()> {
    if enabled {
        Ok(())
    } else {
        Err(format!(
            "{name} is disabled in Settings > Command Line. Enable it before running this command."
        ))
    }
}

fn enabled_capabilities(policy: &CommandLinePolicy) -> String {
    capability_summary(policy)
        .into_iter()
        .filter_map(|(name, enabled)| enabled.then_some(name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn advance_plan(
    ctx: &AppContext,
    plan: &mut Option<TaskPlan>,
    root: &Path,
    stage: &str,
    announce: bool,
) -> AppResult<()> {
    if let Some(plan) = plan.as_mut() {
        plan.advance(root, stage)?;
        if announce {
            println!("{}  {}", ctx.ui.muted("Progress"), ctx.ui.header(stage));
        }
    }
    Ok(())
}

fn should_announce_progress(args: &CodeTaskArgs) -> bool {
    !args.quiet && !args.json && !args.diff_only
}

fn complete_plan(plan: &mut Option<TaskPlan>, root: &Path) -> AppResult<()> {
    if let Some(plan) = plan.as_mut() {
        plan.complete(root)?;
    }
    Ok(())
}

fn extract_unified_diff(response: &str) -> Option<String> {
    if let Some(start) = response.find("```diff") {
        let body = &response[start + "```diff".len()..];
        let end = body.find("```").unwrap_or(body.len());
        let patch = body[..end].trim();
        return (!patch.is_empty()).then(|| format!("{patch}\n"));
    }
    let start = response.find("diff --git ")?;
    let patch = response[start..].trim();
    (!patch.is_empty()).then(|| format!("{patch}\n"))
}

fn split_patch(patch: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();
    for line in patch.lines() {
        if line.starts_with("diff --git ") && !current.is_empty() {
            chunks.push(format!("{}\n", current.trim_end()));
            current.clear();
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.is_empty() {
        chunks.push(format!("{}\n", current.trim_end()));
    }
    chunks
}

fn patch_files(patch: &str) -> Vec<String> {
    split_patch(patch)
        .iter()
        .filter_map(|chunk| {
            let header = chunk.lines().find(|line| line.starts_with("diff --git "))?;
            let raw = header.split_whitespace().nth(3)?;
            Some(
                raw.trim_matches('"')
                    .strip_prefix("b/")
                    .unwrap_or(raw.trim_matches('"'))
                    .to_string(),
            )
        })
        .collect()
}

enum PatchDecision {
    ApplyAll,
    ApplySelected(String),
    Revise(String),
    Reject,
}

fn review_patch(ctx: &AppContext, patch: &str) -> AppResult<PatchDecision> {
    if !io::stdin().is_terminal() {
        return Err("ask-before-edit requires an interactive terminal. Use read-only for non-interactive review.".to_string());
    }
    println!();
    println!("{}", ctx.ui.section("Patch Review"));
    println!("[a] approve all   [f] review files   [r] request revision   [n] reject");
    print!("{} ", ctx.ui.info("Choice:"));
    io::stdout().flush().map_err(|error| error.to_string())?;
    let mut choice = String::new();
    io::stdin()
        .read_line(&mut choice)
        .map_err(|error| error.to_string())?;
    match choice.trim().to_ascii_lowercase().as_str() {
        "a" | "all" | "yes" | "y" => Ok(PatchDecision::ApplyAll),
        "f" | "files" => {
            let mut selected = Vec::new();
            for chunk in split_patch(patch) {
                let name = patch_files(&chunk)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "unknown file".into());
                println!();
                println!("{}", ctx.ui.header(&name));
                println!("{chunk}");
                if confirm(ctx, "Apply this file? [y/N]")? {
                    selected.push(chunk);
                }
            }
            if selected.is_empty() {
                Ok(PatchDecision::Reject)
            } else {
                Ok(PatchDecision::ApplySelected(selected.concat()))
            }
        }
        "r" | "revise" => {
            print!("{} ", ctx.ui.info("Revision feedback:"));
            io::stdout().flush().map_err(|error| error.to_string())?;
            let mut feedback = String::new();
            io::stdin()
                .read_line(&mut feedback)
                .map_err(|error| error.to_string())?;
            let feedback = feedback.trim();
            if feedback.is_empty() {
                Ok(PatchDecision::Reject)
            } else {
                Ok(PatchDecision::Revise(feedback.to_string()))
            }
        }
        _ => Ok(PatchDecision::Reject),
    }
}

fn validate_patch_paths(patch: &str) -> AppResult<()> {
    let mut paths = BTreeSet::new();
    for line in patch.lines() {
        let candidates: Vec<&str> = if let Some(rest) = line.strip_prefix("diff --git ") {
            rest.split_whitespace().take(2).collect()
        } else if let Some(rest) = line
            .strip_prefix("+++ ")
            .or_else(|| line.strip_prefix("--- "))
        {
            vec![rest.split_whitespace().next().unwrap_or("")]
        } else {
            Vec::new()
        };
        for raw in candidates {
            if raw == "/dev/null" || raw.is_empty() {
                continue;
            }
            let normalized = raw
                .trim_matches('"')
                .strip_prefix("a/")
                .or_else(|| raw.trim_matches('"').strip_prefix("b/"))
                .unwrap_or(raw.trim_matches('"'));
            let path = Path::new(normalized);
            if path.is_absolute()
                || normalized.contains(':')
                || path.components().any(|part| {
                    matches!(
                        part,
                        Component::ParentDir | Component::RootDir | Component::Prefix(_)
                    )
                })
            {
                return Err(format!(
                    "Patch rejected: path `{raw}` escapes the repository."
                ));
            }
            if path
                .components()
                .any(|part| part.as_os_str().eq_ignore_ascii_case(".git"))
            {
                return Err(format!(
                    "Patch rejected: path `{raw}` targets Git metadata."
                ));
            }
            paths.insert(normalized.to_string());
        }
    }
    if paths.is_empty() {
        return Err("Patch rejected: no file paths were found in the proposed diff.".to_string());
    }
    Ok(())
}

fn verify_patch(root: &Path, patch: &str) -> AppResult<()> {
    run_git_apply(root, patch, true).map(|_| ())
}

fn apply_patch(root: &Path, patch: &str) -> AppResult<()> {
    run_git_apply(root, patch, false).map(|_| ())
}

fn reverse_patch(root: &Path, patch: &str) -> AppResult<()> {
    let mut command = Command::new("git");
    command
        .arg("-c")
        .arg("safe.directory=*")
        .args(["apply", "--reverse", "--whitespace=nowarn", "-"])
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start patch rollback: {error}"))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Could not open rollback input.".to_string())?
        .write_all(patch.as_bytes())
        .map_err(|error| error.to_string())?;
    let output = child
        .wait_with_output()
        .map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn run_git_apply(root: &Path, patch: &str, check: bool) -> AppResult<String> {
    let mut command = Command::new("git");
    command.arg("-c").arg("safe.directory=*").arg("apply");
    if check {
        command.arg("--check");
    }
    command
        .arg("--whitespace=error")
        .arg("-")
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start `git apply`: {error}"))?;
    child
        .stdin
        .as_mut()
        .ok_or_else(|| "Could not open patch input.".to_string())?
        .write_all(patch.as_bytes())
        .map_err(|error| format!("Could not send patch to Git: {error}"))?;
    let output = child
        .wait_with_output()
        .map_err(|error| format!("Could not finish patch validation: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(format!(
            "Patch {} failed: {}",
            if check { "validation" } else { "application" },
            if detail.is_empty() {
                "git apply rejected the patch"
            } else {
                &detail
            }
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn confirm(ctx: &AppContext, message: &str) -> AppResult<bool> {
    if !io::stdin().is_terminal() {
        return Ok(false);
    }
    print!("{} ", ctx.ui.warning(message));
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not show approval prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("Could not read approval: {error}"))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn validate_unattended_patch(repository: &RepositoryContext, patch: &str) -> AppResult<()> {
    if repository.dirty_entries > 0 {
        return Err("Unattended-safe mode requires a clean working tree so existing user changes cannot be overwritten.".to_string());
    }
    let lower = patch.to_ascii_lowercase();
    if [
        "deleted file mode",
        "rename from ",
        "rename to ",
        "binary files ",
        "git binary patch",
        "+++ /dev/null",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
    {
        return Err("Unattended-safe mode rejected a deletion, rename, or binary change. Use ask-before-edit to review it interactively.".to_string());
    }
    let file_count = patch
        .lines()
        .filter(|line| line.starts_with("diff --git "))
        .count();
    let additions = patch
        .lines()
        .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
        .count();
    if file_count > 10 || additions > 1_000 {
        return Err(format!(
            "Unattended-safe mode limits patches to 10 files and 1000 added lines; proposed patch has {file_count} files and {additions} additions."
        ));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct VerificationCommand {
    label: &'static str,
    purpose: &'static str,
    risk: &'static str,
    program: String,
    args: Vec<String>,
}

impl VerificationCommand {
    fn display(&self) -> String {
        format!("{} {}", self.program, self.args.join(" "))
            .trim()
            .to_string()
    }
}

fn offer_verification(
    ctx: &AppContext,
    repository: &RepositoryContext,
    touched: &[String],
) -> AppResult<Vec<String>> {
    let mut commands = affected_test_commands(repository, touched);
    if commands.is_empty() {
        commands = verification_commands(repository);
    }
    if commands.is_empty() {
        println!(
            "{}",
            ctx.ui
                .muted("No standard verification command was detected.")
        );
        return Ok(Vec::new());
    }
    println!();
    println!("{}", ctx.ui.section("Suggested Verification"));
    for command in &commands {
        print_command_approval(ctx, &repository.root, command);
    }
    if !io::stdin().is_terminal() {
        println!(
            "{}",
            ctx.ui
                .muted("Checks were not run because approval requires an interactive terminal.")
        );
        return Ok(Vec::new());
    }
    print!("{} ", ctx.ui.warning("Run these checks now? [y/N]"));
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not show verification prompt: {error}"))?;
    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| format!("Could not read verification approval: {error}"))?;
    if !matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes") {
        println!("{}", ctx.ui.muted("Verification skipped."));
        return Ok(Vec::new());
    }
    let passed = run_commands(ctx, &repository.root, commands, true)?;
    println!(
        "{}",
        ctx.ui.success("All approved verification checks passed.")
    );
    Ok(passed)
}

fn run_commands(
    ctx: &AppContext,
    root: &Path,
    commands: Vec<VerificationCommand>,
    approved: bool,
) -> AppResult<Vec<String>> {
    if !approved {
        return Ok(Vec::new());
    }
    let mut passed = Vec::new();
    for command in commands {
        run_verification_command(ctx, root, &command)?;
        passed.push(command.display());
    }
    Ok(passed)
}

fn verification_commands(repository: &RepositoryContext) -> Vec<VerificationCommand> {
    let mut commands = Vec::new();
    if repository.root.join("Cargo.toml").is_file() {
        commands.push(VerificationCommand {
            label: "Rust format",
            purpose: "Verify Rust formatting without changing files",
            risk: "low (project build configuration may execute)",
            program: "cargo".to_string(),
            args: vec!["fmt".into(), "--check".into()],
        });
        commands.push(VerificationCommand {
            label: "Rust tests",
            purpose: "Compile and run the Rust test suite",
            risk: "medium (project tests and build scripts execute)",
            program: "cargo".to_string(),
            args: vec!["test".into()],
        });
    }
    if repository.root.join("package.json").is_file() {
        let program = if cfg!(windows) { "npm.cmd" } else { "npm" };
        commands.push(VerificationCommand {
            label: "Node tests",
            purpose: "Run the package-defined Node.js test script",
            risk: "medium (package scripts execute project code)",
            program: program.to_string(),
            args: vec!["test".into()],
        });
    }
    if repository.root.join("pyproject.toml").is_file()
        || repository.root.join("requirements.txt").is_file()
    {
        let program = if cfg!(windows) {
            "python.exe"
        } else {
            "python3"
        };
        commands.push(VerificationCommand {
            label: "Python syntax",
            purpose: "Compile Python source to detect syntax errors",
            risk: "low (writes local bytecode caches)",
            program: program.to_string(),
            args: vec!["-m".into(), "compileall".into(), ".".into()],
        });
    }
    commands
}

fn print_command_approval(ctx: &AppContext, root: &Path, command: &VerificationCommand) {
    println!();
    println!("  {}", ctx.ui.header(command.label));
    println!("  Command : {}", command.display());
    println!("  CWD     : {}", root.display());
    println!("  Purpose : {}", command.purpose);
    println!("  Risk    : {}", command.risk);
    println!("  Timeout : 10 minutes");
}

fn run_verification_command(
    ctx: &AppContext,
    root: &Path,
    command: &VerificationCommand,
) -> AppResult<()> {
    println!();
    println!(
        "{} {} {}",
        ctx.ui.info("Running"),
        command.program,
        command.args.join(" ")
    );
    let mut child = Command::new(&command.program)
        .args(&command.args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| format!("Could not start {}: {error}", command.label))?;
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| format!("Could not monitor {}: {error}", command.label))?
        {
            if status.success() {
                break;
            }
            return Err(format!("{} failed with status {status}.", command.label));
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "{} exceeded the 10 minute safety timeout.",
                command.label
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    println!("{}", ctx.ui.success(&format!("{} passed.", command.label)));
    Ok(())
}

fn git_root(start: &Path) -> Option<PathBuf> {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(start)
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| PathBuf::from(String::from_utf8_lossy(&output.stdout).trim()))
}

fn git_text(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-c")
        .arg("safe.directory=*")
        .args(args)
        .current_dir(root)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_string();
    (!value.trim().is_empty()).then_some(value)
}

fn git_status_files(root: &Path) -> Vec<String> {
    git_text(root, &["status", "--porcelain=v1", "-z"])
        .map(|status| {
            status
                .split('\0')
                .filter_map(|entry| {
                    let path = entry.get(3..)?.trim();
                    if path.is_empty() {
                        None
                    } else {
                        Some(path.split(" -> ").last().unwrap_or(path).to_string())
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn scan_files(
    root: &Path,
    count: &mut usize,
    truncated: &mut bool,
    extensions: &mut BTreeMap<String, usize>,
) -> AppResult<()> {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("Could not scan `{}`: {error}", directory.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(should_skip_dir)
                {
                    continue;
                }
                pending.push(path);
            } else if path.is_file() {
                *count += 1;
                if let Some(extension) = path.extension().and_then(|value| value.to_str()) {
                    *extensions
                        .entry(extension.to_ascii_lowercase())
                        .or_default() += 1;
                }
                if *count >= MAX_DISCOVERY_FILES {
                    *truncated = true;
                    return Ok(());
                }
            }
        }
    }
    Ok(())
}

fn language_name(extension: &str) -> Option<&'static str> {
    Some(match extension {
        "rs" => "Rust",
        "ts" | "tsx" => "TypeScript",
        "js" | "jsx" => "JavaScript",
        "py" => "Python",
        "go" => "Go",
        "java" => "Java",
        "kt" | "kts" => "Kotlin",
        "cs" => "C#",
        "cpp" | "cc" | "cxx" | "hpp" => "C++",
        "c" | "h" => "C",
        "rb" => "Ruby",
        "php" => "PHP",
        "swift" => "Swift",
        "vue" => "Vue",
        "svelte" => "Svelte",
        "html" | "css" | "scss" => "Web",
        _ => return None,
    })
}

fn detect_build_systems(root: &Path) -> Vec<String> {
    let markers = [
        ("Cargo.toml", "Cargo"),
        ("package.json", "Node.js"),
        ("pyproject.toml", "Python"),
        ("requirements.txt", "Python requirements"),
        ("go.mod", "Go modules"),
        ("pom.xml", "Maven"),
        ("build.gradle", "Gradle"),
        ("CMakeLists.txt", "CMake"),
        ("Makefile", "Make"),
    ];
    let mut detected = BTreeSet::new();
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    while let Some((directory, depth)) = pending.pop() {
        for (file, label) in markers {
            if directory.join(file).is_file() {
                detected.insert(label.to_string());
            }
        }
        if depth >= 2 {
            continue;
        }
        if let Ok(entries) = fs::read_dir(&directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir()
                    && !path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .is_some_and(should_skip_dir)
                {
                    pending.push((path, depth + 1));
                }
            }
        }
    }
    detected.into_iter().collect()
}

fn detect_package_managers(root: &Path) -> Vec<String> {
    let markers = [
        ("Cargo.lock", "Cargo"),
        ("pnpm-lock.yaml", "pnpm"),
        ("yarn.lock", "Yarn"),
        ("package-lock.json", "npm"),
        ("bun.lock", "Bun"),
        ("bun.lockb", "Bun"),
        ("poetry.lock", "Poetry"),
        ("uv.lock", "uv"),
        ("Pipfile.lock", "Pipenv"),
        ("go.sum", "Go modules"),
        ("gradlew", "Gradle wrapper"),
        ("mvnw", "Maven wrapper"),
    ];
    detect_markers(root, &markers)
}

fn detect_frameworks(root: &Path) -> Vec<String> {
    let mut detected = BTreeSet::new();
    let signatures = [
        ("react", "React"),
        ("next", "Next.js"),
        ("vite", "Vite"),
        ("vue", "Vue"),
        ("svelte", "Svelte"),
        ("@angular/core", "Angular"),
        ("express", "Express"),
        ("fastapi", "FastAPI"),
        ("django", "Django"),
        ("flask", "Flask"),
        ("axum", "Axum"),
        ("actix-web", "Actix Web"),
        ("tokio", "Tokio"),
    ];
    for manifest in collect_named_files(
        root,
        &[
            "package.json",
            "Cargo.toml",
            "pyproject.toml",
            "requirements.txt",
        ],
        3,
    ) {
        if let Ok(content) = fs::read_to_string(manifest) {
            let lower = content.to_ascii_lowercase();
            for (signature, label) in signatures {
                if lower.contains(signature) {
                    detected.insert(label.to_string());
                }
            }
        }
    }
    detected.into_iter().collect()
}

fn detect_markers(root: &Path, markers: &[(&str, &str)]) -> Vec<String> {
    let names = markers.iter().map(|(name, _)| *name).collect::<Vec<_>>();
    let found = collect_named_files(root, &names, 3);
    let mut detected = BTreeSet::new();
    for path in found {
        if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
            for (marker, label) in markers {
                if name.eq_ignore_ascii_case(marker) {
                    detected.insert((*label).to_string());
                }
            }
        }
    }
    detected.into_iter().collect()
}

fn collect_named_files(root: &Path, names: &[&str], max_depth: usize) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    while let Some((directory, depth)) = pending.pop() {
        if let Ok(entries) = fs::read_dir(&directory) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(|name| {
                            names
                                .iter()
                                .any(|candidate| name.eq_ignore_ascii_case(candidate))
                        })
                {
                    found.push(path);
                } else if path.is_dir()
                    && depth < max_depth
                    && !path
                        .file_name()
                        .and_then(|value| value.to_str())
                        .is_some_and(should_skip_dir)
                {
                    pending.push((path, depth + 1));
                }
            }
        }
    }
    found
}

fn affected_test_commands(
    repository: &RepositoryContext,
    changed: &[String],
) -> Vec<VerificationCommand> {
    let mut commands = Vec::new();
    let mut cargo_manifests = BTreeSet::new();
    for path in changed.iter().filter(|path| path.ends_with(".rs")) {
        if let Some(manifest) = nearest_manifest(&repository.root, path, "Cargo.toml") {
            cargo_manifests.insert(manifest);
        }
    }
    for manifest in cargo_manifests {
        commands.push(VerificationCommand {
            label: "Affected Rust tests",
            purpose: "Run tests for the nearest changed Rust crate",
            risk: "medium (project tests and build scripts execute)",
            program: "cargo".into(),
            args: vec![
                "test".into(),
                "--manifest-path".into(),
                manifest
                    .strip_prefix(&repository.root)
                    .unwrap_or(&manifest)
                    .display()
                    .to_string(),
            ],
        });
    }

    let node_changed = changed
        .iter()
        .filter(|path| {
            matches!(
                Path::new(path).extension().and_then(|value| value.to_str()),
                Some("ts" | "tsx" | "js" | "jsx")
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let node_tests = inferred_test_files(&repository.root, &node_changed, &["test", "spec"]);
    let mut package_dirs = BTreeSet::new();
    for path in &node_changed {
        if let Some(manifest) = nearest_manifest(&repository.root, path, "package.json") {
            package_dirs.insert(manifest.parent().unwrap_or(&repository.root).to_path_buf());
        }
    }
    for package_dir in package_dirs {
        let relative = package_dir
            .strip_prefix(&repository.root)
            .unwrap_or(&package_dir)
            .display()
            .to_string();
        let mut args = if relative.is_empty() {
            vec!["test".into()]
        } else {
            vec!["--prefix".into(), relative, "test".into()]
        };
        let scoped = node_tests
            .iter()
            .filter(|path| repository.root.join(path).starts_with(&package_dir))
            .cloned()
            .collect::<Vec<_>>();
        if !scoped.is_empty() {
            args.extend(["--".into(), "--run".into()]);
            args.extend(scoped);
        }
        commands.push(VerificationCommand {
            label: "Affected Node tests",
            purpose: "Run the nearest package tests, targeting inferred test files when available",
            risk: "medium (package scripts execute project code)",
            program: if cfg!(windows) {
                "npm.cmd".into()
            } else {
                "npm".into()
            },
            args,
        });
    }

    let python_changed = changed
        .iter()
        .filter(|path| path.ends_with(".py"))
        .cloned()
        .collect::<Vec<_>>();
    if !python_changed.is_empty() {
        let mut args = vec!["-m".into(), "pytest".into()];
        args.extend(inferred_python_tests(&repository.root, &python_changed));
        commands.push(VerificationCommand {
            label: "Affected Python tests",
            purpose: "Run inferred Python tests, or the suite when no direct test is found",
            risk: "medium (project tests execute)",
            program: if cfg!(windows) {
                "python.exe".into()
            } else {
                "python3".into()
            },
            args,
        });
    }
    commands
}

fn nearest_manifest(root: &Path, changed: &str, manifest: &str) -> Option<PathBuf> {
    let mut directory = root.join(changed).parent()?.to_path_buf();
    loop {
        let candidate = directory.join(manifest);
        if candidate.is_file() {
            return Some(candidate);
        }
        if directory == root || !directory.pop() {
            return None;
        }
    }
}

fn inferred_test_files(root: &Path, changed: &[String], suffixes: &[&str]) -> Vec<String> {
    let mut tests = BTreeSet::new();
    for relative in changed {
        let path = root.join(relative);
        let (Some(stem), Some(extension)) = (
            path.file_stem().and_then(|value| value.to_str()),
            path.extension().and_then(|value| value.to_str()),
        ) else {
            continue;
        };
        for suffix in suffixes {
            let candidate = path
                .parent()
                .unwrap_or(root)
                .join(format!("{stem}.{suffix}.{extension}"));
            if candidate.is_file() {
                tests.insert(
                    candidate
                        .strip_prefix(root)
                        .unwrap_or(&candidate)
                        .display()
                        .to_string(),
                );
            }
        }
    }
    tests.into_iter().collect()
}

fn inferred_python_tests(root: &Path, changed: &[String]) -> Vec<String> {
    let mut tests = BTreeSet::new();
    for relative in changed {
        let path = root.join(relative);
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        for candidate in [
            path.parent()
                .unwrap_or(root)
                .join(format!("test_{stem}.py")),
            path.parent()
                .unwrap_or(root)
                .join(format!("{stem}_test.py")),
            root.join("tests").join(format!("test_{stem}.py")),
        ] {
            if candidate.is_file() {
                tests.insert(
                    candidate
                        .strip_prefix(root)
                        .unwrap_or(&candidate)
                        .display()
                        .to_string(),
                );
            }
        }
    }
    tests.into_iter().collect()
}

fn should_skip_dir(name: &str) -> bool {
    SKIPPED_DIRS
        .iter()
        .any(|skip| name.eq_ignore_ascii_case(skip))
        || name.starts_with("target-")
        || name.starts_with(".cargo-target")
        || matches!(name, ".test-dist" | ".next" | ".nuxt" | "coverage")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_fenced_diff() {
        let result = extract_unified_diff("text\n```diff\ndiff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-a\n+b\n```\n").unwrap();
        assert!(result.starts_with("diff --git"));
        assert!(!result.contains("```"));
    }

    #[test]
    fn rejects_parent_directory_patch_paths() {
        let patch = "diff --git a/../secret b/../secret\n--- a/../secret\n+++ b/../secret\n";
        assert!(validate_patch_paths(patch).is_err());
    }

    #[test]
    fn accepts_workspace_relative_patch_paths() {
        let patch =
            "diff --git a/src/main.rs b/src/main.rs\n--- a/src/main.rs\n+++ b/src/main.rs\n";
        assert!(validate_patch_paths(patch).is_ok());
    }

    #[test]
    fn splits_multi_file_patches_for_individual_review() {
        let patch = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/b.rs b/b.rs\n--- a/b.rs\n+++ b/b.rs\n@@ -1 +1 @@\n-c\n+d\n";
        let chunks = split_patch(patch);
        assert_eq!(chunks.len(), 2);
        assert_eq!(patch_files(patch), vec!["a.rs", "b.rs"]);
    }

    #[test]
    fn rejects_git_metadata_patch_paths() {
        let patch =
            "diff --git a/.git/config b/.git/config\n--- a/.git/config\n+++ b/.git/config\n";
        assert!(validate_patch_paths(patch).is_err());
    }

    #[test]
    fn recognizes_source_extensions_only_for_fast_find() {
        assert!(is_searchable_file(Path::new("src/main.rs")));
        assert!(!is_searchable_file(Path::new("image.png")));
    }
}
