// @file crates/browser-terminal/src/input_tests.rs
// @description Unit tests for key-event to input-action mapping and arm transitions.
// @layer terminal
// @created meerita <meerita@icloud.com>

use super::{map_key_event, quit_armed_after, refresh_armed_after, InputAction};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

#[test]
fn esc_from_disarmed_arms_the_quit() {
    assert_eq!(
        map_key_event(key(KeyCode::Esc), false, false, false),
        InputAction::ArmQuit
    );
}

#[test]
fn esc_from_armed_yields_the_quit_action() {
    assert_eq!(
        map_key_event(key(KeyCode::Esc), true, false, false),
        InputAction::Quit
    );
}

#[test]
fn ctrl_c_yields_the_quit_action() {
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(map_key_event(event, false, false, false), InputAction::Quit);
}

#[test]
fn a_non_esc_key_while_armed_disarms_the_quit() {
    let action = map_key_event(key(KeyCode::Char('j')), true, false, false);
    assert_eq!(action, InputAction::ScrollLineDown);
    assert!(!quit_armed_after(action));
}

#[test]
fn arming_keeps_the_quit_armed_for_the_next_key() {
    assert!(quit_armed_after(InputAction::ArmQuit));
}

#[test]
fn r_from_disarmed_arms_the_refresh() {
    assert_eq!(
        map_key_event(key(KeyCode::Char('r')), false, false, false),
        InputAction::ArmRefresh
    );
}

#[test]
fn r_from_armed_yields_the_refresh_armed_action() {
    assert_eq!(
        map_key_event(key(KeyCode::Char('r')), false, true, false),
        InputAction::RefreshArmed
    );
}

#[test]
fn arming_refresh_keeps_the_flag_set_for_the_next_key() {
    assert!(refresh_armed_after(InputAction::ArmRefresh));
}

#[test]
fn arrow_and_letter_keys_map_to_line_scrolls() {
    assert_eq!(
        map_key_event(key(KeyCode::Down), false, false, false),
        InputAction::ScrollLineDown
    );
    assert_eq!(
        map_key_event(key(KeyCode::Char('j')), false, false, false),
        InputAction::ScrollLineDown
    );
    assert_eq!(
        map_key_event(key(KeyCode::Up), false, false, false),
        InputAction::ScrollLineUp
    );
    assert_eq!(
        map_key_event(key(KeyCode::Char('k')), false, false, false),
        InputAction::ScrollLineUp
    );
}

#[test]
fn page_keys_map_to_page_scrolls() {
    assert_eq!(
        map_key_event(key(KeyCode::PageDown), false, false, false),
        InputAction::ScrollPageDown
    );
    assert_eq!(
        map_key_event(key(KeyCode::PageUp), false, false, false),
        InputAction::ScrollPageUp
    );
}

#[test]
fn g_keys_map_to_top_and_bottom() {
    assert_eq!(
        map_key_event(key(KeyCode::Char('g')), false, false, false),
        InputAction::ScrollToTop
    );
    assert_eq!(
        map_key_event(key(KeyCode::Char('G')), false, false, false),
        InputAction::ScrollToBottom
    );
}

#[test]
fn unbound_printable_char_in_reading_mode_enters_command_mode() {
    assert_eq!(
        map_key_event(key(KeyCode::Char('u')), false, false, false),
        InputAction::EnterCommand('u')
    );
}

#[test]
fn unbound_printable_char_with_upper_case_in_reading_mode_enters_command_mode() {
    assert_eq!(
        map_key_event(key(KeyCode::Char('H')), false, false, false),
        InputAction::EnterCommand('H')
    );
}

#[test]
fn ctrl_c_in_command_mode_yields_quit() {
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(map_key_event(event, false, false, true), InputAction::Quit);
}

#[test]
fn esc_in_command_mode_cancels() {
    assert_eq!(
        map_key_event(key(KeyCode::Esc), false, false, true),
        InputAction::CommandCancel
    );
}

#[test]
fn enter_in_command_mode_submits() {
    assert_eq!(
        map_key_event(key(KeyCode::Enter), false, false, true),
        InputAction::CommandSubmit
    );
}

#[test]
fn left_in_command_mode_moves_cursor_left() {
    assert_eq!(
        map_key_event(key(KeyCode::Left), false, false, true),
        InputAction::CommandMoveCursorLeft
    );
}

#[test]
fn right_in_command_mode_moves_cursor_right() {
    assert_eq!(
        map_key_event(key(KeyCode::Right), false, false, true),
        InputAction::CommandMoveCursorRight
    );
}

#[test]
fn backspace_in_command_mode_deletes_back() {
    assert_eq!(
        map_key_event(key(KeyCode::Backspace), false, false, true),
        InputAction::CommandDeleteBack
    );
}

#[test]
fn printable_char_in_command_mode_appends() {
    assert_eq!(
        map_key_event(key(KeyCode::Char('x')), false, false, true),
        InputAction::CommandAppend('x')
    );
}
