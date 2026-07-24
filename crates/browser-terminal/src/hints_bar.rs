// @file crates/browser-terminal/src/hints_bar.rs
// @description Composes the bottom hints bar string: static shortcuts and optional system message.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

const SHORTCUTS: &str = "? for shortcuts · r for refresh";

/// Composes the bottom hints bar string: static shortcuts on the left, an optional
/// system message on the right.
pub(crate) fn compose_hints_bar(system_message: Option<&str>, terminal_width: u16) -> String {
    let width = terminal_width as usize;
    let left_cols = UnicodeWidthStr::width(SHORTCUTS);
    let right = system_message.unwrap_or("");
    let right_cols = UnicodeWidthStr::width(right);
    let padding = width.saturating_sub(left_cols + right_cols);
    format!("{}{}{}", SHORTCUTS, " ".repeat(padding), right)
}

#[cfg(test)]
#[path = "hints_bar_tests.rs"]
mod tests;
