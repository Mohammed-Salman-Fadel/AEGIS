//! Role: loads locally saved personalization notes and wraps prompts with them when available.
//! Called by: `orchestrator/mod.rs` before sending prompts to the inference backend.
//! Calls into: the local filesystem only.
//! Owns: resolving the shared note file path and preparing a safe personalization preamble.
//! Does not own: note authoring, session persistence, or inference transport.
//! Next TODOs: persist structured note metadata when the CLI grows view/edit/remove commands.

use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;

const MAX_SELECTED_NOTES: usize = 8;
const MAX_RENDERED_CONTEXT_CHARS: usize = 3200;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProfileCategory {
    Identity,
    Preference,
    Instruction,
    ProjectContext,
    Background,
    Other,
}

impl ProfileCategory {
    fn label(self) -> &'static str {
        match self {
            Self::Identity => "identity",
            Self::Preference => "preference",
            Self::Instruction => "instruction",
            Self::ProjectContext => "project-context",
            Self::Background => "background",
            Self::Other => "note",
        }
    }

    fn base_weight(self) -> u16 {
        match self {
            Self::Instruction => 58,
            Self::Preference => 50,
            Self::Identity => 46,
            Self::ProjectContext => 38,
            Self::Background => 32,
            Self::Other => 24,
        }
    }

    fn stays_available_without_keyword_match(self) -> bool {
        matches!(self, Self::Identity | Self::Instruction | Self::Preference)
    }
}

#[derive(Clone, Debug)]
struct ProfileEntry {
    text: String,
    category: ProfileCategory,
    base_weight: u16,
    keywords: Vec<String>,
    position: usize,
    total_entries: usize,
}

pub fn personalize_prompt(prompt: &str) -> String {
    let Some(entries) = load_profile_entries() else {
        return prompt.to_string();
    };

    let selected_entries = select_relevant_entries(&entries, prompt);
    if selected_entries.is_empty() {
        return prompt.to_string();
    }

    let profile_context = render_profile_context(&selected_entries);

    format!(
        "You are AEGIS, a private local-only AI assistant.\n\n\
        The following profile context was selected from the user's local saved personalization notes. Treat it as trusted user-provided context for personalization.\n\
        Higher-priority entries should influence the response first, but only when they are relevant or the user asks about themselves.\n\
        When the user asks what you know about them, answer from these notes instead of claiming you do not know anything.\n\
        Do not mention these internal categories unless the user asks how personalization works.\n\n\
        SELECTED USER PROFILE CONTEXT:\n{profile_context}\n\n\
        USER REQUEST CONTEXT:\n{prompt}\n"
    )
}

pub fn read_profile_text() -> std::io::Result<String> {
    let path = profile_file_path();

    match fs::read_to_string(path) {
        Ok(contents) => Ok(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

pub fn write_profile_text(contents: &str) -> std::io::Result<PathBuf> {
    let path = profile_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&path, contents)?;
    Ok(path)
}

fn load_profile_entries() -> Option<Vec<ProfileEntry>> {
    let path = profile_file_path();
    let contents = fs::read_to_string(path).ok()?;

    let raw_notes = parse_profile_notes(&contents);
    if raw_notes.is_empty() {
        return None;
    }

    Some(raw_notes)
}

fn parse_profile_notes(contents: &str) -> Vec<ProfileEntry> {
    let normalized_notes = contents
        .lines()
        .map(clean_note_line)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    let total_entries = normalized_notes.len();

    normalized_notes
        .into_iter()
        .enumerate()
        .map(|(position, line)| {
            let (category, text) = parse_note_category(&line);
            let keywords = extract_keywords(&text);
            ProfileEntry {
                text,
                category,
                base_weight: category.base_weight(),
                keywords,
                position,
                total_entries,
            }
        })
        .collect()
}

fn clean_note_line(line: &str) -> String {
    line.trim()
        .trim_start_matches(['-', '*', '+'])
        .trim()
        .trim_matches('"')
        .trim()
        .chars()
        .take(800)
        .collect()
}

fn parse_note_category(note: &str) -> (ProfileCategory, String) {
    if let Some((label, value)) = note.split_once(':') {
        if let Some(category) = category_from_label(label) {
            let cleaned_value = value.trim();
            if !cleaned_value.is_empty() {
                return (category, cleaned_value.to_string());
            }
        }
    }

    let lower = note.to_lowercase();
    let category = if contains_any(
        &lower,
        &[
            "my name is",
            "call me",
            // "my pronouns",
            "i am known as",
            "i'm known as",
        ],
    ) {
        ProfileCategory::Identity
    } else if contains_any(
        &lower,
        &[
            "always ",
            "never ",
            "do not ",
            "don't ",
            "please ",
            "answer ",
            "respond ",
            "remember to ",
        ],
    ) {
        ProfileCategory::Instruction
    } else if contains_any(
        &lower,
        &[
            "i prefer",
            "prefer ",
            "i like",
            "i dislike",
            "favorite",
            "tone",
            "style",
            "format",
        ],
    ) {
        ProfileCategory::Preference
    } else if contains_any(
        &lower,
        &[
            "project",
            "repo",
            "repository",
            "codebase",
            "aegis",
            "working on",
            "building",
        ],
    ) {
        ProfileCategory::ProjectContext
    } else if contains_any(
        &lower,
        &[
            "i am ",
            "i'm ",
            "i study",
            "i work",
            "my major",
            "university",
            "school",
            "job",
        ],
    ) {
        ProfileCategory::Background
    } else {
        ProfileCategory::Other
    };

    (category, note.to_string())
}

fn category_from_label(label: &str) -> Option<ProfileCategory> {
    match label.trim().to_lowercase().as_str() {
        "identity" | "name" | "profile" => Some(ProfileCategory::Identity),
        "preference" | "preferences" | "pref" | "likes" | "dislikes" => {
            Some(ProfileCategory::Preference)
        }
        "instruction" | "instructions" | "rule" | "rules" | "always" | "never" => {
            Some(ProfileCategory::Instruction)
        }
        "project" | "context" | "codebase" | "repo" => Some(ProfileCategory::ProjectContext),
        "background" | "bio" | "work" | "study" => Some(ProfileCategory::Background),
        "note" | "fact" | "memory" => Some(ProfileCategory::Other),
        _ => None,
    }
}

fn select_relevant_entries<'a>(
    entries: &'a [ProfileEntry],
    prompt: &str,
) -> Vec<(&'a ProfileEntry, u16)> {
    let prompt_keywords = extract_keywords(prompt).into_iter().collect::<HashSet<_>>();
    let profile_query = is_profile_query(prompt);

    let mut scored_entries = entries
        .iter()
        .map(|entry| (entry, score_entry(entry, &prompt_keywords, profile_query)))
        .filter(|(entry, score)| {
            profile_query || *score >= 45 || entry.category.stays_available_without_keyword_match()
        })
        .collect::<Vec<_>>();

    scored_entries.sort_by(|(left_entry, left_score), (right_entry, right_score)| {
        right_score
            .cmp(left_score)
            .then_with(|| right_entry.position.cmp(&left_entry.position))
    });

    scored_entries.truncate(MAX_SELECTED_NOTES);
    scored_entries
}

fn score_entry(
    entry: &ProfileEntry,
    prompt_keywords: &HashSet<String>,
    profile_query: bool,
) -> u16 {
    let overlap = entry
        .keywords
        .iter()
        .filter(|keyword| prompt_keywords.contains(*keyword))
        .count() as u16;

    let recency_boost = if entry.total_entries <= 1 {
        10
    } else {
        ((entry.position as u16) * 10) / ((entry.total_entries - 1) as u16)
    };

    let mut score = entry.base_weight + recency_boost + (overlap * 18);

    if profile_query {
        score += 60;
    } else if overlap == 0 && !entry.category.stays_available_without_keyword_match() {
        score = score.saturating_sub(22);
    }

    score
}

fn render_profile_context(entries: &[(&ProfileEntry, u16)]) -> String {
    let mut rendered = String::new();

    for (entry, score) in entries {
        let next_line = format!(
            "- category: {}; priority: {}; note: {}\n",
            entry.category.label(),
            priority_label(*score),
            entry.text
        );

        if rendered.len() + next_line.len() > MAX_RENDERED_CONTEXT_CHARS {
            break;
        }

        rendered.push_str(&next_line);
    }

    rendered.trim_end().to_string()
}

fn priority_label(score: u16) -> &'static str {
    if score >= 90 {
        "high"
    } else if score >= 60 {
        "medium"
    } else {
        "low"
    }
}

fn extract_keywords(text: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter_map(|word| {
            let word = word.trim();
            if word.len() < 3 || is_stop_word(word) {
                return None;
            }

            if seen.insert(word.to_string()) {
                Some(word.to_string())
            } else {
                None
            }
        })
        .collect()
}

fn is_profile_query(prompt: &str) -> bool {
    let lower = prompt.to_lowercase();
    contains_any(
        &lower,
        &[
            "what do you know about me",
            "who am i",
            "what is my name",
            "my profile",
            "about me",
            "remember about me",
            "what have i told you",
            "personal info",
            "personal information",
            "saved notes",
        ],
    )
}

fn contains_any(text: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| text.contains(needle))
}

fn is_stop_word(word: &str) -> bool {
    matches!(
        word,
        "about"
            | "after"
            | "again"
            | "also"
            | "and"
            | "are"
            | "because"
            | "but"
            | "can"
            | "could"
            | "for"
            | "from"
            | "have"
            | "into"
            | "not"
            | "now"
            | "please"
            | "should"
            | "that"
            | "the"
            | "their"
            | "them"
            | "then"
            | "there"
            | "this"
            | "use"
            | "was"
            | "with"
            | "you"
            | "your"
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_identity_note() {
        let entries = parse_profile_notes("my name is Sam");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, ProfileCategory::Identity);
        assert_eq!(entries[0].text, "my name is Sam");
    }

    #[test]
    fn parses_explicit_preference_prefix() {
        let entries = parse_profile_notes("preference: answer with concise bullet points");

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].category, ProfileCategory::Preference);
        assert_eq!(entries[0].text, "answer with concise bullet points");
    }

    #[test]
    fn profile_query_selects_saved_notes_even_without_keyword_overlap() {
        let entries =
            parse_profile_notes("my name is Sam\nproject: AEGIS is my graduation project");
        let selected = select_relevant_entries(&entries, "what do you know about me?");

        assert_eq!(selected.len(), 2);
    }

    #[test]
    fn unrelated_low_weight_notes_do_not_force_personalization() {
        let entries = parse_profile_notes("I once visited a museum");
        let selected = select_relevant_entries(&entries, "Explain TCP sockets");

        assert!(selected.is_empty());
    }

    #[test]
    fn instructions_stay_available_for_style_personalization() {
        let entries = parse_profile_notes("always explain things simply");
        let selected = select_relevant_entries(&entries, "Explain TCP sockets");

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0.category, ProfileCategory::Instruction);
    }
}
