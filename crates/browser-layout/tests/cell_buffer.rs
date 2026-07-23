// @file crates/browser-layout/tests/cell_buffer.rs
// @description Behavior tests for CellBuffer construction and bounds-checked access.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_layout::CellBuffer;

#[test]
fn new_buffer_reports_requested_dimensions() {
    let buffer = CellBuffer::new(80, 24);
    assert_eq!(buffer.width(), 80);
    assert_eq!(buffer.height(), 24);
}

#[test]
fn new_buffer_is_filled_with_blank_cells() {
    let buffer = CellBuffer::new(3, 2);
    let cell = buffer.cell_at(0, 0).expect("origin cell must exist");
    assert_eq!(cell.grapheme(), " ");
}

#[test]
fn cell_at_returns_some_inside_bounds() {
    let buffer = CellBuffer::new(4, 3);
    assert!(buffer.cell_at(0, 0).is_some());
    assert!(buffer.cell_at(3, 2).is_some());
}

#[test]
fn cell_at_returns_none_outside_bounds() {
    let buffer = CellBuffer::new(4, 3);
    assert!(buffer.cell_at(4, 0).is_none());
    assert!(buffer.cell_at(0, 3).is_none());
}

#[test]
fn cell_at_on_zero_sized_buffer_is_always_none() {
    let buffer = CellBuffer::new(0, 0);
    assert!(buffer.cell_at(0, 0).is_none());
}
