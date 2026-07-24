// @file crates/browser-terminal/src/command_bar_tests.rs
// @description Tests for command bar composition in reading mode.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

use super::compose_command_bar_reading;

#[test]
fn reading_mode_shows_url_hint_when_not_armed() {
    let result = compose_command_bar_reading(false, 80);
    assert!(
        result.starts_with("> Type a URL or press / for commands"),
        "unexpected content: {result}"
    );
}

#[test]
fn reading_mode_shows_quit_hint_when_armed() {
    let result = compose_command_bar_reading(true, 80);
    assert!(
        result.starts_with("> Press Esc again to quit"),
        "unexpected content: {result}"
    );
}

#[test]
fn composed_string_width_equals_terminal_width() {
    for terminal_width in [40u16, 80, 120] {
        for armed in [false, true] {
            let result = compose_command_bar_reading(armed, terminal_width);
            let actual_width = UnicodeWidthStr::width(result.as_str());
            assert_eq!(
                actual_width,
                terminal_width as usize,
                "width mismatch for terminal_width={terminal_width} armed={armed}: got {actual_width}"
            );
        }
    }
}
