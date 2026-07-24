// @file crates/browser-terminal/src/title_bar_tests.rs
// @description Tests for title bar composition and truncation logic.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

use super::{compose_title_bar, truncate_with_ellipsis};

#[test]
fn ascii_title_fits_without_truncation() {
    let result = truncate_with_ellipsis("Hello", 10);
    assert_eq!(result, "Hello");
}

#[test]
fn long_title_is_truncated_with_ellipsis() {
    let result = truncate_with_ellipsis("Hello World", 8);
    assert!(result.ends_with('…'), "expected ellipsis: {result}");
    assert!(UnicodeWidthStr::width(result.as_str()) <= 8);
}

#[test]
fn script_count_zero_omits_script_segment() {
    let result = compose_title_bar("Page", 50, 0, 40);
    assert!(
        !result.contains("script"),
        "unexpected script segment: {result}"
    );
}

#[test]
fn script_count_one_uses_singular() {
    let result = compose_title_bar("Page", 50, 1, 80);
    assert!(
        result.contains("1 script blocked"),
        "expected singular: {result}"
    );
    assert!(!result.contains("scripts"), "unexpected plural: {result}");
}

#[test]
fn script_count_many_uses_plural() {
    let result = compose_title_bar("Page", 50, 5, 80);
    assert!(
        result.contains("5 scripts blocked"),
        "expected plural: {result}"
    );
}

#[test]
fn composed_string_width_equals_terminal_width() {
    for terminal_width in [40u16, 80, 120, 200] {
        let result = compose_title_bar("Some Page Title That May Be Long", 75, 3, terminal_width);
        let actual_width = UnicodeWidthStr::width(result.as_str());
        assert_eq!(
            actual_width, terminal_width as usize,
            "width mismatch for terminal_width={terminal_width}: got {actual_width}"
        );
    }
}
