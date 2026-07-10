//! Role: banner policy and the AEGIS ASCII art used by friendly entry flows.
//! Called by: `main.rs` before command dispatch.
//! Calls into: `cli.rs` command variants for banner decisions.
//! Owns: the banner string and the policy for when it should appear.
//! Does not own: command output, menus, or backend interactions.
//! Next TODOs: let users disable the banner from future CLI config and theme settings.

use crate::cli::CommandKind;

pub const AEGIS_ASCII_ART: &str = r#"
 ______________________________________________________________________________________________________
|                                                                                                       |
|    _____/\\\\\\\\\_____/\\\\\\\\\\\\\\\_____/\\\\\\\\\\\\__/\\\\\\\\\\\_____/\\\\\\\\\\\___           |
|     ___/\\\\\\\\\\\\\__\/\\\///////////____/\\\//////////__\/////\\\///____/\\\/////////\\\_          |
|      __/\\\/////////\\\_\/\\\______________/\\\_________________\/\\\______\//\\\______\///__         |
|       _\/\\\_______\/\\\_\/\\\\\\\\\\\_____\/\\\____/\\\\\\\_____\/\\\_______\////\\\_________        |
|        _\/\\\\\\\\\\\\\\\_\/\\\///////______\/\\\___\/////\\\_____\/\\\__________\////\\\______       |
|         _\/\\\/////////\\\_\/\\\_____________\/\\\_______\/\\\_____\/\\\_____________\////\\\___      |
|          _\/\\\_______\/\\\_\/\\\_____________\/\\\_______\/\\\_____\/\\\______/\\\______\//\\\__     |
|           _\/\\\_______\/\\\_\/\\\\\\\\\\\\\\\_\//\\\\\\\\\\\\/___/\\\\\\\\\\\_\///\\\\\\\\\\\/___    |
|            _\///________\///__\///////////////___\////////////____\///////////____\///////////_____   |
|                                                                                                       |
|_______________________________________________________________________________________________________|
"#;

pub fn render_with_model(active_model: Option<&str>) -> String {
    let model = active_model
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .unwrap_or("unavailable");
    let label = format!("Current active model: {model}");
    let mut lines = AEGIS_ASCII_ART
        .trim_matches('\n')
        .lines()
        .map(str::to_string)
        .collect::<Vec<_>>();

    let Some(bottom_border) = lines.pop() else {
        return AEGIS_ASCII_ART.to_string();
    };

    let width = bottom_border.chars().count();
    let inner_width = width.saturating_sub(2);
    let label = fit_to_width(&label, inner_width);
    let label_width = label.chars().count();
    let left_padding = inner_width.saturating_sub(label_width) / 2;
    let right_padding = inner_width.saturating_sub(label_width + left_padding);

    lines.push(model_line(&label, left_padding, right_padding));
    lines.push(bottom_border);

    format!("\n{}\n", lines.join("\n"))
}

fn model_line(label: &str, left_padding: usize, right_padding: usize) -> String {
    format!(
        "|{}{}{}|",
        " ".repeat(left_padding),
        label,
        " ".repeat(right_padding)
    )
}

fn fit_to_width(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }

    if width <= 3 {
        return ".".repeat(width);
    }

    let mut fitted = text.chars().take(width - 3).collect::<String>();
    fitted.push_str("...");
    fitted
}

pub fn should_render_banner(command: Option<&CommandKind>) -> bool {
    matches!(
        command,
        None | Some(CommandKind::Open) | Some(CommandKind::Ask(_)) | Some(CommandKind::Repl(_))
    )
}
