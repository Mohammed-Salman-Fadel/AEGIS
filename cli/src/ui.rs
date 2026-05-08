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

pub struct StreamedMarkdownRenderer<'a> {
    ui: &'a Ui,
    pending: String,
    in_code_block: bool,
    code_language: String,
    code_line_number: usize,
    printed_content: bool,
}

impl Ui {
    pub fn new(no_color: bool, verbose: bool) -> Self {
        Self { no_color, verbose }
    }

    pub fn print_banner(&self, art: &str) {
        println!("{}", self.header(art));
    }

    pub fn print_markdownish_response(&self, text: &str) {
        let mut renderer = self.streamed_markdown_renderer();

        println!();
        let _ = renderer.push(text);
        let _ = renderer.finish();
        println!();
    }

    pub fn streamed_markdown_renderer(&self) -> StreamedMarkdownRenderer<'_> {
        StreamedMarkdownRenderer {
            ui: self,
            pending: String::new(),
            in_code_block: false,
            code_language: String::new(),
            code_line_number: 1,
            printed_content: false,
        }
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

    fn render_markdown_line(&self, line: &str) -> String {
        let trimmed = line.trim();

        if let Some((prefix, rest)) = ordered_list_parts(trimmed) {
            return format!(
                "{} {}",
                self.render_list_marker(prefix),
                self.render_inline_markdown(rest)
            );
        }

        if let Some(rest) = unordered_list_part(trimmed) {
            return format!(
                "{} {}",
                self.render_list_marker("-"),
                self.render_inline_markdown(rest)
            );
        }

        if let Some(heading) = trimmed.strip_prefix("### ") {
            return self.header(heading);
        }

        if let Some(heading) = trimmed.strip_prefix("## ") {
            return self.header(heading);
        }

        if let Some(heading) = trimmed.strip_prefix("# ") {
            return self.header(heading);
        }

        self.render_inline_markdown(trimmed)
    }

    fn render_list_marker(&self, marker: &str) -> String {
        if self.no_color {
            marker.to_string()
        } else {
            format!("{}", marker.cyan().bold())
        }
    }

    fn render_code_header(&self, language: &str) -> String {
        let language = normalized_code_language(language);
        let label = format!("--- code: {language} ---");

        if self.no_color {
            label
        } else {
            format!("{}", label.bold().cyan())
        }
    }

    fn render_code_footer(&self) -> String {
        let label = "--- end code ---";

        if self.no_color {
            label.to_string()
        } else {
            format!("{}", label.dimmed())
        }
    }

    fn render_code_line(&self, line_number: usize, line: &str) -> String {
        let prefix = format!("{line_number:>3} | ");

        if self.no_color {
            return format!("{prefix}{line}");
        }

        format!(
            "{}{}",
            prefix.dimmed(),
            render_highlighted_code(line, self.no_color)
        )
    }
}

impl StreamedMarkdownRenderer<'_> {
    pub fn push(&mut self, token: &str) -> io::Result<()> {
        self.pending.push_str(token);
        self.flush_complete_lines()?;

        if !self.in_code_block {
            self.flush_soft_boundaries(false)?;
        }

        io::stdout().flush()
    }

    pub fn finish(&mut self) -> io::Result<()> {
        self.flush_soft_boundaries(true)?;

        if self.in_code_block {
            println!("{}", self.ui.render_code_footer());
            self.in_code_block = false;
        }

        io::stdout().flush()
    }

    fn flush_complete_lines(&mut self) -> io::Result<()> {
        while let Some(newline_index) = self.pending.find('\n') {
            let mut line = self.pending[..newline_index].to_string();
            if line.ends_with('\r') {
                line.pop();
            }
            self.pending.replace_range(..=newline_index, "");
            self.print_line(&line);
        }

        Ok(())
    }

    fn flush_soft_boundaries(&mut self, force: bool) -> io::Result<()> {
        if self.pending.is_empty() {
            return Ok(());
        }

        if self.in_code_block {
            if force {
                let line = std::mem::take(&mut self.pending);
                self.print_line(&line);
            }
            return Ok(());
        }

        let boundary = if force {
            Some(self.pending.len())
        } else {
            soft_text_boundary(&self.pending)
        };

        let Some(boundary) = boundary else {
            return Ok(());
        };

        let segment = self.pending[..boundary].to_string();
        self.pending.replace_range(..boundary, "");
        self.print_normal_segment(&segment);
        Ok(())
    }

    fn print_line(&mut self, line: &str) {
        let trimmed = line.trim();

        if let Some(language) = code_fence_language(trimmed) {
            if self.in_code_block {
                println!("{}", self.ui.render_code_footer());
                self.in_code_block = false;
                self.code_language.clear();
                self.code_line_number = 1;
            } else {
                if self.printed_content {
                    println!();
                }
                self.in_code_block = true;
                self.code_language = language.to_string();
                self.code_line_number = 1;
                println!("{}", self.ui.render_code_header(&self.code_language));
                self.printed_content = true;
            }
            return;
        }

        if self.in_code_block {
            println!("{}", self.ui.render_code_line(self.code_line_number, line));
            self.code_line_number += 1;
            self.printed_content = true;
            return;
        }

        self.print_normal_segment(line);
    }

    fn print_normal_segment(&mut self, segment: &str) {
        let normalized = normalize_model_response(segment);

        for line in normalized.lines() {
            if line.trim().is_empty() {
                println!();
            } else {
                println!("{}", self.ui.render_markdown_line(line));
                self.printed_content = true;
            }
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

fn ordered_list_parts(line: &str) -> Option<(&str, &str)> {
    let dot_index = line.find(". ")?;
    let marker = &line[..=dot_index];

    if marker
        .trim_end_matches('.')
        .chars()
        .all(|character| character.is_ascii_digit())
    {
        Some((marker, line[dot_index + 2..].trim()))
    } else {
        None
    }
}

fn unordered_list_part(line: &str) -> Option<&str> {
    if line.len() > 2
        && matches!(line.as_bytes()[0], b'-' | b'*' | b'+')
        && line.as_bytes()[1] == b' '
    {
        Some(line[2..].trim())
    } else {
        None
    }
}

fn code_fence_language(line: &str) -> Option<&str> {
    line.strip_prefix("```").map(|language| language.trim())
}

fn normalized_code_language(language: &str) -> String {
    match language.trim().to_ascii_lowercase().as_str() {
        "" | "text" | "txt" => "code".to_string(),
        "js" => "javascript".to_string(),
        "ts" => "typescript".to_string(),
        "py" => "python".to_string(),
        "rs" => "rust".to_string(),
        other => other.to_string(),
    }
}

fn soft_text_boundary(text: &str) -> Option<usize> {
    if text.contains("```") {
        return None;
    }

    let mut last_sentence_boundary = None;
    let mut previous = '\0';

    for (index, character) in text.char_indices() {
        if index < 72 {
            previous = character;
            continue;
        }

        if character.is_whitespace() && matches!(previous, '.' | '!' | '?' | ':') {
            last_sentence_boundary = Some(index + character.len_utf8());
        }

        previous = character;
    }

    if last_sentence_boundary.is_some() {
        return last_sentence_boundary;
    }

    if text.len() < 180 {
        return None;
    }

    text.char_indices()
        .rev()
        .find(|(index, character)| *index > 96 && character.is_whitespace())
        .map(|(index, character)| index + character.len_utf8())
}

fn render_highlighted_code(line: &str, no_color: bool) -> String {
    if no_color {
        return line.to_string();
    }

    let chars: Vec<char> = line.chars().collect();
    let mut rendered = String::new();
    let mut index = 0usize;

    while index < chars.len() {
        if index + 1 < chars.len() && chars[index] == '/' && chars[index + 1] == '/' {
            let rest: String = chars[index..].iter().collect();
            rendered.push_str(&ansi(&rest, "32;3"));
            break;
        }

        if chars[index] == '#' {
            let rest: String = chars[index..].iter().collect();
            rendered.push_str(&ansi(&rest, "32;3"));
            break;
        }

        if matches!(chars[index], '"' | '\'' | '`') {
            let (string_token, next_index) = take_string_token(&chars, index);
            rendered.push_str(&ansi(&string_token, "33"));
            index = next_index;
            continue;
        }

        if chars[index].is_ascii_digit() {
            let (number_token, next_index) = take_while(&chars, index, |character| {
                character.is_ascii_digit() || character == '.'
            });
            rendered.push_str(&ansi(&number_token, "36"));
            index = next_index;
            continue;
        }

        if is_identifier_start(chars[index]) {
            let (identifier, next_index) = take_while(&chars, index, is_identifier_continue);
            rendered.push_str(&render_identifier(&identifier));
            index = next_index;
            continue;
        }

        if is_code_punctuation(chars[index]) {
            rendered.push_str(&ansi(&chars[index].to_string(), "90"));
            index += 1;
            continue;
        }

        rendered.push(chars[index]);
        index += 1;
    }

    rendered
}

fn render_identifier(identifier: &str) -> String {
    let lower = identifier.to_ascii_lowercase();

    if is_code_keyword(&lower) {
        ansi(identifier, "34;1")
    } else if is_code_type(&lower) || identifier.chars().next().is_some_and(char::is_uppercase) {
        ansi(identifier, "35")
    } else {
        identifier.to_string()
    }
}

fn take_string_token(chars: &[char], start: usize) -> (String, usize) {
    let delimiter = chars[start];
    let mut token = String::new();
    let mut index = start;
    let mut escaped = false;

    while index < chars.len() {
        let current = chars[index];
        token.push(current);
        index += 1;

        if escaped {
            escaped = false;
            continue;
        }

        if current == '\\' {
            escaped = true;
            continue;
        }

        if index > start + 1 && current == delimiter {
            break;
        }
    }

    (token, index)
}

fn take_while<F>(chars: &[char], start: usize, predicate: F) -> (String, usize)
where
    F: Fn(char) -> bool,
{
    let mut token = String::new();
    let mut index = start;

    while index < chars.len() && predicate(chars[index]) {
        token.push(chars[index]);
        index += 1;
    }

    (token, index)
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn is_code_punctuation(character: char) -> bool {
    "{}()[].,;:+-*/%=<>!&|?".contains(character)
}

fn is_code_keyword(token: &str) -> bool {
    matches!(
        token,
        "as" | "async"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "def"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "false"
            | "fn"
            | "for"
            | "from"
            | "function"
            | "if"
            | "impl"
            | "import"
            | "in"
            | "interface"
            | "let"
            | "match"
            | "mod"
            | "mut"
            | "new"
            | "none"
            | "null"
            | "ok"
            | "pub"
            | "return"
            | "self"
            | "some"
            | "struct"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "type"
            | "use"
            | "var"
            | "while"
            | "with"
    )
}

fn is_code_type(token: &str) -> bool {
    matches!(
        token,
        "bool"
            | "dict"
            | "error"
            | "i32"
            | "i64"
            | "number"
            | "object"
            | "result"
            | "str"
            | "string"
            | "u32"
            | "u64"
            | "vec"
            | "void"
    )
}

fn ansi(text: &str, code: &str) -> String {
    format!("\x1b[{code}m{text}\x1b[0m")
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
