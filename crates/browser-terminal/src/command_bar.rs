// @file crates/browser-terminal/src/command_bar.rs
// @description Composes the command bar string for reading mode.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

/// Composes the command bar string for reading mode: the prompt prefix and the current
/// hint on the left, padded to fill the terminal width.
pub(crate) fn compose_command_bar_reading(hint: &str, terminal_width: u16) -> String {
    let left = format!("> {hint}");
    let width = terminal_width as usize;
    let left_cols = UnicodeWidthStr::width(left.as_str());
    let padding = width.saturating_sub(left_cols);
    format!("{}{}", left, " ".repeat(padding))
}

#[cfg(test)]
#[path = "command_bar_tests.rs"]
mod tests;
