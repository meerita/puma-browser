// @file crates/browser-terminal/src/selection_tests.rs
// @description Unit tests for TextSelection gesture state and range normalization.
// @layer terminal
// @created meerita <meerita@icloud.com>

use super::TextSelection;
use browser_layout::CellPosition;

fn position(column: u16, row: u16) -> CellPosition {
    CellPosition { column, row }
}

#[test]
fn new_selection_is_idle_with_no_range() {
    let selection = TextSelection::new();
    assert!(!selection.is_dragging());
    assert!(!selection.has_moved());
    assert_eq!(selection.range(), None);
}

#[test]
fn begin_starts_dragging_without_a_range() {
    let mut selection = TextSelection::new();
    selection.begin(position(4, 2));
    assert!(selection.is_dragging());
    assert!(!selection.has_moved());
    assert_eq!(selection.range(), None);
}

#[test]
fn update_to_the_anchor_cell_does_not_mark_moved() {
    let mut selection = TextSelection::new();
    selection.begin(position(4, 2));
    selection.update(position(4, 2));
    assert!(!selection.has_moved());
    assert_eq!(selection.range(), None);
}

#[test]
fn update_to_a_different_cell_marks_moved_and_yields_a_range() {
    let mut selection = TextSelection::new();
    selection.begin(position(4, 2));
    selection.update(position(7, 2));
    assert!(selection.has_moved());
    assert_eq!(selection.range(), Some((position(4, 2), position(7, 2))));
}

#[test]
fn update_without_a_begin_is_ignored() {
    let mut selection = TextSelection::new();
    selection.update(position(7, 2));
    assert!(!selection.is_dragging());
    assert_eq!(selection.range(), None);
}

#[test]
fn range_is_normalized_when_the_cursor_precedes_the_anchor_by_row() {
    let mut selection = TextSelection::new();
    selection.begin(position(3, 5));
    selection.update(position(8, 2));
    assert_eq!(selection.range(), Some((position(8, 2), position(3, 5))));
}

#[test]
fn range_is_normalized_by_column_on_the_same_row() {
    let mut selection = TextSelection::new();
    selection.begin(position(9, 1));
    selection.update(position(2, 1));
    assert_eq!(selection.range(), Some((position(2, 1), position(9, 1))));
}

#[test]
fn moved_gesture_stays_a_drag_after_returning_to_the_anchor() {
    let mut selection = TextSelection::new();
    selection.begin(position(4, 2));
    selection.update(position(7, 2));
    selection.update(position(4, 2));
    assert!(selection.has_moved());
    assert_eq!(selection.range(), Some((position(4, 2), position(4, 2))));
}

#[test]
fn clear_discards_the_selection() {
    let mut selection = TextSelection::new();
    selection.begin(position(4, 2));
    selection.update(position(7, 2));
    selection.clear();
    assert!(!selection.is_dragging());
    assert!(!selection.has_moved());
    assert_eq!(selection.range(), None);
}

#[test]
fn begin_resets_a_prior_moved_gesture() {
    let mut selection = TextSelection::new();
    selection.begin(position(4, 2));
    selection.update(position(7, 2));
    selection.begin(position(1, 0));
    assert!(selection.is_dragging());
    assert!(!selection.has_moved());
    assert_eq!(selection.range(), None);
}
