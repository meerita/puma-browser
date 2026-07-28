// @file crates/browser-terminal/src/palette_menu_tests.rs
// @description Unit tests for palette popup row formatting, truncation, and scroll window.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

use super::compose_palette_menu;
use crate::command::filter;

#[test]
fn all_commands_render_one_row_each() {
    let matches = filter("");
    let menu = compose_palette_menu(&matches, 0, 80, 9);
    assert_eq!(menu.rows.len(), matches.len());
    assert_eq!(menu.selected_row, 0);
}

#[test]
fn rows_keep_the_leading_slash_on_the_command_name() {
    let matches = filter("");
    let menu = compose_palette_menu(&matches, 0, 80, 9);
    assert!(menu.rows[0].starts_with("/open"));
}

#[test]
fn descriptions_align_into_a_shared_column() {
    let matches = filter("");
    let menu = compose_palette_menu(&matches, 0, 80, 9);
    // "/settings" is the widest name (9 columns); a two-space gap puts every description
    // at column 11 regardless of the command name length.
    let open_row = menu
        .rows
        .iter()
        .find(|row| row.starts_with("/open"))
        .expect("open command must be present");
    let settings_row = menu
        .rows
        .iter()
        .find(|row| row.starts_with("/settings"))
        .expect("settings command must be present");
    assert!(open_row[11..].starts_with("open a URL"));
    assert!(settings_row[11..].starts_with("open browser settings"));
}

#[test]
fn every_row_is_padded_to_the_full_popup_width() {
    let matches = filter("");
    let menu = compose_palette_menu(&matches, 0, 80, 9);
    for row in &menu.rows {
        assert_eq!(UnicodeWidthStr::width(row.as_str()), 80);
    }
}

#[test]
fn rows_truncate_to_a_narrow_width() {
    let matches = filter("");
    let menu = compose_palette_menu(&matches, 0, 5, 8);
    for row in &menu.rows {
        assert_eq!(UnicodeWidthStr::width(row.as_str()), 5);
    }
    // Five columns fit exactly the "/open" name and nothing more.
    let open_row = menu
        .rows
        .iter()
        .find(|row| row.starts_with("/open"))
        .expect("open command must be present");
    assert_eq!(open_row, "/open");
}

#[test]
fn scroll_window_keeps_a_selection_near_the_end_in_view() {
    let matches = filter("");
    // A cap of three rows over six matches with the last selected must scroll so the
    // selection is the bottom visible row.
    let menu = compose_palette_menu(&matches, matches.len() - 1, 80, 3);
    assert_eq!(menu.rows.len(), 3);
    assert_eq!(menu.selected_row, 2);
    assert!(menu.rows[2].starts_with("/settings"));
}

#[test]
fn scroll_window_anchors_to_the_top_when_the_selection_fits() {
    let matches = filter("");
    let menu = compose_palette_menu(&matches, 0, 80, 3);
    assert_eq!(menu.rows.len(), 3);
    assert_eq!(menu.selected_row, 0);
    assert!(menu.rows[0].starts_with("/open"));
}

#[test]
fn no_matches_produce_no_rows() {
    let menu = compose_palette_menu(&[], 0, 80, 8);
    assert!(menu.rows.is_empty());
    assert_eq!(menu.selected_row, 0);
}

#[test]
fn zero_width_produces_no_rows() {
    let matches = filter("");
    let menu = compose_palette_menu(&matches, 0, 0, 8);
    assert!(menu.rows.is_empty());
}
