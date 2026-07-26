// @file crates/browser-terminal/src/palette_menu.rs
// @description Composes the slash-command palette popup rows, alignment, and scroll window.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::command::CommandMatch;

/// Maximum number of command rows the palette popup shows at once. Longer match lists
/// scroll within this cap so the popup never grows unbounded above the command bar.
pub(crate) const MENU_MAX_ROWS: usize = 8;

/// Columns of blank space between the command-name column and the description column.
const DESCRIPTION_GAP: &str = "  ";

/// Marker shown before the argument hint, echoing the Tab key that completes the command.
const ARG_HINT_PREFIX: &str = "↹ ";

/// A composed palette popup: the visible rows, each already truncated and padded to the
/// popup width, and which of those rows carries the selection highlight.
pub(crate) struct PaletteMenu {
    pub(crate) rows: Vec<String>,
    pub(crate) selected_row: usize,
}

/// Composes the visible palette rows for `matches`, scrolled so `selected` stays in view
/// and each row fitted to `width` columns. Descriptions align into a column past the
/// widest command name across the whole match list, so the column stays put while
/// scrolling. Returns no rows when there is nothing to show or no width to show it in.
pub(crate) fn compose_palette_menu(
    matches: &[CommandMatch],
    selected: usize,
    width: u16,
    max_rows: usize,
) -> PaletteMenu {
    let width = width as usize;
    if matches.is_empty() || width == 0 || max_rows == 0 {
        return PaletteMenu {
            rows: Vec::new(),
            selected_row: 0,
        };
    }
    let selected = selected.min(matches.len() - 1);
    let (window_start, window_len) = visible_window(matches.len(), selected, max_rows);
    let name_column = name_column_width(matches);
    let rows = matches[window_start..window_start + window_len]
        .iter()
        .map(|command_match| {
            format_menu_row(
                command_match.spec.name,
                command_match.spec.description,
                name_column,
                width,
            )
        })
        .collect();
    PaletteMenu {
        rows,
        selected_row: selected - window_start,
    }
}

/// Formats the argument-hint row (for example `↹ /open <url>`) shown beneath the menu,
/// fitted to `width` columns so it spans the popup line like the command rows. The hint
/// text comes from the static registry, so the row never carries page content.
pub(crate) fn compose_arg_hint_row(hint: &str, width: u16) -> String {
    let prefixed = format!("{ARG_HINT_PREFIX}{hint}");
    fit_to_width(&prefixed, width as usize)
}

/// Chooses which slice of the match list is visible so the selected row stays on screen.
/// The window is anchored to the top until the selection would fall past the last visible
/// row, then it scrolls down just enough to keep the selection as the bottom row.
fn visible_window(match_count: usize, selected: usize, max_rows: usize) -> (usize, usize) {
    let window_len = match_count.min(max_rows);
    if window_len == 0 {
        return (0, 0);
    }
    let start = if selected < max_rows {
        0
    } else {
        selected + 1 - max_rows
    };
    let max_start = match_count - window_len;
    (start.min(max_start), window_len)
}

/// Display width of the widest `/name` label across every match, used as the aligned
/// description column position.
fn name_column_width(matches: &[CommandMatch]) -> usize {
    matches
        .iter()
        .map(|command_match| {
            UnicodeWidthStr::width(format!("/{}", command_match.spec.name).as_str())
        })
        .max()
        .unwrap_or(0)
}

/// Formats one popup row as `/name  description`, padding the name into its aligned column
/// and fitting the whole row to exactly `width` columns.
fn format_menu_row(name: &str, description: &str, name_column: usize, width: usize) -> String {
    let labeled_name = format!("/{name}");
    let name_width = UnicodeWidthStr::width(labeled_name.as_str());
    let name_padding = name_column.saturating_sub(name_width);
    let full_row = format!(
        "{labeled_name}{}{DESCRIPTION_GAP}{description}",
        " ".repeat(name_padding)
    );
    fit_to_width(&full_row, width)
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
#[path = "palette_menu_tests.rs"]
mod tests;
