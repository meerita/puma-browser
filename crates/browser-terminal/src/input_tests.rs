// @file crates/browser-terminal/src/input_tests.rs
// @description Unit tests for key-event to input-action mapping and arm transitions.
// @layer terminal
// @created meerita <meerita@icloud.com>

use super::{map_key_event, quit_armed_after, refresh_armed_after, InputAction};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::empty())
}

/// Maps a key in reading mode with the given arm flags and no other mode active.
fn reading(code: KeyCode, quit_armed: bool, refresh_armed: bool) -> InputAction {
    map_key_event(
        key(code),
        quit_armed,
        refresh_armed,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
    )
}

/// Maps a key while a plain (non-palette) command buffer is being edited, so the
/// command-mode base bindings apply.
fn command_mode(code: KeyCode) -> InputAction {
    map_key_event(
        key(code),
        false,
        false,
        true,
        false,
        false,
        false,
        false,
        false,
        false,
    )
}

/// Maps a key while the slash-command palette is active on top of command mode.
fn palette_mode(code: KeyCode) -> InputAction {
    map_key_event(
        key(code),
        false,
        false,
        true,
        false,
        true,
        false,
        false,
        false,
        false,
    )
}

/// Maps a key while address-bar suggestions are offered over a non-slash command buffer.
fn suggestion_mode(code: KeyCode) -> InputAction {
    map_key_event(
        key(code),
        false,
        false,
        true,
        false,
        false,
        true,
        false,
        false,
        false,
    )
}

/// Maps a key while the history list is open.
fn history_mode(code: KeyCode) -> InputAction {
    map_key_event(
        key(code),
        false,
        false,
        false,
        false,
        false,
        false,
        true,
        false,
        false,
    )
}

/// Maps a key while the cookie inspection popup is open.
fn cookies_mode(code: KeyCode) -> InputAction {
    map_key_event(
        key(code),
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        true,
        false,
    )
}

/// Maps a key while the settings panel is open.
fn settings_mode(code: KeyCode) -> InputAction {
    map_key_event(
        key(code),
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        false,
        true,
    )
}

/// Maps a key while a link is focused (link-navigation mode).
fn link_navigation(code: KeyCode) -> InputAction {
    map_key_event(
        key(code),
        false,
        false,
        false,
        true,
        false,
        false,
        false,
        false,
        false,
    )
}

#[test]
fn esc_from_disarmed_arms_the_quit() {
    assert_eq!(reading(KeyCode::Esc, false, false), InputAction::ArmQuit);
}

#[test]
fn esc_from_armed_yields_the_quit_action() {
    assert_eq!(reading(KeyCode::Esc, true, false), InputAction::Quit);
}

#[test]
fn ctrl_c_yields_the_quit_action() {
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        map_key_event(event, false, false, false, false, false, false, false, false, false),
        InputAction::Quit
    );
}

#[test]
fn a_non_esc_key_while_armed_disarms_the_quit() {
    let action = reading(KeyCode::Char('j'), true, false);
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
        reading(KeyCode::Char('r'), false, false),
        InputAction::ArmRefresh
    );
}

#[test]
fn r_from_armed_yields_the_refresh_armed_action() {
    assert_eq!(
        reading(KeyCode::Char('r'), false, true),
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
        reading(KeyCode::Down, false, false),
        InputAction::ScrollLineDown
    );
    assert_eq!(
        reading(KeyCode::Char('j'), false, false),
        InputAction::ScrollLineDown
    );
    assert_eq!(
        reading(KeyCode::Up, false, false),
        InputAction::ScrollLineUp
    );
    assert_eq!(
        reading(KeyCode::Char('k'), false, false),
        InputAction::ScrollLineUp
    );
}

#[test]
fn page_keys_map_to_page_scrolls() {
    assert_eq!(
        reading(KeyCode::PageDown, false, false),
        InputAction::ScrollPageDown
    );
    assert_eq!(
        reading(KeyCode::PageUp, false, false),
        InputAction::ScrollPageUp
    );
}

#[test]
fn g_keys_map_to_top_and_bottom() {
    assert_eq!(
        reading(KeyCode::Char('g'), false, false),
        InputAction::ScrollToTop
    );
    assert_eq!(
        reading(KeyCode::Char('G'), false, false),
        InputAction::ScrollToBottom
    );
}

#[test]
fn unbound_printable_char_in_reading_mode_enters_command_mode() {
    assert_eq!(
        reading(KeyCode::Char('u'), false, false),
        InputAction::EnterCommand('u')
    );
}

#[test]
fn unbound_printable_char_with_upper_case_in_reading_mode_enters_command_mode() {
    assert_eq!(
        reading(KeyCode::Char('H'), false, false),
        InputAction::EnterCommand('H')
    );
}

#[test]
fn ctrl_c_in_command_mode_yields_quit() {
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        map_key_event(event, false, false, true, false, false, false, false, false, false),
        InputAction::Quit
    );
}

#[test]
fn esc_in_command_mode_cancels() {
    assert_eq!(command_mode(KeyCode::Esc), InputAction::CommandCancel);
}

#[test]
fn enter_in_command_mode_submits() {
    assert_eq!(command_mode(KeyCode::Enter), InputAction::CommandSubmit);
}

#[test]
fn left_in_command_mode_moves_cursor_left() {
    assert_eq!(
        command_mode(KeyCode::Left),
        InputAction::CommandMoveCursorLeft
    );
}

#[test]
fn right_in_command_mode_moves_cursor_right() {
    assert_eq!(
        command_mode(KeyCode::Right),
        InputAction::CommandMoveCursorRight
    );
}

#[test]
fn backspace_in_command_mode_deletes_back() {
    assert_eq!(
        command_mode(KeyCode::Backspace),
        InputAction::CommandDeleteBack
    );
}

#[test]
fn printable_char_in_command_mode_appends() {
    assert_eq!(
        command_mode(KeyCode::Char('x')),
        InputAction::CommandAppend('x')
    );
}

#[test]
fn arrows_in_a_non_palette_command_buffer_do_not_move_the_palette() {
    assert_eq!(command_mode(KeyCode::Up), InputAction::Disarm);
    assert_eq!(command_mode(KeyCode::Down), InputAction::Disarm);
}

#[test]
fn tab_in_a_non_palette_command_buffer_is_not_a_palette_completion() {
    assert_eq!(command_mode(KeyCode::Tab), InputAction::Disarm);
}

#[test]
fn up_in_the_active_palette_selects_the_previous_row() {
    assert_eq!(palette_mode(KeyCode::Up), InputAction::PaletteSelectPrev);
}

#[test]
fn down_in_the_active_palette_selects_the_next_row() {
    assert_eq!(palette_mode(KeyCode::Down), InputAction::PaletteSelectNext);
}

#[test]
fn tab_in_the_active_palette_completes_the_buffer() {
    assert_eq!(palette_mode(KeyCode::Tab), InputAction::PaletteComplete);
}

#[test]
fn esc_in_the_active_palette_still_cancels() {
    assert_eq!(palette_mode(KeyCode::Esc), InputAction::CommandCancel);
}

#[test]
fn enter_in_the_active_palette_still_submits() {
    assert_eq!(palette_mode(KeyCode::Enter), InputAction::CommandSubmit);
}

#[test]
fn backspace_in_the_active_palette_still_deletes_back() {
    assert_eq!(
        palette_mode(KeyCode::Backspace),
        InputAction::CommandDeleteBack
    );
}

#[test]
fn printable_char_in_the_active_palette_still_appends() {
    assert_eq!(
        palette_mode(KeyCode::Char('o')),
        InputAction::CommandAppend('o')
    );
}

#[test]
fn down_with_suggestions_selects_the_next_suggestion() {
    assert_eq!(
        suggestion_mode(KeyCode::Down),
        InputAction::SuggestionSelectNext
    );
}

#[test]
fn up_with_suggestions_selects_the_previous_suggestion() {
    assert_eq!(
        suggestion_mode(KeyCode::Up),
        InputAction::SuggestionSelectPrev
    );
}

#[test]
fn tab_with_suggestions_moves_the_selection_forward() {
    assert_eq!(
        suggestion_mode(KeyCode::Tab),
        InputAction::SuggestionSelectNext
    );
}

#[test]
fn esc_with_suggestions_dismisses_them_instead_of_cancelling() {
    assert_eq!(
        suggestion_mode(KeyCode::Esc),
        InputAction::SuggestionDismiss
    );
}

#[test]
fn enter_with_suggestions_still_submits() {
    assert_eq!(suggestion_mode(KeyCode::Enter), InputAction::CommandSubmit);
}

#[test]
fn a_printable_char_with_suggestions_still_appends_to_the_buffer() {
    assert_eq!(
        suggestion_mode(KeyCode::Char('a')),
        InputAction::CommandAppend('a')
    );
}

#[test]
fn arrows_in_the_history_list_move_the_selection() {
    assert_eq!(history_mode(KeyCode::Down), InputAction::HistorySelectNext);
    assert_eq!(history_mode(KeyCode::Up), InputAction::HistorySelectPrev);
}

#[test]
fn enter_in_the_history_list_activates_the_selected_entry() {
    assert_eq!(
        history_mode(KeyCode::Enter),
        InputAction::HistoryActivateSelected
    );
}

#[test]
fn delete_in_the_history_list_removes_the_selected_entry() {
    assert_eq!(
        history_mode(KeyCode::Delete),
        InputAction::HistoryDeleteSelected
    );
    assert_eq!(
        history_mode(KeyCode::Char('d')),
        InputAction::HistoryDeleteSelected
    );
}

#[test]
fn esc_in_the_history_list_closes_it() {
    assert_eq!(history_mode(KeyCode::Esc), InputAction::HistoryClose);
}

#[test]
fn ctrl_c_still_quits_from_the_history_list() {
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        map_key_event(event, false, false, false, false, false, false, true, false, false),
        InputAction::Quit
    );
}

#[test]
fn tab_in_reading_mode_returns_focus_next_link() {
    assert_eq!(
        reading(KeyCode::Tab, false, false),
        InputAction::FocusNextLink
    );
}

#[test]
fn shift_tab_in_reading_mode_returns_focus_previous_link() {
    assert_eq!(
        reading(KeyCode::BackTab, false, false),
        InputAction::FocusPreviousLink
    );
}

#[test]
fn backspace_in_reading_mode_returns_navigate_back() {
    assert_eq!(
        reading(KeyCode::Backspace, false, false),
        InputAction::NavigateBack
    );
}

#[test]
fn enter_in_link_navigation_returns_activate_focused_link() {
    assert_eq!(
        link_navigation(KeyCode::Enter),
        InputAction::ActivateFocusedLink
    );
}

#[test]
fn esc_in_link_navigation_returns_disarm() {
    assert_eq!(link_navigation(KeyCode::Esc), InputAction::Disarm);
}

#[test]
fn arrows_in_the_cookies_popup_scroll_the_selection() {
    assert_eq!(cookies_mode(KeyCode::Down), InputAction::CookiesSelectNext);
    assert_eq!(cookies_mode(KeyCode::Up), InputAction::CookiesSelectPrev);
}

#[test]
fn esc_in_the_cookies_popup_closes_it() {
    assert_eq!(cookies_mode(KeyCode::Esc), InputAction::CookiesClose);
}

#[test]
fn a_stray_key_in_the_cookies_popup_is_ignored() {
    assert_eq!(cookies_mode(KeyCode::Enter), InputAction::Disarm);
}

#[test]
fn ctrl_c_still_quits_from_the_cookies_popup() {
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        map_key_event(event, false, false, false, false, false, false, false, true, false),
        InputAction::Quit
    );
}

#[test]
fn arrows_in_the_settings_panel_move_the_focus() {
    assert_eq!(
        settings_mode(KeyCode::Down),
        InputAction::SettingsSelectNext
    );
    assert_eq!(settings_mode(KeyCode::Up), InputAction::SettingsSelectPrev);
}

#[test]
fn vim_keys_in_the_settings_panel_move_the_focus() {
    assert_eq!(
        settings_mode(KeyCode::Char('j')),
        InputAction::SettingsSelectNext
    );
    assert_eq!(
        settings_mode(KeyCode::Char('k')),
        InputAction::SettingsSelectPrev
    );
}

#[test]
fn esc_in_the_settings_panel_closes_it() {
    assert_eq!(settings_mode(KeyCode::Esc), InputAction::SettingsClose);
}

#[test]
fn space_and_enter_in_the_settings_panel_toggle_the_focused_row() {
    assert_eq!(
        settings_mode(KeyCode::Char(' ')),
        InputAction::SettingsToggle
    );
    assert_eq!(settings_mode(KeyCode::Enter), InputAction::SettingsToggle);
}

#[test]
fn left_and_right_in_the_settings_panel_cycle_the_focused_radio() {
    assert_eq!(settings_mode(KeyCode::Left), InputAction::SettingsCyclePrev);
    assert_eq!(
        settings_mode(KeyCode::Char('h')),
        InputAction::SettingsCyclePrev
    );
    assert_eq!(
        settings_mode(KeyCode::Right),
        InputAction::SettingsCycleNext
    );
    assert_eq!(
        settings_mode(KeyCode::Char('l')),
        InputAction::SettingsCycleNext
    );
}

#[test]
fn a_stray_key_in_the_settings_panel_is_ignored() {
    assert_eq!(settings_mode(KeyCode::Tab), InputAction::Disarm);
}

#[test]
fn ctrl_c_still_quits_from_the_settings_panel() {
    let event = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(
        map_key_event(event, false, false, false, false, false, false, false, false, true),
        InputAction::Quit
    );
}
