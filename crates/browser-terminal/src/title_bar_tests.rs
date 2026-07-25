// @file crates/browser-terminal/src/title_bar_tests.rs
// @description Tests for title bar composition and truncation logic.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

use super::{compose_title_bar, format_page_size, truncate_with_ellipsis};

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
    let result = compose_title_bar("Page", 50, 0, 0, 40);
    assert!(
        !result.contains("script"),
        "unexpected script segment: {result}"
    );
}

#[test]
fn script_count_one_uses_singular() {
    let result = compose_title_bar("Page", 50, 1, 0, 80);
    assert!(
        result.contains("1 script blocked"),
        "expected singular: {result}"
    );
    assert!(!result.contains("scripts"), "unexpected plural: {result}");
}

#[test]
fn script_count_many_uses_plural() {
    let result = compose_title_bar("Page", 50, 5, 0, 80);
    assert!(
        result.contains("5 scripts blocked"),
        "expected plural: {result}"
    );
}

#[test]
fn composed_string_width_equals_terminal_width() {
    for terminal_width in [40u16, 80, 120, 200] {
        let result = compose_title_bar(
            "Some Page Title That May Be Long",
            75,
            3,
            1536,
            terminal_width,
        );
        let actual_width = UnicodeWidthStr::width(result.as_str());
        assert_eq!(
            actual_width, terminal_width as usize,
            "width mismatch for terminal_width={terminal_width}: got {actual_width}"
        );
    }
}

#[test]
fn page_size_zero_bytes_omits_size_segment() {
    let result = compose_title_bar("Page", 0, 0, 0, 80);
    assert!(
        !result.contains(" B") && !result.contains("KB") && !result.contains("MB"),
        "size segment should be absent when byte_count is zero: {result}"
    );
}

#[test]
fn page_size_shown_when_byte_count_is_nonzero() {
    let result = compose_title_bar("Page", 50, 0, 2048, 80);
    assert!(
        result.contains("↓ 2.0 KB"),
        "expected page size in indicators: {result}"
    );
}

#[test]
fn format_page_size_below_one_kb_shows_bytes() {
    assert_eq!(format_page_size(0), "↓ 0 B");
    assert_eq!(format_page_size(512), "↓ 512 B");
    assert_eq!(format_page_size(1023), "↓ 1023 B");
}

#[test]
fn format_page_size_in_kb_range_shows_one_decimal() {
    assert_eq!(format_page_size(1024), "↓ 1.0 KB");
    assert_eq!(format_page_size(1536), "↓ 1.5 KB");
    assert_eq!(format_page_size(1024 * 1024 - 1), "↓ 1024.0 KB");
}

#[test]
fn format_page_size_one_mb_and_above_shows_mb() {
    assert_eq!(format_page_size(1024 * 1024), "↓ 1.0 MB");
    assert_eq!(format_page_size(2 * 1024 * 1024), "↓ 2.0 MB");
}
