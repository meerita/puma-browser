// @file crates/browser-terminal/src/history_view.rs
// @description Composes the address-suggestion and history-list popup rows and their labels.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::time::{SystemTime, UNIX_EPOCH};

use browser_core::HistoryEntry;
use unicode_width::UnicodeWidthChar;

/// Maximum number of rows a list popup shows at once. Longer lists scroll within this cap
/// so the popup never grows unbounded above the command bar.
pub(crate) const LIST_MENU_MAX_ROWS: usize = 8;

/// The most history entries a single `/history` query loads, bounding both the store read
/// and the number of rows the popup scrolls through.
pub(crate) const HISTORY_QUERY_LIMIT: usize = 100;

/// Columns of blank space between the time column and the label column of a history row.
const HISTORY_COLUMN_GAP: &str = "  ";

/// A composed list popup: the visible rows, each already truncated and padded to the popup
/// width, and which of those rows carries the selection highlight, if any.
pub(crate) struct ListMenu {
    pub(crate) rows: Vec<String>,
    pub(crate) selected_row: Option<usize>,
}

/// Composes the visible popup rows for `labels`, scrolled so `selected` stays in view and
/// each row fitted to `width` columns. A `selected` of `None` highlights no row and anchors
/// the window at the top. Returns no rows when there is nothing to show or no width for it.
pub(crate) fn compose_list_menu(
    labels: &[String],
    selected: Option<usize>,
    width: u16,
    max_rows: usize,
) -> ListMenu {
    let width = width as usize;
    if labels.is_empty() || width == 0 || max_rows == 0 {
        return ListMenu {
            rows: Vec::new(),
            selected_row: None,
        };
    }
    let (window_start, window_len) = visible_window(labels.len(), selected, max_rows);
    let rows = labels[window_start..window_start + window_len]
        .iter()
        .map(|label| fit_to_width(label, width))
        .collect();
    ListMenu {
        rows,
        selected_row: selected.map(|index| index.saturating_sub(window_start)),
    }
}

/// Chooses which slice of the list is visible so the selected row stays on screen. With no
/// selection the window is anchored at the top; with a selection it scrolls down just enough
/// to keep the selection as the bottom row once it would fall past the last visible row.
fn visible_window(list_len: usize, selected: Option<usize>, max_rows: usize) -> (usize, usize) {
    let window_len = list_len.min(max_rows);
    if window_len == 0 {
        return (0, 0);
    }
    let selected = selected.unwrap_or(0).min(list_len - 1);
    let start = if selected < max_rows {
        0
    } else {
        selected + 1 - max_rows
    };
    let max_start = list_len - window_len;
    (start.min(max_start), window_len)
}

/// Formats one history entry as `<time ago>  <label>`, where the label is the title when
/// present followed by the URL, or the URL alone. Control characters are stripped from the
/// title and URL first so a crafted stored value can never carry an escape sequence into the
/// popup, even though stored titles are already sanitized upstream.
pub(crate) fn format_history_label(entry: &HistoryEntry, now_unix: i64) -> String {
    let url = strip_control(entry.url());
    let label = match entry.title() {
        Some(title) if !title.trim().is_empty() => {
            format!("{}{HISTORY_COLUMN_GAP}{url}", strip_control(title))
        }
        _ => url,
    };
    format!(
        "{}{HISTORY_COLUMN_GAP}{label}",
        relative_time(entry.visited_at(), now_unix)
    )
}

/// A short, human-readable age for a visit at `visited_at` seconds, relative to `now`.
///
/// Buckets to seconds, minutes, hours, or days so the list reads without a date library. A
/// future or zero timestamp reports `just now` rather than a negative age.
fn relative_time(visited_at: i64, now: i64) -> String {
    let elapsed = now.saturating_sub(visited_at);
    if elapsed < 60 {
        return "just now".to_string();
    }
    if elapsed < 3_600 {
        return format!("{}m ago", elapsed / 60);
    }
    if elapsed < 86_400 {
        return format!("{}h ago", elapsed / 3_600);
    }
    format!("{}d ago", elapsed / 86_400)
}

/// The current time as Unix epoch seconds, or zero if the clock predates the epoch, so a
/// misconfigured clock only makes every entry read as `just now` rather than panicking.
pub(crate) fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// Removes control characters from `text` so no escape sequence reaches the popup.
pub(crate) fn strip_control(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control())
        .collect()
}

/// Truncates `text` to `width` display columns, then right-pads with spaces to exactly
/// `width` so a selection highlight spans the full popup line. Never splits a character.
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
    fitted.push_str(&" ".repeat(width.saturating_sub(used_columns)));
    fitted
}

#[cfg(test)]
#[path = "history_view_tests.rs"]
mod tests;
