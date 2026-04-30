//! Role: loads locally saved personalization notes and wraps prompts with them when available.
//! Called by: `orchestrator/mod.rs` before sending prompts to the inference backend.
//! Calls into: the local filesystem only.
//! Owns: resolving the shared note file path and preparing a safe personalization preamble.
//! Does not own: note authoring, session persistence, or inference transport.
//! Next TODOs: add structured parsing and preference weighting instead of plain-text concatenation.

use std::env;
use std::fs;
use std::path::PathBuf;

pub fn personalize_prompt(prompt: &str) -> String {
    let Some(notes) = load_notes() else {
        return prompt.to_string();
    };

    format!(
        "You are AEGIS, a private local-only AI assistant.\n\n\
        The user has explicitly saved the following persistent profile notes. Treat them as trusted user-provided facts for personalization.\n\
        When the user asks what you know about them, answer from these notes instead of claiming you do not know anything.\n\
        If the saved notes are unrelated to the current request, do not force them into the answer.\n\n\
        PERSISTENT USER PROFILE NOTES:\n{notes}\n\n\
        USER REQUEST CONTEXT:\n{prompt}"
    )
}

fn load_notes() -> Option<String> {
    let path = profile_file_path();
    let contents = fs::read_to_string(path).ok()?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        return None;
    }

    Some(trimmed.chars().take(4000).collect())
}

fn profile_file_path() -> PathBuf {
    env::var("AEGIS_PROFILE_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(default_profile_file_path)
}

fn default_profile_file_path() -> PathBuf {
    resolve_home_dir()
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
        .join(".aegis")
        .join("user_notes.txt")
}

fn resolve_home_dir() -> Option<PathBuf> {
    env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(PathBuf::from))
}
