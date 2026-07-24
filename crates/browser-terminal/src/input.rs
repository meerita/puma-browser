// @file crates/browser-terminal/src/input.rs
// @description Maps raw key events to viewport input actions and quit-arm transitions.
// @layer terminal
// @created meerita <meerita@icloud.com>

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A single interpreted input command, decoupled from the raw key event.
///
/// The event loop reads one of these per key press: scroll actions move the viewport,
/// `ArmQuit`/`ArmRefresh` prime their respective two-press confirmations, `Quit` ends
/// the loop, and `Disarm` covers every key that neither scrolls nor arms anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputAction {
    ScrollLineDown,
    ScrollLineUp,
    ScrollPageDown,
    ScrollPageUp,
    ScrollToTop,
    ScrollToBottom,
    ArmQuit,
    Quit,
    ArmRefresh,
    RefreshArmed,
    Disarm,
}

/// Interprets one key event against the current arm state.
///
/// `Ctrl+C` always quits immediately. `Esc` arms the quit when not already armed and
/// quits on the second press. `r` arms a refresh on the first press and confirms it on
/// the second. Every other recognized key scrolls; anything else disarms.
pub(crate) fn map_key_event(event: KeyEvent, quit_armed: bool, refresh_armed: bool) -> InputAction {
    if is_quit_combination(event) {
        return InputAction::Quit;
    }
    match event.code {
        KeyCode::Esc => arm_or_quit(quit_armed),
        KeyCode::Char('r') => arm_or_refresh(refresh_armed),
        KeyCode::Down | KeyCode::Char('j') => InputAction::ScrollLineDown,
        KeyCode::Up | KeyCode::Char('k') => InputAction::ScrollLineUp,
        KeyCode::PageDown => InputAction::ScrollPageDown,
        KeyCode::PageUp => InputAction::ScrollPageUp,
        KeyCode::Char('g') => InputAction::ScrollToTop,
        KeyCode::Char('G') => InputAction::ScrollToBottom,
        _ => InputAction::Disarm,
    }
}

/// The quit-arm flag after `action` runs.
///
/// Only `ArmQuit` keeps the flag set; every other action clears it.
pub(crate) fn quit_armed_after(action: InputAction) -> bool {
    matches!(action, InputAction::ArmQuit)
}

/// The refresh-arm flag after `action` runs.
///
/// Only `ArmRefresh` keeps the flag set; every other action clears it.
pub(crate) fn refresh_armed_after(action: InputAction) -> bool {
    matches!(action, InputAction::ArmRefresh)
}

fn is_quit_combination(event: KeyEvent) -> bool {
    event.code == KeyCode::Char('c') && event.modifiers.contains(KeyModifiers::CONTROL)
}

fn arm_or_quit(quit_armed: bool) -> InputAction {
    if quit_armed {
        return InputAction::Quit;
    }
    InputAction::ArmQuit
}

fn arm_or_refresh(refresh_armed: bool) -> InputAction {
    if refresh_armed {
        return InputAction::RefreshArmed;
    }
    InputAction::ArmRefresh
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
