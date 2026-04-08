//! Role: terminal presentation helpers for the AEGIS CLI scaffold.
//! Called by: `main.rs`, `commands.rs`, and `menu.rs`.
//! Calls into: `owo-colors` only for styling and formatting.
//! Owns: banner printing, badges, TODO emphasis, and lightweight menu formatting.
//! Does not own: backend calls, command routing, or workspace discovery.
//! Next TODOs: add richer layout helpers, width-aware formatting, and configurable themes.

use std::io::{self, Write};
use std::thread;
use std::time::Duration;

use owo_colors::OwoColorize;

use crate::doctor::Health;
use crate::workspace::ComponentState;

#[derive(Debug, Clone)]
pub struct Ui {
    pub no_color: bool,
    pub verbose: bool,
}

impl Ui {
    pub fn new(no_color: bool, verbose: bool) -> Self {
        Self { no_color, verbose }
    }

    pub fn print_banner(&self, art: &str) {
        println!("{}", self.header(art));
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
    pub fn status_indicator(&self, health: &Health) -> String {
        match health {
            Health::Ok => format!("{} {}", "●".green(), self.success("SYSTEM ACTIVE")),
            Health::Warn => format!("{} {}", "●".yellow(), self.warning("SYSTEM DEGRADED")),
            _ => format!("{} {}", "○".dimmed(), self.muted("SYSTEM IDLE")),
        }
    }

    pub fn local_badge(&self) -> String {
        if self.no_color {
            "[LOCAL-ONLY]".to_string()
        } else {
            format!("{}", "[LOCAL-ONLY]".on_blue().white().bold())
        }
    }

    pub fn stop_button_hint(&self) -> String {
        if self.no_color {
            "Press 'q' or 'Ctrl+C' to STOP".to_string()
        } else {
            format!("{} {}", " ■ STOP ".on_red().white().bold(), "Press 'q' to exit".dimmed())
        }
    }
}
