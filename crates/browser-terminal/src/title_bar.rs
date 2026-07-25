// @file crates/browser-terminal/src/title_bar.rs
// @description Composes the full-width title bar string for the browser chrome.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthChar;
use unicode_width::UnicodeWidthStr;

/// Composes the full-width title bar string: title on the left (auto-ellipsis at 50%
/// of terminal width), status indicators on the right.
pub(crate) fn compose_title_bar(
    label: &str,
    scroll_percent: u16,
    script_count: usize,
    page_byte_count: usize,
    terminal_width: u16,
) -> String {
    let width = terminal_width as usize;
    let max_label_cols = width / 2;
    let left = truncate_with_ellipsis(label, max_label_cols);
    let right = build_right_indicators(scroll_percent, script_count, page_byte_count);
    let left_width = UnicodeWidthStr::width(left.as_str());
    let available = width.saturating_sub(left_width);
    let right_width = UnicodeWidthStr::width(right.as_str());
    if right_width <= available {
        let padding = available - right_width;
        format!("{}{}{}", left, " ".repeat(padding), right)
    } else {
        // Right side overflows the available space; clamp it without an ellipsis.
        let right_clamped = clamp_to_cols(&right, available);
        let clamped_width = UnicodeWidthStr::width(right_clamped);
        let padding = available - clamped_width;
        format!("{}{}{}", left, right_clamped, " ".repeat(padding))
    }
}

/// Formats a byte count as a human-readable string: bytes for values below 1 KB,
/// one decimal place in KB below 1 MB, one decimal place in MB otherwise.
pub(crate) fn format_page_size(byte_count: usize) -> String {
    if byte_count < 1024 {
        format!("{byte_count} B")
    } else if byte_count < 1024 * 1024 {
        format!("{:.1} KB", byte_count as f64 / 1024.0)
    } else {
        format!("{:.1} MB", byte_count as f64 / (1024.0 * 1024.0))
    }
}

fn build_right_indicators(
    scroll_percent: u16,
    script_count: usize,
    page_byte_count: usize,
) -> String {
    let mut right = String::new();
    if page_byte_count > 0 {
        right.push_str(&format!("{} · ", format_page_size(page_byte_count)));
    }
    right.push_str(&format!("{scroll_percent}%"));
    if script_count == 1 {
        right.push_str(" · 1 script blocked");
    } else if script_count > 1 {
        right.push_str(&format!(" · {script_count} scripts blocked"));
    }
    right
}

fn truncate_with_ellipsis(text: &str, max_cols: usize) -> String {
    let ellipsis = '…';
    let ellipsis_width = UnicodeWidthChar::width(ellipsis).unwrap_or(1);
    if max_cols == 0 {
        return String::new();
    }
    let mut cols = 0usize;
    let mut truncation_point = None;
    for (byte_index, ch) in text.char_indices() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if cols + char_width > max_cols {
            truncation_point = Some(byte_index);
            break;
        }
        cols += char_width;
    }
    match truncation_point {
        None => text.to_string(),
        Some(cut) => {
            // Walk back to fit the ellipsis
            let mut result = String::new();
            let mut result_cols = 0usize;
            for ch in text[..cut].chars() {
                let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
                if result_cols + char_width + ellipsis_width > max_cols {
                    break;
                }
                result.push(ch);
                result_cols += char_width;
            }
            result.push(ellipsis);
            result
        }
    }
}

/// Cuts `text` to at most `max_cols` terminal columns without adding an ellipsis.
fn clamp_to_cols(text: &str, max_cols: usize) -> &str {
    let mut cols = 0usize;
    for (byte_index, ch) in text.char_indices() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if cols + char_width > max_cols {
            return &text[..byte_index];
        }
        cols += char_width;
    }
    text
}

#[cfg(test)]
#[path = "title_bar_tests.rs"]
mod tests;
