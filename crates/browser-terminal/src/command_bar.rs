// @file crates/browser-terminal/src/command_bar.rs
// @description Composes the command bar string for reading and command modes.
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

/// Composes the command bar string for command mode: the prompt prefix and the current
/// buffer content, padded to fill the terminal width.
pub(crate) fn compose_command_bar_command(buffer: &str, terminal_width: u16) -> String {
    let left = format!("> {buffer}");
    let width = terminal_width as usize;
    let left_cols = UnicodeWidthStr::width(left.as_str());
    let padding = width.saturating_sub(left_cols);
    format!("{}{}", left, " ".repeat(padding))
}

/// Returns the visual column of the terminal cursor within the command bar row.
///
/// The command bar shows `"> <buffer>"`, so the cursor column is 2 (the "> " prefix)
/// plus the display width of the buffer content before the cursor byte offset.
pub(crate) fn command_cursor_col(buffer: &str, cursor_byte_offset: usize) -> u16 {
    let before_cursor = &buffer[..cursor_byte_offset];
    let text_cols = UnicodeWidthStr::width(before_cursor);
    (2 + text_cols) as u16
}

#[cfg(test)]
#[path = "command_bar_tests.rs"]
mod tests;
