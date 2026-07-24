// @file crates/browser-terminal/src/command_bar_tests.rs
// @description Tests for command bar composition in reading mode.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

use super::compose_command_bar_reading;

#[test]
fn reading_mode_shows_hint_with_prompt_prefix() {
    let result = compose_command_bar_reading("Type a URL or press / for commands", 80);
    assert!(
        result.starts_with("> Type a URL or press / for commands"),
        "unexpected content: {result}"
    );
}

#[test]
fn quit_hint_renders_with_prompt_prefix() {
    let result = compose_command_bar_reading("Press Esc again to quit", 80);
    assert!(
        result.starts_with("> Press Esc again to quit"),
        "unexpected content: {result}"
    );
}

#[test]
fn composed_string_width_equals_terminal_width() {
    let hints = [
        "Type a URL or press / for commands",
        "Press Esc again to quit",
        "Press r again to refresh",
    ];
    for terminal_width in [40u16, 80, 120] {
        for hint in hints {
            let result = compose_command_bar_reading(hint, terminal_width);
            let actual_width = UnicodeWidthStr::width(result.as_str());
            assert_eq!(
                actual_width,
                terminal_width as usize,
                "width mismatch for terminal_width={terminal_width} hint={hint:?}: got {actual_width}"
            );
        }
    }
}
