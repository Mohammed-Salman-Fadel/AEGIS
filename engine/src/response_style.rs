use std::env;
use std::fs;
use std::path::PathBuf;

const DEFAULT_STYLE: &str = "default";

pub fn apply_response_style(prompt: &str, requested_style: Option<&str>) -> String {
    let style = normalize_style(requested_style);
    let style_prompt = load_style_prompt(style);

    format!("SYSTEM RESPONSE STYLE ({style}):\n{style_prompt}\n\nREQUEST CONTEXT:\n{prompt}")
}

fn normalize_style(requested_style: Option<&str>) -> &'static str {
    match requested_style
        .unwrap_or(DEFAULT_STYLE)
        .trim()
        .to_lowercase()
        .as_str()
    {
        "friendly" => "friendly",
        "concise" | "conscise" => "concise",
        "elaborate" | "detailed" => "elaborate",
        "technical" | "precise" => "technical",
        _ => DEFAULT_STYLE,
    }
}

fn load_style_prompt(style: &str) -> String {
    if let Some(path) = style_prompt_path(style) {
        if let Ok(contents) = fs::read_to_string(path) {
            let trimmed = contents.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }

    fallback_style_prompt(style).to_string()
}

fn style_prompt_path(style: &str) -> Option<PathBuf> {
    let file_name = format!("{style}.md");

    prompt_dir_candidates()
        .into_iter()
        .map(|dir| dir.join(&file_name))
        .find(|path| path.is_file())
}

fn prompt_dir_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(path) = env::var("AEGIS_RESPONSE_STYLE_PROMPTS_DIR") {
        candidates.push(PathBuf::from(path));
    }

    if let Ok(current_dir) = env::current_dir() {
        candidates.push(current_dir.join("system_prompts").join("response_styles"));
        candidates.push(
            current_dir
                .join("engine")
                .join("system_prompts")
                .join("response_styles"),
        );
        if let Some(parent) = current_dir.parent() {
            candidates.push(parent.join("system_prompts").join("response_styles"));
            candidates.push(
                parent
                    .join("engine")
                    .join("system_prompts")
                    .join("response_styles"),
            );
        }
    }

    candidates
}

fn fallback_style_prompt(style: &str) -> &'static str {
    match style {
        "friendly" => {
            "Respond warmly and naturally. Be encouraging, approachable, and clear. Keep a collaborative tone, explain without sounding formal, and use gentle structure when it helps."
        }
        "concise" => {
            "Respond with the shortest complete answer that solves the user's request. Prioritize directness, remove filler, and use bullets only when they improve scanability."
        }
        "elaborate" => {
            "Respond with extra depth and context. Explain reasoning, tradeoffs, and examples where useful, while keeping the answer organized and practical."
        }
        "technical" => {
            "Respond with precise technical detail. Use exact terminology, name assumptions, include implementation-level guidance, and avoid vague phrasing."
        }
        _ => {
            "Use the default AEGIS assistant behavior: helpful, accurate, privacy-first, locally aware, and balanced. Match the user's tone and provide clear answers without unnecessary verbosity."
        }
    }
}
