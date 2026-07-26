// @file crates/browser-terminal/src/lib_tests.rs
// @description Tests for mouse gesture dispatch, coordinate mapping, and selection highlight.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::time::Instant;

use super::{
    cell_is_selected, clamped_document_coordinate, copied_message, document_coordinate,
    handle_mouse_event, TextSelection, UiState, BODY_AREA_TOP_ROW, CONTENT_PADDING,
};
use browser_layout::{CellBuffer, CellPosition};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn position(column: u16, row: u16) -> CellPosition {
    CellPosition { column, row }
}

#[test]
fn document_coordinate_shifts_by_padding_and_adds_scroll() {
    let event = mouse(MouseEventKind::Moved, 5, 3);
    let coordinate = document_coordinate(&event, 4, BODY_AREA_TOP_ROW);
    assert_eq!(coordinate, Some(position(5 - CONTENT_PADDING, 3 + 4)));
}

#[test]
fn document_coordinate_in_left_padding_maps_to_none() {
    let event = mouse(MouseEventKind::Moved, 1, 3);
    assert_eq!(document_coordinate(&event, 0, BODY_AREA_TOP_ROW), None);
}

#[test]
fn clamped_coordinate_past_content_clamps_to_last_cell() {
    let buffer = CellBuffer::new(10, 6);
    let event = mouse(MouseEventKind::Drag(MouseButton::Left), 100, 100);
    let coordinate = clamped_document_coordinate(&event, &buffer, 0, BODY_AREA_TOP_ROW);
    assert_eq!(coordinate, position(9, 5));
}

#[test]
fn clamped_coordinate_within_content_is_unchanged() {
    let buffer = CellBuffer::new(10, 6);
    let event = mouse(MouseEventKind::Drag(MouseButton::Left), 5, 2);
    let coordinate = clamped_document_coordinate(&event, &buffer, 1, BODY_AREA_TOP_ROW);
    assert_eq!(coordinate, position(5 - CONTENT_PADDING, 3));
}

#[test]
fn clamped_coordinate_on_empty_buffer_does_not_panic() {
    let buffer = CellBuffer::new(0, 0);
    let event = mouse(MouseEventKind::Drag(MouseButton::Left), 50, 50);
    let coordinate = clamped_document_coordinate(&event, &buffer, 0, BODY_AREA_TOP_ROW);
    assert_eq!(coordinate, position(0, 0));
}

#[test]
fn press_begins_a_selection_without_a_range() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    handle_mouse_event(
        mouse(MouseEventKind::Down(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(),
        Instant::now(),
    );
    assert!(selection.is_dragging());
    assert!(!selection.has_moved());
    assert_eq!(selection.range(), None);
    assert_eq!(navigate_to_url, None);
}

#[test]
fn press_drag_release_keeps_the_highlighted_range() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    let steps = [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ];
    let columns = [3, 7, 7];
    for (kind, column) in steps.into_iter().zip(columns) {
        handle_mouse_event(
            mouse(kind, column, 2),
            Some(&buffer),
            0,
            BODY_AREA_TOP_ROW,
            &mut selection,
            &mut navigate_to_url,
            &mut UiState::new(),
            Instant::now(),
        );
    }
    assert!(selection.has_moved());
    assert_eq!(
        selection.range(),
        Some((
            position(3 - CONTENT_PADDING, 2),
            position(7 - CONTENT_PADDING, 2)
        ))
    );
    assert_eq!(navigate_to_url, None);
}

#[test]
fn press_release_without_movement_clears_the_selection() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    handle_mouse_event(
        mouse(MouseEventKind::Down(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(),
        Instant::now(),
    );
    handle_mouse_event(
        mouse(MouseEventKind::Up(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(),
        Instant::now(),
    );
    assert!(!selection.is_dragging());
    assert_eq!(selection.range(), None);
    assert_eq!(navigate_to_url, None);
}

#[test]
fn a_drag_without_a_prior_press_is_ignored() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    handle_mouse_event(
        mouse(MouseEventKind::Drag(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(),
        Instant::now(),
    );
    assert!(!selection.is_dragging());
    assert_eq!(selection.range(), None);
}

#[test]
fn wheel_scroll_events_do_not_touch_the_selection() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    handle_mouse_event(
        mouse(MouseEventKind::Down(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(),
        Instant::now(),
    );
    handle_mouse_event(
        mouse(MouseEventKind::ScrollDown, 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(),
        Instant::now(),
    );
    assert!(selection.is_dragging());
    assert!(!selection.has_moved());
}

#[test]
fn cell_is_selected_covers_the_single_row_span_inclusively() {
    let range = Some((position(2, 1), position(5, 1)));
    assert!(!cell_is_selected(1, 1, range));
    assert!(cell_is_selected(2, 1, range));
    assert!(cell_is_selected(5, 1, range));
    assert!(!cell_is_selected(6, 1, range));
    assert!(!cell_is_selected(3, 0, range));
}

#[test]
fn cell_is_selected_covers_interior_rows_fully_and_ends_partially() {
    let range = Some((position(4, 1), position(3, 3)));
    assert!(!cell_is_selected(3, 1, range));
    assert!(cell_is_selected(4, 1, range));
    assert!(cell_is_selected(0, 2, range));
    assert!(cell_is_selected(9, 2, range));
    assert!(cell_is_selected(3, 3, range));
    assert!(!cell_is_selected(4, 3, range));
}

#[test]
fn cell_is_selected_is_false_without_a_range() {
    assert!(!cell_is_selected(3, 1, None));
}

#[test]
fn copied_message_reports_the_ascii_grapheme_count() {
    assert_eq!(copied_message("hello"), "copied 5 chars to clipboard");
}

#[test]
fn copied_message_counts_multibyte_characters_as_one_grapheme_each() {
    // "café" is five UTF-8 bytes but four grapheme clusters; a combined-emoji family is
    // many bytes yet a single grapheme. The count must be graphemes, not bytes.
    assert_eq!(copied_message("café"), "copied 4 chars to clipboard");
    assert_eq!(copied_message("👨‍👩‍👧"), "copied 1 chars to clipboard");
}

#[test]
fn copied_message_reports_zero_for_empty_text() {
    assert_eq!(copied_message(""), "copied 0 chars to clipboard");
}
