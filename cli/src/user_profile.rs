//! Role: local personalization note storage shared by the CLI and engine via a text file path convention.
//! Called by: `commands.rs` when the user runs `save "<note>"`.
//! Calls into: the local filesystem only.
//! Owns: resolving the note file path and appending new user notes.
//! Does not own: prompt construction, engine inference, or UI formatting.
//! Next TODOs: add view/edit/remove commands and deduplicate repeated note entries.

use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use crate::AppResult;

pub fn append_note(note: &str) -> AppResult<PathBuf> {
    let note = note.trim();
    if note.is_empty() {
        return Err("Cannot save an empty personalization note.".to_string());
    }

    let path = profile_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create the profile note directory: {error}"))?;
    }

    let needs_separator = fs::metadata(&path)
        .map(|metadata| metadata.len() > 0)
        .unwrap_or(false);

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("Could not open the profile note file: {error}"))?;

    if needs_separator {
        writeln!(file)
            .map_err(|error| format!("Could not separate the new profile note entry: {error}"))?;
    }

    writeln!(file, "{note}")
        .map_err(|error| format!("Could not write the profile note: {error}"))?;

    Ok(path)
}

pub fn profile_file_path() -> PathBuf {
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
