// @file crates/browser-terminal/src/command_bar.rs
// @description Composes the command bar string for reading mode.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

/// Composes the command bar string for reading mode: the prompt and a contextual or
/// static hint on the left; an optional status message on the right.
pub(crate) fn compose_command_bar_reading(quit_armed: bool, terminal_width: u16) -> String {
    let left = if quit_armed {
        "> Press Esc again to quit"
    } else {
        "> Type a URL or press / for commands"
    };
    let width = terminal_width as usize;
    let left_cols = UnicodeWidthStr::width(left);
    let padding = width.saturating_sub(left_cols);
    format!("{}{}", left, " ".repeat(padding))
}

#[cfg(test)]
#[path = "command_bar_tests.rs"]
mod tests;
