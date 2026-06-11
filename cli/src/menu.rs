use std::io::{self, Write};

use crate::ui::Ui;
use crate::AppResult;

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
        println!("{}", ui.warning(&format!("{title}: no choices available.")));
        return Ok(None);
    }

    println!("{}", ui.header(title));
    for (index, choice) in choices.iter().enumerate() {
        println!(
            "{}",
            ui.numbered_option(index + 1, &choice.label, &choice.description)
        );
    }
    println!();

    print!("{prompt}");
    io::stdout()
        .flush()
        .map_err(|error| format!("Could not flush menu prompt: {error}"))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("Could not read menu selection: {error}"))?;

    let selection = input.trim();
    if selection.is_empty() {
        return Ok(None);
    }

    let index: usize = match selection.parse::<usize>() {
        Ok(n) if n > 0 && n <= choices.len() => n - 1,
        _ => {
            println!(
                "{}",
                ui.warning(&format!(
                    "Invalid selection. Please enter a number between 1 and {}.",
                    choices.len()
                ))
            );
            return Ok(None);
        }
    };

    Ok(Some(choices[index].clone()))
}
