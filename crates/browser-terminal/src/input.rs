// @file crates/browser-terminal/src/input.rs
// @description Maps raw key events to viewport input actions and quit-arm transitions.
// @layer terminal
// @created meerita <meerita@icloud.com>

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A single interpreted input command, decoupled from the raw key event.
///
/// The event loop reads one of these per key press: scroll actions move the viewport,
/// `ArmQuit` primes the two-press quit, and `Quit` ends the loop. `Disarm` covers every
/// key that neither scrolls nor quits.
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
    Disarm,
}

/// Interprets one key event against the current quit-arm state.
///
/// `Ctrl+C` always quits immediately. `Esc` arms the quit when it is not already armed
/// and quits on the second press. Every other recognized key scrolls; anything else
/// disarms.
pub(crate) fn map_key_event(event: KeyEvent, quit_armed: bool) -> InputAction {
    if is_quit_combination(event) {
        return InputAction::Quit;
    }
    match event.code {
        KeyCode::Esc => arm_or_quit(quit_armed),
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
/// Only arming keeps the flag set; every other action clears it, which is how any
/// non-`Esc` key disarms a pending quit.
pub(crate) fn quit_armed_after(action: InputAction) -> bool {
    matches!(action, InputAction::ArmQuit)
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

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
