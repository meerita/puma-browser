// @file crates/browser-terminal/src/ui_state_tests.rs
// @description Unit tests for UiState hint rotation and transient hint lifecycle.
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
