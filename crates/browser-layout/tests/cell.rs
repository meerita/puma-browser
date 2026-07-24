// @file crates/browser-layout/tests/cell.rs
// @description Behavior tests for Cell construction and attribute derivation from TextStyle.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::{Color, Emphasis, TextStyle};
use browser_layout::Cell;

#[test]
fn blank_cell_holds_a_space_with_no_attributes() {
    let cell = Cell::blank();
    assert_eq!(cell.grapheme(), " ");
    assert_eq!(cell.foreground(), None);
    assert_eq!(cell.background(), None);
    assert_eq!(cell.emphasis(), Emphasis::None);
    assert!(!cell.underline());
}

#[test]
fn new_cell_carries_the_underline_flag_from_style() {
    let underlined = TextStyle {
        underline: true,
        ..TextStyle::default()
    };
    assert!(Cell::new(String::from("a"), &underlined).underline());
    assert!(!Cell::new(String::from("a"), &TextStyle::default()).underline());
}

#[test]
fn new_cell_derives_attributes_from_text_style() {
    let style = TextStyle {
        foreground: Some(Color::Red),
        background: Some(Color::Black),
        emphasis: Emphasis::Bold,
        ..TextStyle::default()
    };
    let cell = Cell::new(String::from("a"), &style);
    assert_eq!(cell.grapheme(), "a");
    assert_eq!(cell.foreground(), Some(Color::Red));
    assert_eq!(cell.background(), Some(Color::Black));
    assert_eq!(cell.emphasis(), Emphasis::Bold);
}

#[test]
fn new_cell_preserves_a_multi_scalar_grapheme_cluster() {
    let grapheme = String::from("e\u{0301}");
    let cell = Cell::new(grapheme.clone(), &TextStyle::default());
    assert_eq!(cell.grapheme(), grapheme);
}
