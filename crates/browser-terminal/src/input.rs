// @file crates/browser-terminal/src/input.rs
// @description Maps raw key events to viewport input actions and quit-arm transitions.
// @layer terminal
// @created meerita <meerita@icloud.com>

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// A single interpreted input command, decoupled from the raw key event.
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
    EnterCommand(char),
    CommandAppend(char),
    CommandMoveCursorLeft,
    CommandMoveCursorRight,
    CommandDeleteBack,
    CommandCancel,
    CommandSubmit,
    FocusNextLink,
    FocusPreviousLink,
    ActivateFocusedLink,
    NavigateBack,
}

/// Interprets one key event against the current arm state and interaction mode.
///
/// `Ctrl+C` always quits immediately. In reading mode, navigation keys scroll and
/// `Esc`/`r` arm their respective two-press confirmations; any other printable character
/// enters command mode seeded with that character. In command mode, `Esc` cancels,
/// `Enter` submits, arrow keys move the cursor, `Backspace` deletes, and printable
/// characters append to the buffer.
pub(crate) fn map_key_event(
    event: KeyEvent,
    quit_armed: bool,
    refresh_armed: bool,
    in_command_mode: bool,
    in_link_navigation: bool,
) -> InputAction {
    if is_quit_combination(event) {
        return InputAction::Quit;
    }
    if in_command_mode {
        return map_command_mode_key(event);
    }
    if in_link_navigation {
        return map_link_navigation_key(event);
    }
    map_reading_mode_key(event, quit_armed, refresh_armed)
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

fn map_reading_mode_key(event: KeyEvent, quit_armed: bool, refresh_armed: bool) -> InputAction {
    match event.code {
        KeyCode::Esc => arm_or_quit(quit_armed),
        KeyCode::Char('r') => arm_or_refresh(refresh_armed),
        KeyCode::Down | KeyCode::Char('j') => InputAction::ScrollLineDown,
        KeyCode::Up | KeyCode::Char('k') => InputAction::ScrollLineUp,
        KeyCode::PageDown => InputAction::ScrollPageDown,
        KeyCode::PageUp => InputAction::ScrollPageUp,
        KeyCode::Char('g') => InputAction::ScrollToTop,
        KeyCode::Char('G') => InputAction::ScrollToBottom,
        KeyCode::Tab => InputAction::FocusNextLink,
        KeyCode::BackTab => InputAction::FocusPreviousLink,
        KeyCode::Backspace => InputAction::NavigateBack,
        KeyCode::Char(ch) if !ch.is_control() => InputAction::EnterCommand(ch),
        _ => InputAction::Disarm,
    }
}

/// Maps a key while a link is focused. Tab and Shift+Tab move focus, Enter activates the
/// focused link, Backspace goes back in history, Esc leaves link navigation, and the
/// scroll keys keep working so the reader can move the viewport while a link stays
/// focused. Any other key leaves link navigation.
fn map_link_navigation_key(event: KeyEvent) -> InputAction {
    match event.code {
        KeyCode::Tab => InputAction::FocusNextLink,
        KeyCode::BackTab => InputAction::FocusPreviousLink,
        KeyCode::Enter => InputAction::ActivateFocusedLink,
        KeyCode::Esc => InputAction::Disarm,
        KeyCode::Backspace => InputAction::NavigateBack,
        KeyCode::Down | KeyCode::Char('j') => InputAction::ScrollLineDown,
        KeyCode::Up | KeyCode::Char('k') => InputAction::ScrollLineUp,
        KeyCode::PageDown => InputAction::ScrollPageDown,
        KeyCode::PageUp => InputAction::ScrollPageUp,
        KeyCode::Char('g') => InputAction::ScrollToTop,
        KeyCode::Char('G') => InputAction::ScrollToBottom,
        _ => InputAction::Disarm,
    }
}

fn map_command_mode_key(event: KeyEvent) -> InputAction {
    match event.code {
        KeyCode::Esc => InputAction::CommandCancel,
        KeyCode::Enter => InputAction::CommandSubmit,
        KeyCode::Left => InputAction::CommandMoveCursorLeft,
        KeyCode::Right => InputAction::CommandMoveCursorRight,
        KeyCode::Backspace => InputAction::CommandDeleteBack,
        KeyCode::Char(ch) => InputAction::CommandAppend(ch),
        _ => InputAction::Disarm,
    }
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
