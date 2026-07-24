// @file crates/browser-terminal/src/ui_state_tests.rs
// @description Unit tests for UiState hint rotation, transient hint lifecycle, and command mode.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::time::{Duration, Instant};

use super::{UiState, READING_HINTS};

#[test]
fn new_state_shows_first_reading_hint() {
    let state = UiState::new();
    assert_eq!(state.current_hint(), READING_HINTS[0]);
}

#[test]
fn advance_hint_if_due_rotates_to_next_hint_after_thirty_seconds() {
    let mut state = UiState::new();
    let future = Instant::now() + Duration::from_secs(31);
    state.advance_hint_if_due(future);
    assert_eq!(state.current_hint(), READING_HINTS[1]);
}

#[test]
fn advance_hint_if_due_does_not_rotate_before_thirty_seconds() {
    let mut state = UiState::new();
    let soon = Instant::now() + Duration::from_secs(1);
    state.advance_hint_if_due(soon);
    assert_eq!(state.current_hint(), READING_HINTS[0]);
}

#[test]
fn transient_hint_overrides_the_rotating_hint() {
    let mut state = UiState::new();
    state.set_transient_hint("Press Esc again to quit", Instant::now());
    assert_eq!(state.current_hint(), "Press Esc again to quit");
}

#[test]
fn transient_hint_clears_after_five_seconds() {
    let mut state = UiState::new();
    let stale = Instant::now() - Duration::from_secs(6);
    state.set_transient_hint("Press Esc again to quit", stale);
    state.clear_transient_if_expired(Instant::now());
    assert_eq!(state.current_hint(), READING_HINTS[0]);
}

#[test]
fn clear_transient_restores_the_rotating_hint() {
    let mut state = UiState::new();
    state.set_transient_hint("Press r again to refresh", Instant::now());
    state.clear_transient();
    assert_eq!(state.current_hint(), READING_HINTS[0]);
}

#[test]
fn new_state_is_not_in_command_mode() {
    let state = UiState::new();
    assert!(!state.is_in_command_mode());
}

#[test]
fn entering_command_mode_seeds_buffer_with_first_char() {
    let mut state = UiState::new();
    state.enter_command_mode('h');
    assert!(state.is_in_command_mode());
    assert_eq!(state.command_buffer(), "h");
    assert_eq!(state.cursor_byte_offset(), 1);
}

#[test]
fn command_append_char_inserts_at_cursor_and_advances_offset() {
    let mut state = UiState::new();
    state.enter_command_mode('h');
    state.command_append_char('i');
    assert_eq!(state.command_buffer(), "hi");
    assert_eq!(state.cursor_byte_offset(), 2);
}

#[test]
fn command_append_char_inserts_at_mid_cursor_position() {
    let mut state = UiState::new();
    state.enter_command_mode('a');
    state.command_append_char('c');
    state.command_move_left();
    state.command_append_char('b');
    assert_eq!(state.command_buffer(), "abc");
    assert_eq!(state.cursor_byte_offset(), 2);
}

#[test]
fn command_move_left_moves_cursor_back_by_one_char() {
    let mut state = UiState::new();
    state.enter_command_mode('a');
    state.command_append_char('b');
    state.command_move_left();
    assert_eq!(state.cursor_byte_offset(), 1);
}

#[test]
fn command_move_left_at_start_is_a_no_op() {
    let mut state = UiState::new();
    state.enter_command_mode('x');
    state.command_move_left();
    state.command_move_left();
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn command_move_right_moves_cursor_forward_by_one_char() {
    let mut state = UiState::new();
    state.enter_command_mode('a');
    state.command_append_char('b');
    state.command_move_left();
    state.command_move_right();
    assert_eq!(state.cursor_byte_offset(), 2);
}

#[test]
fn command_move_right_at_end_is_a_no_op() {
    let mut state = UiState::new();
    state.enter_command_mode('x');
    state.command_move_right();
    assert_eq!(state.cursor_byte_offset(), 1);
}

#[test]
fn command_delete_before_cursor_removes_char_and_retreats_offset() {
    let mut state = UiState::new();
    state.enter_command_mode('a');
    state.command_append_char('b');
    state.command_delete_before_cursor();
    assert_eq!(state.command_buffer(), "a");
    assert_eq!(state.cursor_byte_offset(), 1);
}

#[test]
fn command_delete_before_cursor_at_start_is_a_no_op() {
    let mut state = UiState::new();
    state.enter_command_mode('x');
    state.command_move_left();
    state.command_delete_before_cursor();
    assert_eq!(state.command_buffer(), "x");
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn cancel_command_mode_clears_buffer_and_returns_to_reading() {
    let mut state = UiState::new();
    state.enter_command_mode('h');
    state.command_append_char('i');
    state.cancel_command_mode();
    assert!(!state.is_in_command_mode());
    assert_eq!(state.command_buffer(), "");
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn take_command_buffer_returns_content_and_returns_to_reading() {
    let mut state = UiState::new();
    state.enter_command_mode('h');
    state.command_append_char('i');
    let url = state.take_command_buffer();
    assert_eq!(url, "hi");
    assert!(!state.is_in_command_mode());
    assert_eq!(state.command_buffer(), "");
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn command_append_char_handles_multibyte_unicode_correctly() {
    let mut state = UiState::new();
    state.enter_command_mode('é');
    assert_eq!(state.cursor_byte_offset(), 'é'.len_utf8());
    state.command_delete_before_cursor();
    assert_eq!(state.command_buffer(), "");
    assert_eq!(state.cursor_byte_offset(), 0);
}
