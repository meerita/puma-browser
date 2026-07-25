// @file crates/browser-terminal/src/command_bar_tests.rs
// @description Tests for command bar composition in reading and command modes.
// @layer terminal
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

use super::{
    command_cursor_col, compose_command_bar_command, compose_command_bar_loading,
    compose_command_bar_reading,
};

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

#[test]
fn command_mode_shows_buffer_with_prompt_prefix() {
    let result = compose_command_bar_command("https://example.com", 80);
    assert!(
        result.starts_with("> https://example.com"),
        "unexpected content: {result}"
    );
}

#[test]
fn command_mode_empty_buffer_shows_prompt_only() {
    let result = compose_command_bar_command("", 80);
    assert!(result.starts_with("> "), "unexpected content: {result}");
}

#[test]
fn command_mode_string_width_equals_terminal_width() {
    let buffers = ["", "https://example.com", "x"];
    for terminal_width in [40u16, 80, 120] {
        for buffer in buffers {
            let result = compose_command_bar_command(buffer, terminal_width);
            let actual_width = UnicodeWidthStr::width(result.as_str());
            assert_eq!(
                actual_width,
                terminal_width as usize,
                "width mismatch for terminal_width={terminal_width} buffer={buffer:?}: got {actual_width}"
            );
        }
    }
}

#[test]
fn command_cursor_col_is_two_for_empty_buffer() {
    assert_eq!(command_cursor_col("", 0), 2);
}

#[test]
fn command_cursor_col_advances_one_per_ascii_char() {
    assert_eq!(command_cursor_col("abc", 1), 3);
    assert_eq!(command_cursor_col("abc", 2), 4);
    assert_eq!(command_cursor_col("abc", 3), 5);
}

#[test]
fn command_cursor_col_accounts_for_multibyte_chars_by_byte_offset() {
    let ch = 'é';
    let byte_len = ch.len_utf8();
    let buffer = ch.to_string();
    assert_eq!(command_cursor_col(&buffer, byte_len), 3);
}

#[test]
fn loading_bar_shows_spinner_url_and_kb() {
    let result = compose_command_bar_loading(0, "https://example.com", 42 * 1024, 80);
    assert!(
        result.contains("⠋"),
        "expected spinner char for frame 0: {result}"
    );
    assert!(
        result.contains("https://example.com"),
        "expected URL in loading bar: {result}"
    );
    assert!(
        result.contains("↓ 42 KB"),
        "expected KB count in loading bar: {result}"
    );
}

#[test]
fn loading_bar_advances_spinner_frame() {
    let frame0 = compose_command_bar_loading(0, "https://example.com", 0, 80);
    let frame1 = compose_command_bar_loading(1, "https://example.com", 0, 80);
    assert_ne!(
        frame0, frame1,
        "consecutive spinner frames must produce different output"
    );
}
