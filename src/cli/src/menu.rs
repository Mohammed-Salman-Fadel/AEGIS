//! Role: numbered prompt scaffold for interactive provider, model, and session picks.
//! Called by: `commands.rs` when a selection command omits a target and stdin is interactive.
//! Calls into: `ui.rs` for rendering and standard library stdin for input.
//! Owns: simple numbered menu prompting and returning the chosen value.
//! Does not own: the source of selectable data or any persistence of the chosen value.
//! Next TODOs: support retries, richer descriptions, and non-blocking terminal UX once the scaffold is replaced.

use std::io::{self, Write};

use crate::AppResult;
use crate::signals;
use crate::ui::Ui;

#[derive(Debug, Clone)]
pub struct MenuChoice {
    pub label: String,
    pub value: String,
    pub description: String,
}

impl MenuChoice {
    pub fn new(
        label: impl Into<String>,
        value: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            description: description.into(),
        }
    }
}

pub fn choose_from_stdin(
    ui: &Ui,
    title: &str,
    prompt: &str,
    choices: &[MenuChoice],
) -> AppResult<Option<MenuChoice>> {
    if choices.is_empty() {
        return Ok(None);
    }

    println!("{}", ui.header(title));
    println!(
        "{}",
        ui.muted("Scaffold menu: future versions should source these options from the engine.")
    );

    for (index, choice) in choices.iter().enumerate() {
        println!(
            "{}",
            ui.numbered_option(index + 1, &choice.label, &choice.description)
        );
    }

    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not flush menu prompt: {error}"))?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).map_err(|error| {
        if signals::was_ctrl_c(&error) {
            signals::ctrl_c_exit_error()
        } else {
            format!("Could not read menu selection: {error}")
        }
    })?;

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let selected = trimmed
        .parse::<usize>()
        .map_err(|_| "Please enter a valid option number.".to_string())?;

    let index = selected
        .checked_sub(1)
        .ok_or_else(|| "Please choose an option number greater than zero.".to_string())?;

    choices
        .get(index)
        .cloned()
        .map(Some)
        .ok_or_else(|| "That option number is outside the available range.".to_string())
}
