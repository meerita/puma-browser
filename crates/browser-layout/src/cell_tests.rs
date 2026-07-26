// @file crates/browser-layout/src/cell_tests.rs
// @description Tests for CellBuffer::text_in_range linear text extraction.
// @layer layout
// @created meerita <meerita@icloud.com>

use super::{Cell, CellBuffer, CellPosition};
use browser_css::TextStyle;
use unicode_segmentation::UnicodeSegmentation;

/// Builds a buffer of the given size and writes each line's grapheme clusters into
/// successive cells starting at column zero. Unwritten cells stay blank.
fn buffer_with_lines(width: u16, height: u16, lines: &[&str]) -> CellBuffer {
    let mut buffer = CellBuffer::new(width, height);
    let style = TextStyle::default();
    for (row, line) in (0_u16..).zip(lines.iter()) {
        for (column, grapheme) in (0_u16..).zip(line.graphemes(true)) {
            buffer.set_cell(column, row, Cell::new(grapheme.to_string(), &style));
        }
    }
    buffer
}

fn position(column: u16, row: u16) -> CellPosition {
    CellPosition { column, row }
}

#[test]
fn single_row_selection_returns_inclusive_columns() {
    let buffer = buffer_with_lines(20, 1, &["Hello world"]);
    let text = buffer.text_in_range(position(0, 0), position(4, 0));
    assert_eq!(text, "Hello");
}

#[test]
fn multi_row_selection_spans_first_interior_and_last_rows() {
    let buffer = buffer_with_lines(20, 2, &["Hello world", "second row"]);
    let text = buffer.text_in_range(position(6, 0), position(5, 1));
    assert_eq!(text, "world\nsecond");
}

#[test]
fn interior_rows_are_taken_in_full() {
    let buffer = buffer_with_lines(10, 3, &["abc", "def", "ghi"]);
    let text = buffer.text_in_range(position(1, 0), position(1, 2));
    assert_eq!(text, "bc\ndef\ngh");
}

#[test]
fn reversed_order_yields_same_text_as_forward_order() {
    let buffer = buffer_with_lines(20, 2, &["Hello world", "second row"]);
    let forward = buffer.text_in_range(position(6, 0), position(5, 1));
    let reversed = buffer.text_in_range(position(5, 1), position(6, 0));
    assert_eq!(reversed, forward);
}

#[test]
fn trailing_blank_cells_are_trimmed_per_row() {
    let buffer = buffer_with_lines(10, 1, &["abc"]);
    let text = buffer.text_in_range(position(0, 0), position(9, 0));
    assert_eq!(text, "abc");
}

#[test]
fn multi_byte_grapheme_cluster_is_preserved_intact() {
    let combining_e_acute = "e\u{0301}";
    let line = format!("x{combining_e_acute}y");
    let buffer = buffer_with_lines(10, 1, &[line.as_str()]);
    let text = buffer.text_in_range(position(0, 0), position(2, 0));
    assert_eq!(text, format!("x{combining_e_acute}y"));
}

#[test]
fn columns_beyond_width_are_clamped_on_a_single_row() {
    let buffer = buffer_with_lines(5, 1, &["hi"]);
    let text = buffer.text_in_range(position(0, 0), position(100, 0));
    assert_eq!(text, "hi");
}

#[test]
fn selection_starting_below_the_buffer_returns_empty() {
    let buffer = buffer_with_lines(5, 2, &["hi", "yo"]);
    let text = buffer.text_in_range(position(0, 10), position(0, 11));
    assert_eq!(text, "");
}

#[test]
fn empty_buffer_returns_empty_string() {
    let buffer = CellBuffer::new(0, 0);
    let text = buffer.text_in_range(position(0, 0), position(3, 3));
    assert_eq!(text, "");
}

#[test]
fn blank_rows_are_trimmed_to_empty_lines_joined_by_newline() {
    let buffer = CellBuffer::new(5, 2);
    let text = buffer.text_in_range(position(0, 0), position(4, 1));
    assert_eq!(text, "\n");
}
