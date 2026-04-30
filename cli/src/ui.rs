//! Role: terminal presentation helpers for the AEGIS CLI scaffold.
//! Called by: `main.rs`, `commands.rs`, and `menu.rs`.
//! Calls into: `owo-colors` only for styling and formatting.
//! Owns: banner printing, badges, TODO emphasis, and lightweight menu formatting.
//! Does not own: backend calls, command routing, or workspace discovery.
//! Next TODOs: add richer layout helpers, width-aware formatting, and configurable themes.

use std::io::{self, IsTerminal, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use owo_colors::OwoColorize;

use crate::doctor::Health;
use crate::workspace::ComponentState;

#[derive(Debug, Clone)]
pub struct Ui {
    pub no_color: bool,
    pub verbose: bool,
}

pub struct LoadingAnimation {
    active: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl Ui {
    pub fn new(no_color: bool, verbose: bool) -> Self {
        Self { no_color, verbose }
    }

    pub fn print_banner(&self, art: &str) {
        println!("{}", self.header(art));
    }

    pub fn print_markdownish_response(&self, text: &str) {
        let normalized = normalize_model_response(text);

        if normalized.trim().is_empty() {
            println!();
            return;
        }

        println!();

        for line in normalized.lines() {
            if line.trim().is_empty() {
                println!();
            } else {
                println!("{}", self.render_inline_markdown(line));
            }
        }

        println!();
    }

    pub fn clear_screen(&self) {
        // ANSI clear screen + scrollback + move cursor home.
        // This only affects terminal display and does not touch any CLI or session state.
        print!("\x1B[2J\x1B[3J\x1B[H");
        let _ = io::stdout().flush();
    }

    pub fn play_exit_animation(&self, reason: &str) {
        let status = if self.no_color {
            "Shutting down AEGIS".to_string()
        } else {
            format!("{}", "Shutting down AEGIS".bold().cyan())
        };

        println!();
        println!("{}", self.warning(reason));

        for frame in ["[=   ]", "[==  ]", "[=== ]", "[====]"] {
            let frame = if self.no_color {
                frame.to_string()
            } else {
                format!("{}", frame.dimmed())
            };

            print!("\r{status} {frame}");
            let _ = io::stdout().flush();
            thread::sleep(Duration::from_millis(90));
        }

        println!();
        println!("{}", self.muted("AEGIS exited cleanly."));
    }

    pub fn start_loading_animation(&self, message: &str) -> LoadingAnimation {
        let active = Arc::new(AtomicBool::new(true));

        if !io::stderr().is_terminal() {
            eprintln!("{message}");
            return LoadingAnimation {
                active,
                handle: None,
            };
        }

        let rendered_message = if self.no_color {
            message.to_string()
        } else {
            format!("{}", message.bold().cyan())
        };
        let clear_width = message.len() + 8;
        let active_clone = Arc::clone(&active);

        let handle = thread::spawn(move || {
            let frames = ["|", "/", "-", "\\"];
            let mut index = 0usize;

            while active_clone.load(Ordering::Relaxed) {
                eprint!("\r{} {}", rendered_message, frames[index % frames.len()]);
                let _ = io::stderr().flush();
                thread::sleep(Duration::from_millis(100));
                index += 1;
            }

            eprint!("\r{}\r", " ".repeat(clear_width));
            let _ = io::stderr().flush();
        });

        LoadingAnimation {
            active,
            handle: Some(handle),
        }
    }

    pub fn header(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("{}", text.bold().cyan())
        }
    }

    pub fn success(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("{}", text.green())
        }
    }

    pub fn info(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("{}", text.blue())
        }
    }

    pub fn warning(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("{}", text.yellow())
        }
    }

    pub fn error(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("{}", text.red())
        }
    }

    pub fn muted(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("{}", text.dimmed())
        }
    }

    pub fn todo(&self, text: &str) -> String {
        let text = text.strip_prefix("TODO: ").unwrap_or(text);
        if self.no_color {
            format!("TODO: {text}")
        } else {
            format!("{} {}", "TODO:".yellow().bold(), text.yellow())
        }
    }

    pub fn numbered_option(&self, index: usize, label: &str, description: &str) -> String {
        format!("{index}. {label} - {description}")
    }

    pub fn badge(&self, health: Health) -> String {
        match health {
            Health::Ok => self.success("[OK]"),
            Health::Info => self.info("[INFO]"),
            Health::Warn => self.warning("[WARN]"),
            Health::Missing => self.error("[MISS]"),
        }
    }

    pub fn component_badge(&self, state: ComponentState) -> String {
        match state {
            ComponentState::Ready => self.success("[READY]"),
            ComponentState::Scaffolded => self.warning("[SCAFFOLD]"),
            ComponentState::Missing => self.error("[MISSING]"),
        }
    }

    fn render_inline_markdown(&self, text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut rendered = String::new();
        let mut index = 0usize;

        while index < chars.len() {
            if starts_with(&chars, index, "**") {
                if let Some(end) = find_delimiter(&chars, index + 2, "**") {
                    let inner: String = chars[index + 2..end].iter().collect();
                    rendered.push_str(&self.render_bold(&inner));
                    index = end + 2;
                    continue;
                }
            }

            if chars[index] == '*' {
                if let Some(end) = find_delimiter(&chars, index + 1, "*") {
                    let inner: String = chars[index + 1..end].iter().collect();
                    rendered.push_str(&self.render_italic(&inner));
                    index = end + 1;
                    continue;
                }
            }

            if chars[index] == '_' {
                if let Some(end) = find_delimiter(&chars, index + 1, "_") {
                    let inner: String = chars[index + 1..end].iter().collect();
                    rendered.push_str(&self.render_italic(&inner));
                    index = end + 1;
                    continue;
                }
            }

            if chars[index] == '`' {
                if let Some(end) = find_delimiter(&chars, index + 1, "`") {
                    let inner: String = chars[index + 1..end].iter().collect();
                    rendered.push_str(&self.render_inline_code(&inner));
                    index = end + 1;
                    continue;
                }
            }

            rendered.push(chars[index]);
            index += 1;
        }

        rendered
    }

    fn render_bold(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("\x1b[1m{text}\x1b[0m")
        }
    }

    fn render_italic(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("\x1b[3m{text}\x1b[0m")
        }
    }

    fn render_inline_code(&self, text: &str) -> String {
        if self.no_color {
            text.to_string()
        } else {
            format!("{}", text.cyan())
        }
    }
}

impl LoadingAnimation {
    pub fn finish(mut self) {
        self.active.store(false, Ordering::Relaxed);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for LoadingAnimation {
    fn drop(&mut self) {
        self.active.store(false, Ordering::Relaxed);

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn normalize_model_response(text: &str) -> String {
    let input = text.replace("\r\n", "\n");
    let chars: Vec<char> = input.chars().collect();
    let mut normalized = String::with_capacity(input.len() + 32);
    let mut index = 0usize;

    while index < chars.len() {
        if is_list_start(&chars, index) && !normalized.is_empty() && !normalized.ends_with('\n') {
            normalized.push('\n');
        }

        let current = chars[index];
        normalized.push(current);

        if matches!(current, '.' | '!' | '?' | ':')
            && chars
                .get(index + 1)
                .is_some_and(|next| !next.is_whitespace())
        {
            if current == ':' && is_list_start(&chars, index + 1) {
                normalized.push('\n');
            } else {
                normalized.push(' ');
            }
        }

        index += 1;
    }

    collapse_blank_lines(&normalized)
}

fn collapse_blank_lines(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut blank_run = 0usize;

    for line in text.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                output.push('\n');
            }
            continue;
        }

        blank_run = 0;
        if !output.is_empty() && !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(line.trim_end());
        output.push('\n');
    }

    output.trim_end().to_string()
}

fn is_list_start(chars: &[char], index: usize) -> bool {
    if index >= chars.len() {
        return false;
    }

    if index + 1 < chars.len() && matches!(chars[index], '*' | '-') && chars[index + 1] == ' ' {
        return true;
    }

    let mut cursor = index;
    let mut saw_digit = false;
    while cursor < chars.len() && chars[cursor].is_ascii_digit() {
        saw_digit = true;
        cursor += 1;
    }

    saw_digit && cursor + 1 < chars.len() && chars[cursor] == '.' && chars[cursor + 1] == ' '
}

fn starts_with(chars: &[char], index: usize, needle: &str) -> bool {
    let needle_chars: Vec<char> = needle.chars().collect();
    chars
        .get(index..index + needle_chars.len())
        .is_some_and(|slice| slice == needle_chars.as_slice())
}

fn find_delimiter(chars: &[char], start: usize, delimiter: &str) -> Option<usize> {
    let delimiter_chars: Vec<char> = delimiter.chars().collect();

    (start..chars.len()).find(|&index| {
        chars
            .get(index..index + delimiter_chars.len())
            .is_some_and(|slice| slice == delimiter_chars.as_slice())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_missing_spaces_and_lists() {
        let raw = "Differential Equations!A differential equation is useful.Applications:1. **Physics**2. **Biology**";
        let normalized = normalize_model_response(raw);

        assert!(normalized.contains("Differential Equations! A differential equation is useful."));
        assert!(normalized.contains("Applications:\n1. **Physics**"));
        assert!(normalized.contains("\n2. **Biology**"));
    }

    #[test]
    fn strips_markdown_markers_when_colors_are_disabled() {
        let ui = Ui::new(true, false);
        let rendered = ui.render_inline_markdown("Use **bold**, *italic*, and `code`.");

        assert_eq!(rendered, "Use bold, italic, and code.");
    }
}
