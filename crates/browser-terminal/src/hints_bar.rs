// @file crates/browser-terminal/src/hints_bar.rs
// @description Composes the bottom hints bar string: static shortcuts and optional system message.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const SHORTCUTS: &str = "? for shortcuts · / for commands · r for refresh";

/// Composes the bottom hints bar string: static shortcuts on the left, an optional
/// system message on the right. The result always spans exactly `terminal_width`
/// columns; a line wider than the terminal is truncated so the bar never overflows the
/// row and disturbs the layout.
pub(crate) fn compose_hints_bar(system_message: Option<&str>, terminal_width: u16) -> String {
    let width = terminal_width as usize;
    let left_cols = UnicodeWidthStr::width(SHORTCUTS);
    let right = system_message.unwrap_or("");
    let right_cols = UnicodeWidthStr::width(right);
    let padding = width.saturating_sub(left_cols + right_cols);
    let composed = format!("{}{}{}", SHORTCUTS, " ".repeat(padding), right);
    fit_to_width(&composed, width)
}

/// Truncates `text` to `width` display columns without splitting a character. The caller
/// has already padded shorter lines, so this only ever trims an overflowing line.
fn fit_to_width(text: &str, width: usize) -> String {
    let mut fitted = String::new();
    let mut used_columns = 0usize;
    for character in text.chars() {
        let character_columns = UnicodeWidthChar::width(character).unwrap_or(0);
        if used_columns + character_columns > width {
            break;
        }
        fitted.push(character);
        used_columns += character_columns;
    }
    fitted
}

#[cfg(test)]
#[path = "hints_bar_tests.rs"]
mod tests;
