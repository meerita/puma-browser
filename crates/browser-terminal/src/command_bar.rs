// @file crates/browser-terminal/src/command_bar.rs
// @description Composes the command bar string for reading, command, and loading modes.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

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

/// Composes the command bar string for loading mode: spinner, URL being loaded, and live
/// byte count right-aligned. The byte count is shown in KB (truncated, no decimals).
pub(crate) fn compose_command_bar_loading(
    frame: usize,
    url: &str,
    bytes_received: usize,
    terminal_width: u16,
) -> String {
    let spinner = SPINNER_FRAMES[frame % SPINNER_FRAMES.len()];
    let kb = bytes_received / 1024;
    let left = format!("{spinner} Loading {url}\u{2026}");
    let right = format!("{kb} KB");
    let width = terminal_width as usize;
    let left_cols = UnicodeWidthStr::width(left.as_str());
    let right_cols = UnicodeWidthStr::width(right.as_str());
    if left_cols + right_cols <= width {
        let padding = width - left_cols - right_cols;
        format!("{}{}{}", left, " ".repeat(padding), right)
    } else {
        let padding = width.saturating_sub(left_cols);
        format!("{}{}", left, " ".repeat(padding))
    }
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
