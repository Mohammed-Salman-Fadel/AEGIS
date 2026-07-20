use std::collections::{BTreeMap, HashMap, HashSet, hash_map::DefaultHasher};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::AppResult;
use crate::command_line::{CommandLinePolicy, state_root};

const INDEX_VERSION: u32 = 1;
const MAX_INDEX_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 1_000_000;
const DEFAULT_CONTEXT_BUDGET: usize = 14_000;
const EXPANDED_CONTEXT_BUDGET: usize = 36_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStage {
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub id: String,
    pub task: String,
    pub repository: String,
    pub updated_at: u64,
    pub stages: Vec<PlanStage>,
}

impl TaskPlan {
    pub fn start(root: &Path, task: &str, enabled: bool) -> AppResult<Option<Self>> {
        if !enabled {
            return Ok(None);
        }
        let path = plan_path(root)?;
        let id = stable_key(&(root.display().to_string() + task));
        if let Ok(raw) = fs::read_to_string(&path)
            && let Ok(existing) = serde_json::from_str::<Self>(&raw)
            && existing.id == id
            && existing
                .stages
                .iter()
                .any(|stage| stage.status != "completed")
        {
            return Ok(Some(existing));
        }
        let plan = Self {
            id,
            task: task.trim().to_string(),
            repository: root.display().to_string(),
            updated_at: now_epoch(),
            stages: [
                "Understand",
                "Explore",
                "Plan",
                "Request permission",
                "Edit",
                "Format",
                "Test",
                "Review",
            ]
            .into_iter()
            .map(|name| PlanStage {
                name: name.to_string(),
                status: "pending".to_string(),
            })
            .collect(),
        };
        plan.save(root)?;
        Ok(Some(plan))
    }

    pub fn advance(&mut self, root: &Path, stage: &str) -> AppResult<()> {
        let Some(index) = self.stages.iter().position(|item| item.name == stage) else {
            return Ok(());
        };
        for (position, item) in self.stages.iter_mut().enumerate() {
            if position < index {
                item.status = "completed".to_string();
            } else if position == index {
                item.status = "in_progress".to_string();
            } else if item.status != "completed" {
                item.status = "pending".to_string();
            }
        }
        self.updated_at = now_epoch();
        self.save(root)
    }

    pub fn complete(&mut self, root: &Path) -> AppResult<()> {
        for stage in &mut self.stages {
            stage.status = "completed".to_string();
        }
        self.updated_at = now_epoch();
        self.save(root)
    }

    pub fn summary(&self) -> String {
        self.stages
            .iter()
            .map(|stage| {
                let marker = match stage.status.as_str() {
                    "completed" => "done",
                    "in_progress" => "active",
                    _ => "pending",
                };
                format!("{} [{}]", stage.name, marker)
            })
            .collect::<Vec<_>>()
            .join(" -> ")
    }

    fn save(&self, root: &Path) -> AppResult<()> {
        let path = plan_path(root)?;
        write_json(&path, self)
    }
}

pub fn current_plan(root: &Path) -> AppResult<Option<TaskPlan>> {
    let path = plan_path(root)?;
    let Ok(raw) = fs::read_to_string(&path) else {
        return Ok(None);
    };
    serde_json::from_str(&raw).map(Some).map_err(|error| {
        format!(
            "Could not read persisted task plan `{}`: {error}",
            path.display()
        )
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct IndexEntry {
    path: String,
    modified_at: u64,
    size: u64,
    kind: String,
    symbols: Vec<String>,
    preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RepositoryIndex {
    version: u32,
    repository: String,
    generated_at: u64,
    git_history: String,
    entries: Vec<IndexEntry>,
}

pub struct PreparedRepositoryContext {
    pub instructions: String,
    pub ranked_context: String,
    pub indexed_files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SemanticHit {
    pub path: String,
    pub score: usize,
    pub symbols: Vec<String>,
    pub preview: String,
}

pub fn semantic_search(root: &Path, query: &str, limit: usize) -> AppResult<Vec<SemanticHit>> {
    let index = build_or_refresh_index(root)?;
    let terms = query_terms(query);
    let mut hits = index
        .entries
        .into_iter()
        .map(|entry| {
            let score = relevance(&entry, &terms);
            (score, entry)
        })
        .filter(|(score, _)| *score > 0)
        .collect::<Vec<_>>();
    hits.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.path.cmp(&right.1.path)));
    Ok(hits
        .into_iter()
        .take(limit)
        .map(|(score, entry)| SemanticHit {
            path: entry.path,
            score,
            symbols: entry.symbols,
            preview: entry.preview,
        })
        .collect())
}

pub fn prepare_repository_context(
    root: &Path,
    instruction_files: &[PathBuf],
    task: &str,
    policy: &CommandLinePolicy,
) -> AppResult<PreparedRepositoryContext> {
    let instructions = if policy.repository_instructions {
        read_instructions(root, instruction_files)?
    } else {
        String::new()
    };
    if !policy.semantic_index {
        return Ok(PreparedRepositoryContext {
            instructions,
            ranked_context: String::new(),
            indexed_files: 0,
        });
    }

    let index = build_or_refresh_index(root)?;
    let budget = if policy.context_budgeting {
        DEFAULT_CONTEXT_BUDGET
    } else {
        EXPANDED_CONTEXT_BUDGET
    };
    let ranked_context = rank_context(root, &index, task, budget);
    Ok(PreparedRepositoryContext {
        instructions,
        ranked_context,
        indexed_files: index.entries.len(),
    })
}

fn read_instructions(root: &Path, files: &[PathBuf]) -> AppResult<String> {
    let mut output = String::new();
    for path in files {
        let content = fs::read_to_string(path).map_err(|error| {
            format!(
                "Could not read repository instruction `{}`: {error}",
                path.display()
            )
        })?;
        let relative = path.strip_prefix(root).unwrap_or(path);
        output.push_str(&format!("\n### {}\n", relative.display()));
        output.extend(content.chars().take(16_000));
        output.push('\n');
        if output.len() >= 40_000 {
            output.truncate(40_000);
            output.push_str("\n[Repository instructions compacted at 40,000 characters.]\n");
            break;
        }
    }
    Ok(output)
}

fn build_or_refresh_index(root: &Path) -> AppResult<RepositoryIndex> {
    let path = repository_state_dir(root)?.join("repository-index.json");
    let previous = fs::read_to_string(&path)
        .ok()
        .and_then(|raw| serde_json::from_str::<RepositoryIndex>(&raw).ok())
        .filter(|index| index.version == INDEX_VERSION);
    let cached = previous
        .map(|index| {
            index
                .entries
                .into_iter()
                .map(|entry| (entry.path.clone(), entry))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let mut entries = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(&directory)
            .map_err(|error| format!("Could not index `{}`: {error}", directory.display()))?
            .flatten()
        {
            let file_path = entry.path();
            if file_path.is_dir() {
                if !file_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(skip_directory)
                {
                    pending.push(file_path);
                }
                continue;
            }
            if entries.len() >= MAX_INDEX_FILES || !indexable_file(&file_path) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.len() > MAX_FILE_BYTES {
                continue;
            }
            let relative = file_path
                .strip_prefix(root)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .replace('\\', "/");
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|value| value.as_secs())
                .unwrap_or(0);
            if let Some(cached_entry) = cached.get(&relative)
                && cached_entry.modified_at == modified_at
                && cached_entry.size == metadata.len()
            {
                entries.push(cached_entry.clone());
                continue;
            }
            let Ok(content) = fs::read_to_string(&file_path) else {
                continue;
            };
            entries.push(IndexEntry {
                path: relative.clone(),
                modified_at,
                size: metadata.len(),
                kind: classify_path(&relative),
                symbols: extract_symbols(&content),
                preview: compact_preview(&content),
            });
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    let index = RepositoryIndex {
        version: INDEX_VERSION,
        repository: root.display().to_string(),
        generated_at: now_epoch(),
        git_history: recent_git_history(root),
        entries,
    };
    write_json(&path, &index)?;
    Ok(index)
}

fn rank_context(root: &Path, index: &RepositoryIndex, task: &str, budget: usize) -> String {
    let terms = query_terms(task);
    let mut ranked = index
        .entries
        .iter()
        .map(|entry| (relevance(entry, &terms), entry))
        .filter(|(score, _)| *score > 0)
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then(left.1.path.cmp(&right.1.path)));

    let mut output = String::new();
    if !index.git_history.is_empty() {
        output.push_str("Recent Git history:\n");
        output.push_str(&index.git_history);
        output.push_str("\n\n");
    }
    for (score, entry) in ranked.into_iter().take(18) {
        if output.len() >= budget {
            break;
        }
        let path = root.join(&entry.path);
        let content = fs::read_to_string(&path).unwrap_or_default();
        let remaining = budget.saturating_sub(output.len());
        if remaining < 180 {
            break;
        }
        let excerpt_limit = remaining.min(2_800);
        let excerpt = content.chars().take(excerpt_limit).collect::<String>();
        output.push_str(&format!(
            "### {} (relevance {}, {}, symbols: {})\n{}\n\n",
            entry.path,
            score,
            entry.kind,
            if entry.symbols.is_empty() {
                "none".to_string()
            } else {
                entry
                    .symbols
                    .iter()
                    .take(12)
                    .cloned()
                    .collect::<Vec<_>>()
                    .join(", ")
            },
            excerpt
        ));
    }
    if output.len() >= budget {
        output.truncate(budget);
        output.push_str("\n[Repository context compacted to the configured budget.]\n");
    }
    output
}

fn relevance(entry: &IndexEntry, terms: &[String]) -> usize {
    if terms.is_empty() {
        return usize::from(matches!(
            entry.kind.as_str(),
            "instruction" | "documentation" | "config"
        ));
    }
    let path = entry.path.to_ascii_lowercase();
    let symbols = entry.symbols.join(" ").to_ascii_lowercase();
    let preview = entry.preview.to_ascii_lowercase();
    terms
        .iter()
        .map(|term| {
            usize::from(path.contains(term)) * 10
                + usize::from(symbols.contains(term)) * 7
                + usize::from(preview.contains(term)) * 2
        })
        .sum::<usize>()
        + usize::from(entry.kind == "instruction") * 2
}

fn query_terms(query: &str) -> Vec<String> {
    let stop = [
        "the", "and", "for", "with", "that", "this", "from", "into", "fix", "add",
    ];
    query
        .split(|character: char| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
        .map(str::to_ascii_lowercase)
        .filter(|term| term.len() >= 3 && !stop.contains(&term.as_str()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn extract_symbols(content: &str) -> Vec<String> {
    let prefixes = [
        "fn ",
        "pub fn ",
        "async fn ",
        "pub async fn ",
        "struct ",
        "pub struct ",
        "enum ",
        "pub enum ",
        "trait ",
        "impl ",
        "class ",
        "def ",
        "function ",
        "interface ",
        "type ",
        "const ",
        "export function ",
        "export const ",
    ];
    let mut symbols = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        if let Some(prefix) = prefixes.iter().find(|prefix| trimmed.starts_with(**prefix)) {
            let symbol = trimmed[prefix.len()..]
                .split(|character: char| {
                    character.is_whitespace() || matches!(character, '(' | '{' | '<' | ':' | '=')
                })
                .next()
                .unwrap_or("")
                .trim_matches('&');
            if !symbol.is_empty() && !symbols.iter().any(|existing| existing == symbol) {
                symbols.push(symbol.to_string());
            }
        }
        if symbols.len() >= 80 {
            break;
        }
    }
    symbols
}

fn compact_preview(content: &str) -> String {
    content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(16)
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(1_200)
        .collect()
}

fn classify_path(path: &str) -> String {
    let lower = path.to_ascii_lowercase();
    if lower.ends_with("agents.md")
        || lower.ends_with("contributing.md")
        || lower.ends_with(".aegis.md")
    {
        "instruction"
    } else if lower.ends_with(".md") || lower.contains("/docs/") || lower.starts_with("docs/") {
        "documentation"
    } else if lower.contains("test") || lower.contains("spec") {
        "test"
    } else if [".toml", ".json", ".yaml", ".yml", ".xml"]
        .iter()
        .any(|suffix| lower.ends_with(suffix))
    {
        "config"
    } else {
        "source"
    }
    .to_string()
}

fn recent_git_history(root: &Path) -> String {
    if !root.join(".git").exists() {
        return String::new();
    }
    Command::new("git")
        .arg("-c")
        .arg("safe.directory=*")
        .args(["log", "--oneline", "--name-only", "-12"])
        .current_dir(root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| {
            String::from_utf8_lossy(&output.stdout)
                .chars()
                .take(5_000)
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointFile {
    path: String,
    existed: bool,
    post_edit_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointManifest {
    pub id: String,
    pub repository: String,
    pub task: String,
    pub created_at: u64,
    pub files: Vec<CheckpointFile>,
}

pub fn create_checkpoint(
    root: &Path,
    task: &str,
    files: &[String],
) -> AppResult<CheckpointManifest> {
    let id = format!("{}-{}", now_epoch(), std::process::id());
    let directory = checkpoint_root(root)?.join(&id);
    let snapshot_root = directory.join("files");
    fs::create_dir_all(&snapshot_root).map_err(|error| {
        format!(
            "Could not create checkpoint `{}`: {error}",
            directory.display()
        )
    })?;
    let mut snapshots = Vec::new();
    for relative in files {
        let source = root.join(relative);
        let existed = source.is_file();
        if existed {
            let destination = snapshot_root.join(relative);
            if let Some(parent) = destination.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(&source, &destination)
                .map_err(|error| format!("Could not snapshot `{}`: {error}", source.display()))?;
        }
        snapshots.push(CheckpointFile {
            path: relative.clone(),
            existed,
            post_edit_hash: None,
        });
    }
    let manifest = CheckpointManifest {
        id,
        repository: root.display().to_string(),
        task: task.trim().to_string(),
        created_at: now_epoch(),
        files: snapshots,
    };
    write_json(&directory.join("manifest.json"), &manifest)?;
    Ok(manifest)
}

pub fn finalize_checkpoint(root: &Path, manifest: &mut CheckpointManifest) -> AppResult<()> {
    for file in &mut manifest.files {
        file.post_edit_hash = file_hash(&root.join(&file.path));
    }
    write_json(
        &checkpoint_root(root)?
            .join(&manifest.id)
            .join("manifest.json"),
        manifest,
    )
}

pub fn list_checkpoints(root: &Path) -> AppResult<Vec<CheckpointManifest>> {
    let directory = checkpoint_root(root)?;
    if !directory.is_dir() {
        return Ok(Vec::new());
    }
    let mut manifests = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .flatten()
        .filter_map(|entry| fs::read_to_string(entry.path().join("manifest.json")).ok())
        .filter_map(|raw| serde_json::from_str::<CheckpointManifest>(&raw).ok())
        .collect::<Vec<_>>();
    manifests.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    Ok(manifests)
}

pub fn restore_checkpoint(root: &Path, id: &str) -> AppResult<Vec<String>> {
    let directory = checkpoint_root(root)?.join(id);
    let manifest: CheckpointManifest = serde_json::from_str(
        &fs::read_to_string(directory.join("manifest.json"))
            .map_err(|error| format!("Checkpoint `{id}` was not found: {error}"))?,
    )
    .map_err(|error| format!("Checkpoint `{id}` is invalid: {error}"))?;
    for file in &manifest.files {
        if file_hash(&root.join(&file.path)) != file.post_edit_hash {
            return Err(format!(
                "Restore refused because `{}` changed after checkpoint `{id}`. Preserve or commit that work before restoring.",
                file.path
            ));
        }
    }
    let mut restored = Vec::new();
    for file in &manifest.files {
        let target = root.join(&file.path);
        if file.existed {
            let snapshot = directory.join("files").join(&file.path);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| error.to_string())?;
            }
            fs::copy(&snapshot, &target)
                .map_err(|error| format!("Could not restore `{}`: {error}", target.display()))?;
        } else if target.exists() {
            fs::remove_file(&target)
                .map_err(|error| format!("Could not remove `{}`: {error}", target.display()))?;
        }
        restored.push(file.path.clone());
    }
    Ok(restored)
}

fn file_hash(path: &Path) -> Option<String> {
    let bytes = fs::read(path).ok()?;
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}

fn indexable_file(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    [
        "rs", "ts", "tsx", "js", "jsx", "py", "go", "java", "kt", "cs", "c", "h", "cpp", "hpp",
        "rb", "php", "swift", "vue", "svelte", "html", "css", "scss", "json", "toml", "yaml",
        "yml", "md", "txt", "xml", "sql", "sh", "ps1",
    ]
    .iter()
    .any(|candidate| extension.eq_ignore_ascii_case(candidate))
}

fn skip_directory(name: &str) -> bool {
    [
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        ".venv",
        "venv",
        "__pycache__",
        ".aegis",
    ]
    .iter()
    .any(|candidate| name.eq_ignore_ascii_case(candidate))
}

fn repository_state_dir(root: &Path) -> AppResult<PathBuf> {
    let directory = state_root()?
        .join("repositories")
        .join(stable_key(&root.display().to_string()));
    fs::create_dir_all(&directory).map_err(|error| {
        format!(
            "Could not create repository state `{}`: {error}",
            directory.display()
        )
    })?;
    Ok(directory)
}

fn checkpoint_root(root: &Path) -> AppResult<PathBuf> {
    Ok(repository_state_dir(root)?.join("checkpoints"))
}

fn plan_path(root: &Path) -> AppResult<PathBuf> {
    Ok(repository_state_dir(root)?.join("task-plan.json"))
}

fn stable_key(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.to_ascii_lowercase().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn write_json(path: &Path, value: &impl Serialize) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let content = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    fs::write(path, content)
        .map_err(|error| format!("Could not write `{}`: {error}", path.display()))
}

pub fn capability_summary(policy: &CommandLinePolicy) -> BTreeMap<&'static str, bool> {
    BTreeMap::from([
        ("agentic_loop", policy.agentic_loop),
        ("repository_instructions", policy.repository_instructions),
        ("semantic_index", policy.semantic_index),
        ("persistent_task_plan", policy.persistent_task_plan),
        ("task_checkpoints", policy.task_checkpoints),
        ("context_budgeting", policy.context_budgeting),
        ("patch_application", policy.patch_application),
        ("command_execution", policy.command_execution),
        ("automatic_verification", policy.automatic_verification),
        ("deep_reasoning", policy.deep_reasoning),
        ("git_safety", policy.git_safety),
    ])
}
