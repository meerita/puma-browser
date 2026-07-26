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
fn transient_message_overrides_the_rotating_hint() {
    let mut state = UiState::new();
    state.set_transient_message("copied 12 chars to clipboard".to_string(), Instant::now());
    assert_eq!(state.current_hint(), "copied 12 chars to clipboard");
    assert_eq!(
        state.transient_message(),
        Some("copied 12 chars to clipboard")
    );
}

#[test]
fn transient_message_clears_after_five_seconds() {
    let mut state = UiState::new();
    let stale = Instant::now() - Duration::from_secs(6);
    state.set_transient_message("copied 3 chars to clipboard".to_string(), stale);
    state.clear_transient_if_expired(Instant::now());
    assert_eq!(state.current_hint(), READING_HINTS[0]);
    assert_eq!(state.transient_message(), None);
}

#[test]
fn transient_message_survives_before_five_seconds() {
    let mut state = UiState::new();
    let recent = Instant::now() - Duration::from_secs(2);
    state.set_transient_message("copied 7 chars to clipboard".to_string(), recent);
    state.clear_transient_if_expired(Instant::now());
    assert_eq!(
        state.transient_message(),
        Some("copied 7 chars to clipboard")
    );
}

#[test]
fn new_state_has_no_transient_message() {
    let state = UiState::new();
    assert_eq!(state.transient_message(), None);
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

#[test]
fn focus_next_link_advances_index() {
    let mut state = UiState::new();
    state.focus_next_link(3);
    assert_eq!(state.focused_link_index, Some(0));
    state.focus_next_link(3);
    assert_eq!(state.focused_link_index, Some(1));
    state.focus_next_link(3);
    assert_eq!(state.focused_link_index, Some(2));
    state.focus_next_link(3);
    assert_eq!(state.focused_link_index, Some(0));
}

#[test]
fn focus_previous_link_wraps() {
    let mut state = UiState::new();
    state.focus_previous_link(3);
    assert_eq!(state.focused_link_index, Some(2));
}

#[test]
fn enter_and_exit_link_navigation() {
    let mut state = UiState::new();
    state.enter_link_navigation(1);
    assert!(state.is_in_link_navigation());
    assert_eq!(state.focused_link_index, Some(1));
    state.exit_link_navigation();
    assert!(!state.is_in_link_navigation());
    assert_eq!(state.focused_link_index, None);
}

#[test]
fn mark_visited_and_is_visited() {
    let mut state = UiState::new();
    state.mark_visited("https://a.test/");
    assert!(state.is_visited("https://a.test/"));
    assert!(!state.is_visited("https://b.test/"));
}

#[test]
fn new_state_has_no_palette_matches() {
    let state = UiState::new();
    assert!(!state.is_palette_active());
    assert!(state.palette_matches().is_empty());
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn entering_slash_fills_the_palette_with_all_commands() {
    let mut state = UiState::new();
    state.enter_command_mode('/');
    assert!(state.is_palette_active());
    assert_eq!(state.palette_matches().len(), 6);
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn typing_filters_the_palette_and_resets_selection() {
    let mut state = UiState::new();
    state.enter_command_mode('/');
    state.command_append_char('r');
    assert_eq!(state.palette_matches().len(), 1);
    assert_eq!(state.palette_matches()[0].spec.name, "reload");
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn deleting_a_character_rewidens_the_palette() {
    let mut state = UiState::new();
    state.enter_command_mode('/');
    state.command_append_char('r');
    assert_eq!(state.palette_matches().len(), 1);
    state.command_delete_before_cursor();
    assert_eq!(state.palette_matches().len(), 6);
}

#[test]
fn no_matching_command_leaves_an_empty_palette() {
    let mut state = UiState::new();
    state.enter_command_mode('/');
    state.command_append_char('z');
    state.command_append_char('z');
    assert!(state.palette_matches().is_empty());
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn non_slash_buffer_is_not_palette_active() {
    let mut state = UiState::new();
    state.enter_command_mode('h');
    assert!(!state.is_palette_active());
    assert!(state.palette_matches().is_empty());
}

#[test]
fn cancel_command_mode_clears_the_palette() {
    let mut state = UiState::new();
    state.enter_command_mode('/');
    state.cancel_command_mode();
    assert!(!state.is_palette_active());
    assert!(state.palette_matches().is_empty());
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn take_command_buffer_clears_the_palette() {
    let mut state = UiState::new();
    state.enter_command_mode('/');
    state.command_append_char('o');
    let buffer = state.take_command_buffer();
    assert_eq!(buffer, "/o");
    assert!(state.palette_matches().is_empty());
    assert_eq!(state.palette_selected(), 0);
}
