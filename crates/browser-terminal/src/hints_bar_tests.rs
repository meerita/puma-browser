// @file crates/browser-terminal/src/hints_bar_tests.rs
// @description Tests for hints bar composition.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

use super::compose_hints_bar;

#[test]
fn no_system_message_shows_shortcuts_only() {
    let result = compose_hints_bar(None, 80);
    assert!(
        result.starts_with("? for shortcuts · r for refresh"),
        "unexpected content: {result}"
    );
}

#[test]
fn system_message_appears_on_right() {
    let result = compose_hints_bar(Some("Loading…"), 80);
    assert!(
        result.ends_with("Loading…"),
        "expected message on right: {result}"
    );
    assert!(
        result.starts_with("? for shortcuts · r for refresh"),
        "expected shortcuts on left: {result}"
    );
}

#[test]
fn composed_string_width_equals_terminal_width() {
    for terminal_width in [40u16, 80, 120] {
        for msg in [None, Some("Status")] {
            let result = compose_hints_bar(msg, terminal_width);
            let actual_width = UnicodeWidthStr::width(result.as_str());
            assert_eq!(
                actual_width, terminal_width as usize,
                "width mismatch for terminal_width={terminal_width}: got {actual_width}"
            );
        }
    }
}
