// @file crates/browser-terminal/src/history_view_tests.rs
// @description Unit tests for list-popup composition, history labels, and relative time.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::HistoryEntry;
use unicode_width::UnicodeWidthStr;

use super::{compose_list_menu, format_history_label, relative_time};

fn labels(count: usize) -> Vec<String> {
    (0..count).map(|number| format!("row {number}")).collect()
}

#[test]
fn an_empty_label_list_composes_no_rows() {
    let menu = compose_list_menu(&[], Some(0), 40, 8);
    assert!(menu.rows.is_empty());
    assert_eq!(menu.selected_row, None);
}

#[test]
fn a_zero_width_popup_composes_no_rows() {
    let menu = compose_list_menu(&labels(3), Some(0), 0, 8);
    assert!(menu.rows.is_empty());
}

#[test]
fn each_row_is_padded_to_the_popup_width() {
    let menu = compose_list_menu(&labels(2), Some(0), 20, 8);
    assert_eq!(menu.rows.len(), 2);
    for row in &menu.rows {
        assert_eq!(UnicodeWidthStr::width(row.as_str()), 20);
    }
}

#[test]
fn no_selection_highlights_no_row() {
    let menu = compose_list_menu(&labels(3), None, 40, 8);
    assert_eq!(menu.selected_row, None);
}

#[test]
fn the_selection_maps_into_the_visible_window() {
    let menu = compose_list_menu(&labels(3), Some(2), 40, 8);
    assert_eq!(menu.selected_row, Some(2));
}

#[test]
fn a_long_list_scrolls_to_keep_the_selection_as_the_bottom_row() {
    // Ten labels into a four-row window: selecting row 5 scrolls so it sits on the bottom.
    let menu = compose_list_menu(&labels(10), Some(5), 40, 4);
    assert_eq!(menu.rows.len(), 4);
    assert_eq!(menu.selected_row, Some(3));
}

#[test]
fn a_label_wider_than_the_popup_is_truncated_to_the_width() {
    let long = vec!["a-very-long-row-that-exceeds-the-width".to_string()];
    let menu = compose_list_menu(&long, Some(0), 10, 8);
    assert_eq!(UnicodeWidthStr::width(menu.rows[0].as_str()), 10);
}

#[test]
fn a_history_label_shows_the_url_when_no_title_is_stored() {
    let entry = HistoryEntry::new(1, "https://example.com/".to_string(), None, 0);
    let label = format_history_label(&entry, 0);
    assert!(
        label.contains("https://example.com/"),
        "label was {label:?}"
    );
}

#[test]
fn a_history_label_shows_the_title_and_url_when_a_title_is_stored() {
    let entry = HistoryEntry::new(
        1,
        "https://example.com/".to_string(),
        Some("Example Domain".to_string()),
        0,
    );
    let label = format_history_label(&entry, 0);
    assert!(label.contains("Example Domain"), "label was {label:?}");
    assert!(
        label.contains("https://example.com/"),
        "label was {label:?}"
    );
}

#[test]
fn a_history_label_strips_control_characters_from_the_title() {
    let entry = HistoryEntry::new(
        1,
        "https://example.com/".to_string(),
        Some("Ti\u{1b}tle".to_string()),
        0,
    );
    let label = format_history_label(&entry, 0);
    assert!(!label.contains('\u{1b}'), "label kept an escape: {label:?}");
    assert!(label.contains("Title"), "label was {label:?}");
}

#[test]
fn relative_time_buckets_recent_visits_as_just_now() {
    assert_eq!(relative_time(100, 130), "just now");
}

#[test]
fn relative_time_reports_minutes_hours_and_days() {
    assert_eq!(relative_time(0, 120), "2m ago");
    assert_eq!(relative_time(0, 7_200), "2h ago");
    assert_eq!(relative_time(0, 172_800), "2d ago");
}

#[test]
fn relative_time_of_a_future_visit_reads_as_just_now() {
    assert_eq!(relative_time(500, 100), "just now");
}
