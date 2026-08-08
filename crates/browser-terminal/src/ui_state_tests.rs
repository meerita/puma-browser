// @file crates/browser-terminal/src/ui_state_tests.rs
// @description Unit tests for UiState hint rotation, transient hint lifecycle, and command mode.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::time::{Duration, Instant};

use browser_core::{CookiePolicy, CookiePolicyPair, HistoryEntry, SearchEngine};

use super::{UiState, READING_HINTS};
use crate::settings_view::{build_settings_model, CycleDirection, SettingId, SettingsModel};
use crate::{EnvOverrides, TerminalSettings};

fn history_entry(id: u64, url: &str) -> HistoryEntry {
    HistoryEntry::new(id, url.to_string(), None, 0)
}

/// Whether the open palette currently offers the command named `name`.
fn palette_contains(state: &UiState, name: &str) -> bool {
    state
        .palette_matches()
        .iter()
        .any(|found| found.spec.name == name)
}

/// A settings model with the default policy and engine, giving the eight config-key rows the
/// focus tests wrap over.
fn sample_settings_model() -> SettingsModel {
    let settings = TerminalSettings {
        copy_on_select: true,
        force_osc52: false,
        search_enabled: true,
        unwrap_tracking: false,
        env_overridden: EnvOverrides::default(),
    };
    build_settings_model(
        &settings,
        CookiePolicyPair::default(),
        &SearchEngine::default(),
    )
}

#[test]
fn new_state_shows_first_reading_hint() {
    let state = UiState::new(true);
    assert_eq!(state.current_hint(), READING_HINTS[0]);
}

#[test]
fn advance_hint_if_due_rotates_to_next_hint_after_thirty_seconds() {
    let mut state = UiState::new(true);
    let future = Instant::now() + Duration::from_secs(31);
    state.advance_hint_if_due(future);
    assert_eq!(state.current_hint(), READING_HINTS[1]);
}

#[test]
fn advance_hint_if_due_does_not_rotate_before_thirty_seconds() {
    let mut state = UiState::new(true);
    let soon = Instant::now() + Duration::from_secs(1);
    state.advance_hint_if_due(soon);
    assert_eq!(state.current_hint(), READING_HINTS[0]);
}

#[test]
fn transient_hint_overrides_the_rotating_hint() {
    let mut state = UiState::new(true);
    state.set_transient_hint("Press Esc again to quit", Instant::now());
    assert_eq!(state.current_hint(), "Press Esc again to quit");
}

#[test]
fn transient_hint_clears_after_five_seconds() {
    let mut state = UiState::new(true);
    let stale = Instant::now() - Duration::from_secs(6);
    state.set_transient_hint("Press Esc again to quit", stale);
    state.clear_transient_if_expired(Instant::now());
    assert_eq!(state.current_hint(), READING_HINTS[0]);
}

#[test]
fn clear_transient_restores_the_rotating_hint() {
    let mut state = UiState::new(true);
    state.set_transient_hint("Press r again to refresh", Instant::now());
    state.clear_transient();
    assert_eq!(state.current_hint(), READING_HINTS[0]);
}

#[test]
fn transient_message_overrides_the_rotating_hint() {
    let mut state = UiState::new(true);
    state.set_transient_message("copied 12 chars to clipboard".to_string(), Instant::now());
    assert_eq!(state.current_hint(), "copied 12 chars to clipboard");
    assert_eq!(
        state.transient_message(),
        Some("copied 12 chars to clipboard")
    );
}

#[test]
fn transient_message_clears_after_five_seconds() {
    let mut state = UiState::new(true);
    let stale = Instant::now() - Duration::from_secs(6);
    state.set_transient_message("copied 3 chars to clipboard".to_string(), stale);
    state.clear_transient_if_expired(Instant::now());
    assert_eq!(state.current_hint(), READING_HINTS[0]);
    assert_eq!(state.transient_message(), None);
}

#[test]
fn transient_message_survives_before_five_seconds() {
    let mut state = UiState::new(true);
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
    let state = UiState::new(true);
    assert_eq!(state.transient_message(), None);
}

#[test]
fn new_state_is_not_in_command_mode() {
    let state = UiState::new(true);
    assert!(!state.is_in_command_mode());
}

#[test]
fn entering_command_mode_seeds_buffer_with_first_char() {
    let mut state = UiState::new(true);
    state.enter_command_mode('h');
    assert!(state.is_in_command_mode());
    assert_eq!(state.command_buffer(), "h");
    assert_eq!(state.cursor_byte_offset(), 1);
}

#[test]
fn command_append_char_inserts_at_cursor_and_advances_offset() {
    let mut state = UiState::new(true);
    state.enter_command_mode('h');
    state.command_append_char('i');
    assert_eq!(state.command_buffer(), "hi");
    assert_eq!(state.cursor_byte_offset(), 2);
}

#[test]
fn command_append_char_inserts_at_mid_cursor_position() {
    let mut state = UiState::new(true);
    state.enter_command_mode('a');
    state.command_append_char('c');
    state.command_move_left();
    state.command_append_char('b');
    assert_eq!(state.command_buffer(), "abc");
    assert_eq!(state.cursor_byte_offset(), 2);
}

#[test]
fn command_move_left_moves_cursor_back_by_one_char() {
    let mut state = UiState::new(true);
    state.enter_command_mode('a');
    state.command_append_char('b');
    state.command_move_left();
    assert_eq!(state.cursor_byte_offset(), 1);
}

#[test]
fn command_move_left_at_start_is_a_no_op() {
    let mut state = UiState::new(true);
    state.enter_command_mode('x');
    state.command_move_left();
    state.command_move_left();
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn command_move_right_moves_cursor_forward_by_one_char() {
    let mut state = UiState::new(true);
    state.enter_command_mode('a');
    state.command_append_char('b');
    state.command_move_left();
    state.command_move_right();
    assert_eq!(state.cursor_byte_offset(), 2);
}

#[test]
fn command_move_right_at_end_is_a_no_op() {
    let mut state = UiState::new(true);
    state.enter_command_mode('x');
    state.command_move_right();
    assert_eq!(state.cursor_byte_offset(), 1);
}

#[test]
fn command_delete_before_cursor_removes_char_and_retreats_offset() {
    let mut state = UiState::new(true);
    state.enter_command_mode('a');
    state.command_append_char('b');
    state.command_delete_before_cursor();
    assert_eq!(state.command_buffer(), "a");
    assert_eq!(state.cursor_byte_offset(), 1);
}

#[test]
fn command_delete_before_cursor_at_start_is_a_no_op() {
    let mut state = UiState::new(true);
    state.enter_command_mode('x');
    state.command_move_left();
    state.command_delete_before_cursor();
    assert_eq!(state.command_buffer(), "x");
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn cancel_command_mode_clears_buffer_and_returns_to_reading() {
    let mut state = UiState::new(true);
    state.enter_command_mode('h');
    state.command_append_char('i');
    state.cancel_command_mode();
    assert!(!state.is_in_command_mode());
    assert_eq!(state.command_buffer(), "");
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn take_submit_buffer_returns_a_url_buffer_verbatim_and_returns_to_reading() {
    let mut state = UiState::new(true);
    state.enter_command_mode('h');
    state.command_append_char('i');
    let url = state.take_submit_buffer();
    assert_eq!(url, "hi");
    assert!(!state.is_in_command_mode());
    assert_eq!(state.command_buffer(), "");
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn command_append_char_handles_multibyte_unicode_correctly() {
    let mut state = UiState::new(true);
    state.enter_command_mode('é');
    assert_eq!(state.cursor_byte_offset(), 'é'.len_utf8());
    state.command_delete_before_cursor();
    assert_eq!(state.command_buffer(), "");
    assert_eq!(state.cursor_byte_offset(), 0);
}

#[test]
fn focus_next_link_advances_index() {
    let mut state = UiState::new(true);
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
    let mut state = UiState::new(true);
    state.focus_previous_link(3);
    assert_eq!(state.focused_link_index, Some(2));
}

#[test]
fn enter_and_exit_link_navigation() {
    let mut state = UiState::new(true);
    state.enter_link_navigation(1);
    assert!(state.is_in_link_navigation());
    assert_eq!(state.focused_link_index, Some(1));
    state.exit_link_navigation();
    assert!(!state.is_in_link_navigation());
    assert_eq!(state.focused_link_index, None);
}

#[test]
fn mark_visited_and_is_visited() {
    let mut state = UiState::new(true);
    state.mark_visited("https://a.test/");
    assert!(state.is_visited("https://a.test/"));
    assert!(!state.is_visited("https://b.test/"));
}

#[test]
fn new_state_has_no_palette_matches() {
    let state = UiState::new(true);
    assert!(!state.is_palette_active());
    assert!(state.palette_matches().is_empty());
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn entering_slash_fills_the_palette_with_all_commands() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    assert!(state.is_palette_active());
    assert_eq!(state.palette_matches().len(), 9);
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn typing_filters_the_palette_and_resets_selection() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('q');
    assert_eq!(state.palette_matches().len(), 1);
    assert_eq!(state.palette_matches()[0].spec.name, "quit");
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn deleting_a_character_rewidens_the_palette() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('q');
    assert_eq!(state.palette_matches().len(), 1);
    state.command_delete_before_cursor();
    assert_eq!(state.palette_matches().len(), 9);
}

#[test]
fn no_matching_command_leaves_an_empty_palette() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('z');
    state.command_append_char('z');
    assert!(state.palette_matches().is_empty());
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn non_slash_buffer_is_not_palette_active() {
    let mut state = UiState::new(true);
    state.enter_command_mode('h');
    assert!(!state.is_palette_active());
    assert!(state.palette_matches().is_empty());
}

#[test]
fn cancel_command_mode_clears_the_palette() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.cancel_command_mode();
    assert!(!state.is_palette_active());
    assert!(state.palette_matches().is_empty());
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn take_submit_buffer_clears_the_palette() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('r');
    let buffer = state.take_submit_buffer();
    assert_eq!(buffer, "/reload");
    assert!(!state.is_in_command_mode());
    assert!(state.palette_matches().is_empty());
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn palette_select_next_wraps_around_the_match_list() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    assert_eq!(state.palette_selected(), 0);
    state.palette_select_next();
    assert_eq!(state.palette_selected(), 1);
    for _ in 0..8 {
        state.palette_select_next();
    }
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn palette_select_prev_wraps_to_the_last_row() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.palette_select_prev();
    assert_eq!(state.palette_selected(), state.palette_matches().len() - 1);
}

#[test]
fn palette_select_on_an_empty_list_is_a_no_op() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('z');
    state.command_append_char('z');
    assert!(state.palette_matches().is_empty());
    state.palette_select_next();
    state.palette_select_prev();
    assert_eq!(state.palette_selected(), 0);
}

#[test]
fn palette_complete_fills_the_buffer_with_the_selected_command() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('r');
    state.palette_complete();
    assert_eq!(state.command_buffer(), "/reload");
    assert_eq!(state.cursor_byte_offset(), "/reload".len());
}

#[test]
fn palette_complete_appends_a_space_for_a_command_that_takes_an_argument() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('o');
    state.palette_complete();
    assert_eq!(state.command_buffer(), "/open ");
    assert_eq!(state.cursor_byte_offset(), "/open ".len());
}

#[test]
fn palette_complete_keeps_the_selected_command_filtered() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('o');
    state.palette_complete();
    assert_eq!(state.palette_matches().len(), 1);
    assert_eq!(state.palette_matches()[0].spec.name, "open");
}

#[test]
fn palette_complete_on_an_empty_list_is_a_no_op() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('z');
    state.command_append_char('z');
    state.palette_complete();
    assert_eq!(state.command_buffer(), "/zz");
}

#[test]
fn take_submit_buffer_runs_the_highlighted_command_when_the_token_is_not_exact() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('o');
    let buffer = state.take_submit_buffer();
    assert_eq!(buffer, "/open");
}

#[test]
fn take_submit_buffer_keeps_the_typed_argument_when_resolving_the_selection() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('o');
    state.command_append_char(' ');
    state.command_append_char('a');
    let buffer = state.take_submit_buffer();
    assert_eq!(buffer, "/open a");
}

#[test]
fn take_submit_buffer_runs_the_exact_command_over_the_highlighted_row() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('b');
    state.command_append_char('a');
    state.command_append_char('c');
    state.command_append_char('k');
    let buffer = state.take_submit_buffer();
    assert_eq!(buffer, "/back");
}

#[test]
fn take_submit_buffer_returns_an_unmatched_slash_token_unchanged() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('z');
    state.command_append_char('z');
    let buffer = state.take_submit_buffer();
    assert_eq!(buffer, "/zz");
}

#[test]
fn backspace_deleting_the_leading_slash_exits_command_mode() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_delete_or_exit();
    assert!(!state.is_in_command_mode());
    assert_eq!(state.command_buffer(), "");
    assert!(state.palette_matches().is_empty());
}

#[test]
fn backspace_before_the_leading_slash_only_deletes_a_character() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('o');
    state.command_delete_or_exit();
    assert!(state.is_in_command_mode());
    assert_eq!(state.command_buffer(), "/");
    assert_eq!(state.palette_matches().len(), 9);
}

#[test]
fn palette_includes_the_search_command_when_search_is_enabled() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    state.command_append_char('s');
    state.command_append_char('e');
    let names: Vec<&str> = state
        .palette_matches()
        .iter()
        .map(|found| found.spec.name)
        .collect();
    assert!(names.contains(&"search"), "expected search in {names:?}");
}

#[test]
fn palette_excludes_the_search_command_when_search_is_disabled() {
    let mut state = UiState::new(false);
    state.enter_command_mode('/');
    state.command_append_char('s');
    state.command_append_char('e');
    let names: Vec<&str> = state
        .palette_matches()
        .iter()
        .map(|found| found.spec.name)
        .collect();
    assert!(
        !names.contains(&"search"),
        "search must be hidden in {names:?}"
    );
    // Other commands that match the same query still appear.
    assert!(
        names.contains(&"settings"),
        "expected settings in {names:?}"
    );
}

#[test]
fn disabling_search_hides_only_the_search_command_from_the_full_palette() {
    let mut enabled = UiState::new(true);
    enabled.enter_command_mode('/');
    let mut disabled = UiState::new(false);
    disabled.enter_command_mode('/');
    assert_eq!(enabled.palette_matches().len(), 9);
    assert_eq!(disabled.palette_matches().len(), 8);
}

#[test]
fn new_state_has_no_address_suggestions() {
    let state = UiState::new(true);
    assert!(!state.has_address_suggestions());
    assert_eq!(state.selected_suggestion(), None);
}

#[test]
fn setting_address_suggestions_offers_them_with_no_selection() {
    let mut state = UiState::new(true);
    state.set_address_suggestions(vec!["https://a.test/".to_string()]);
    assert!(state.has_address_suggestions());
    assert_eq!(state.selected_suggestion(), None);
}

#[test]
fn suggestion_select_next_enters_the_list_and_wraps() {
    let mut state = UiState::new(true);
    state.set_address_suggestions(vec!["a".to_string(), "b".to_string()]);
    state.suggestion_select_next();
    assert_eq!(state.selected_suggestion(), Some(0));
    state.suggestion_select_next();
    assert_eq!(state.selected_suggestion(), Some(1));
    state.suggestion_select_next();
    assert_eq!(state.selected_suggestion(), Some(0));
}

#[test]
fn suggestion_select_prev_enters_at_the_last_row() {
    let mut state = UiState::new(true);
    state.set_address_suggestions(vec!["a".to_string(), "b".to_string()]);
    state.suggestion_select_prev();
    assert_eq!(state.selected_suggestion(), Some(1));
}

#[test]
fn setting_new_suggestions_resets_the_selection() {
    let mut state = UiState::new(true);
    state.set_address_suggestions(vec!["a".to_string(), "b".to_string()]);
    state.suggestion_select_next();
    state.set_address_suggestions(vec!["c".to_string()]);
    assert_eq!(state.selected_suggestion(), None);
}

#[test]
fn take_selected_suggestion_returns_the_url_and_leaves_command_mode() {
    let mut state = UiState::new(true);
    state.enter_command_mode('a');
    state.set_address_suggestions(vec!["https://a.test/".to_string()]);
    state.suggestion_select_next();
    let taken = state.take_selected_suggestion();
    assert_eq!(taken.as_deref(), Some("https://a.test/"));
    assert!(!state.is_in_command_mode());
    assert!(!state.has_address_suggestions());
}

#[test]
fn take_selected_suggestion_is_none_without_a_selection() {
    let mut state = UiState::new(true);
    state.enter_command_mode('a');
    state.set_address_suggestions(vec!["https://a.test/".to_string()]);
    assert_eq!(state.take_selected_suggestion(), None);
}

#[test]
fn clearing_address_suggestions_empties_the_list_and_selection() {
    let mut state = UiState::new(true);
    state.set_address_suggestions(vec!["a".to_string()]);
    state.suggestion_select_next();
    state.clear_address_suggestions();
    assert!(!state.has_address_suggestions());
    assert_eq!(state.selected_suggestion(), None);
}

#[test]
fn new_state_is_not_in_history_mode() {
    let state = UiState::new(true);
    assert!(!state.is_in_history_mode());
}

#[test]
fn entering_history_mode_holds_the_entries_and_selects_the_first() {
    let mut state = UiState::new(true);
    state.enter_history_mode(vec![
        history_entry(1, "https://a.test/"),
        history_entry(2, "https://b.test/"),
    ]);
    assert!(state.is_in_history_mode());
    assert_eq!(state.history_entries().len(), 2);
    assert_eq!(state.history_selected(), 0);
    assert_eq!(
        state.selected_history_entry().map(|entry| entry.url()),
        Some("https://a.test/")
    );
}

#[test]
fn history_select_next_and_prev_wrap_around_the_list() {
    let mut state = UiState::new(true);
    state.enter_history_mode(vec![history_entry(1, "a"), history_entry(2, "b")]);
    state.history_select_next();
    assert_eq!(state.history_selected(), 1);
    state.history_select_next();
    assert_eq!(state.history_selected(), 0);
    state.history_select_prev();
    assert_eq!(state.history_selected(), 1);
}

#[test]
fn removing_the_selected_history_entry_drops_it_and_clamps_the_selection() {
    let mut state = UiState::new(true);
    state.enter_history_mode(vec![history_entry(1, "a"), history_entry(2, "b")]);
    state.history_select_next();
    state.remove_selected_history_entry();
    assert_eq!(state.history_entries().len(), 1);
    assert_eq!(state.history_selected(), 0);
    assert_eq!(
        state.selected_history_entry().map(|entry| entry.url()),
        Some("a")
    );
}

#[test]
fn removing_the_last_history_entry_closes_the_list() {
    let mut state = UiState::new(true);
    state.enter_history_mode(vec![history_entry(1, "a")]);
    state.remove_selected_history_entry();
    assert!(!state.is_in_history_mode());
    assert!(state.history_entries().is_empty());
}

#[test]
fn exiting_history_mode_returns_to_reading() {
    let mut state = UiState::new(true);
    state.enter_history_mode(vec![history_entry(1, "a")]);
    state.exit_history_mode();
    assert!(!state.is_in_history_mode());
    assert!(state.history_entries().is_empty());
}

#[test]
fn entering_settings_mode_focuses_the_first_row() {
    let mut state = UiState::new(true);
    state.enter_settings_mode(sample_settings_model());
    assert!(state.is_in_settings_mode());
    assert_eq!(state.settings_focus(), 0);
}

#[test]
fn exiting_settings_mode_restores_the_previous_mode() {
    let mut state = UiState::new(true);
    state.enter_link_navigation(0);
    state.enter_settings_mode(sample_settings_model());
    assert!(state.is_in_settings_mode());
    state.exit_settings_mode();
    assert!(!state.is_in_settings_mode());
    assert!(state.is_in_link_navigation());
    assert!(state.settings_model().is_none());
}

#[test]
fn settings_focus_next_wraps_from_the_last_row_to_the_first() {
    let mut state = UiState::new(true);
    state.enter_settings_mode(sample_settings_model());
    for _ in 0..7 {
        state.settings_focus_next();
    }
    assert_eq!(state.settings_focus(), 7);
    state.settings_focus_next();
    assert_eq!(state.settings_focus(), 0);
}

#[test]
fn settings_focus_prev_wraps_from_the_first_row_to_the_last() {
    let mut state = UiState::new(true);
    state.enter_settings_mode(sample_settings_model());
    state.settings_focus_prev();
    assert_eq!(state.settings_focus(), 7);
}

#[test]
fn settings_focus_is_a_no_op_when_the_panel_is_closed() {
    let mut state = UiState::new(true);
    state.settings_focus_next();
    assert_eq!(state.settings_focus(), 0);
}

#[test]
fn toggling_the_focused_checkbox_flips_only_a_checkbox_row() {
    let mut state = UiState::new(true);
    state.enter_settings_mode(sample_settings_model());
    // The first row is the first-party cookie radio, so a toggle does nothing there.
    assert_eq!(state.toggle_focused_checkbox(), None);
    // Move focus to the copy-on-select checkbox, seeded true, and toggle it off.
    state.settings_focus_next();
    state.settings_focus_next();
    assert_eq!(
        state.toggle_focused_checkbox(),
        Some((SettingId::CopyOnSelect, false))
    );
}

#[test]
fn cycling_the_focused_radio_reports_the_new_policy() {
    let mut state = UiState::new(true);
    state.enter_settings_mode(sample_settings_model());
    // The first-party default is reject, the last option, so cycling forward wraps to allow.
    assert_eq!(
        state.cycle_focused_radio(CycleDirection::Next),
        Some((SettingId::CookiesFirstParty, CookiePolicy::Allow))
    );
}

#[test]
fn cycling_the_focused_radio_does_nothing_on_a_checkbox_row() {
    let mut state = UiState::new(true);
    state.enter_settings_mode(sample_settings_model());
    state.settings_focus_next();
    state.settings_focus_next();
    assert_eq!(state.cycle_focused_radio(CycleDirection::Next), None);
}

#[test]
fn disabling_search_live_removes_it_from_the_open_palette() {
    let mut state = UiState::new(true);
    state.enter_command_mode('/');
    assert!(palette_contains(&state, "search"));
    state.set_search_enabled(false);
    // Editing the buffer refreshes the palette against the new flag.
    state.command_append_char('s');
    assert!(!palette_contains(&state, "search"));
}

#[test]
fn re_enabling_search_live_restores_it_to_the_open_palette() {
    let mut state = UiState::new(false);
    state.enter_command_mode('/');
    assert!(!palette_contains(&state, "search"));
    state.set_search_enabled(true);
    state.command_append_char('s');
    assert!(palette_contains(&state, "search"));
}
