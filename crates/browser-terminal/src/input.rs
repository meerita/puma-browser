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
    PaletteSelectPrev,
    PaletteSelectNext,
    PaletteComplete,
    SuggestionSelectPrev,
    SuggestionSelectNext,
    SuggestionDismiss,
    HistorySelectPrev,
    HistorySelectNext,
    HistoryActivateSelected,
    HistoryDeleteSelected,
    HistoryClose,
    CookiesSelectPrev,
    CookiesSelectNext,
    CookiesClose,
    SettingsSelectPrev,
    SettingsSelectNext,
    SettingsToggle,
    SettingsCyclePrev,
    SettingsCycleNext,
    SettingsClose,
    SettingsTextInput(char),
    SettingsTextDeleteBack,
    SettingsTextMoveCursorLeft,
    SettingsTextMoveCursorRight,
    SettingsTextCancel,
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
/// characters append to the buffer. When the slash-command palette is active, the arrow
/// keys move the highlighted row and `Tab` completes the buffer to it instead.
#[allow(clippy::too_many_arguments)]
pub(crate) fn map_key_event(
    event: KeyEvent,
    quit_armed: bool,
    refresh_armed: bool,
    in_command_mode: bool,
    in_link_navigation: bool,
    palette_active: bool,
    address_suggestions_active: bool,
    in_history: bool,
    in_cookies: bool,
    in_settings: bool,
    settings_text_field_focused: bool,
) -> InputAction {
    if is_quit_combination(event) {
        return InputAction::Quit;
    }
    if in_settings {
        return map_settings_key(event, settings_text_field_focused);
    }
    if in_history {
        return map_history_key(event);
    }
    if in_cookies {
        return map_cookies_key(event);
    }
    if in_command_mode {
        return map_command_mode_key(event, palette_active, address_suggestions_active);
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

fn map_command_mode_key(
    event: KeyEvent,
    palette_active: bool,
    address_suggestions_active: bool,
) -> InputAction {
    if palette_active {
        if let Some(action) = map_palette_key(event) {
            return action;
        }
    } else if address_suggestions_active {
        if let Some(action) = map_suggestion_key(event) {
            return action;
        }
    }
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

/// Suggestion-only keys layered over an address (non-slash) command buffer: the arrows and
/// Tab move the highlighted suggestion, and `Esc` dismisses the list without leaving command
/// mode. Returns `None` for every other key so the base command-mode map still handles
/// `Enter` (which submits the selection or the typed text), cursor moves, and typing.
fn map_suggestion_key(event: KeyEvent) -> Option<InputAction> {
    match event.code {
        KeyCode::Up | KeyCode::BackTab => Some(InputAction::SuggestionSelectPrev),
        KeyCode::Down | KeyCode::Tab => Some(InputAction::SuggestionSelectNext),
        KeyCode::Esc => Some(InputAction::SuggestionDismiss),
        _ => None,
    }
}

/// Maps a key while the history list is open. The arrows move the selection, `Enter` opens
/// the selected entry, `Delete` (or `d`) removes it, and `Esc` closes the list. Any other key
/// is ignored so a stray keystroke never dismisses the list or scrolls the page behind it.
fn map_history_key(event: KeyEvent) -> InputAction {
    match event.code {
        KeyCode::Up | KeyCode::Char('k') => InputAction::HistorySelectPrev,
        KeyCode::Down | KeyCode::Char('j') => InputAction::HistorySelectNext,
        KeyCode::Enter => InputAction::HistoryActivateSelected,
        KeyCode::Delete | KeyCode::Char('d') => InputAction::HistoryDeleteSelected,
        KeyCode::Esc => InputAction::HistoryClose,
        _ => InputAction::Disarm,
    }
}

/// Maps a key while the cookie inspection popup is open. The arrows scroll the highlighted
/// row and `Esc` closes the popup. Any other key is ignored so a stray keystroke never
/// dismisses the popup or scrolls the page behind it. The popup is read-only, so there is
/// no activate or delete action here.
fn map_cookies_key(event: KeyEvent) -> InputAction {
    match event.code {
        KeyCode::Up | KeyCode::Char('k') => InputAction::CookiesSelectPrev,
        KeyCode::Down | KeyCode::Char('j') => InputAction::CookiesSelectNext,
        KeyCode::Esc => InputAction::CookiesClose,
        _ => InputAction::Disarm,
    }
}

/// Maps a key while the settings panel is open. A focused text input takes its own key map so
/// printable characters edit the draft; every other row uses the control map. In the control
/// map the arrows (and `k`/`j`) move the focused row, `Space`/`Enter` toggles a focused
/// checkbox, `Left`/`Right` cycle a focused radio group's selection, and `Esc` closes the
/// panel. Any other key is ignored so a stray keystroke never dismisses the panel.
fn map_settings_key(event: KeyEvent, text_field_focused: bool) -> InputAction {
    if text_field_focused {
        return map_settings_text_key(event);
    }
    match event.code {
        KeyCode::Up | KeyCode::Char('k') => InputAction::SettingsSelectPrev,
        KeyCode::Down | KeyCode::Char('j') => InputAction::SettingsSelectNext,
        KeyCode::Char(' ') | KeyCode::Enter => InputAction::SettingsToggle,
        KeyCode::Left | KeyCode::Char('h') => InputAction::SettingsCyclePrev,
        KeyCode::Right | KeyCode::Char('l') => InputAction::SettingsCycleNext,
        KeyCode::Esc => InputAction::SettingsClose,
        _ => InputAction::Disarm,
    }
}

/// Maps a key while a settings text input is focused. Printable characters (including space)
/// edit the draft, `Backspace` deletes, `Left`/`Right` move the cursor, the vertical arrows
/// move focus to the next or previous row (committing the edit on the way out), `Enter` moves
/// to the next row for the same reason, and `Esc` reverts an unsaved draft or closes the panel.
/// Only the vertical arrows move focus here: `j`, `k`, `h`, and `l` are ordinary characters in
/// a text field, not navigation.
fn map_settings_text_key(event: KeyEvent) -> InputAction {
    match event.code {
        KeyCode::Up => InputAction::SettingsSelectPrev,
        KeyCode::Down | KeyCode::Enter => InputAction::SettingsSelectNext,
        KeyCode::Left => InputAction::SettingsTextMoveCursorLeft,
        KeyCode::Right => InputAction::SettingsTextMoveCursorRight,
        KeyCode::Backspace => InputAction::SettingsTextDeleteBack,
        KeyCode::Esc => InputAction::SettingsTextCancel,
        KeyCode::Char(character) if !character.is_control() => {
            InputAction::SettingsTextInput(character)
        }
        _ => InputAction::Disarm,
    }
}

/// Palette-only keys layered over command mode: the arrows move the highlighted row and
/// `Tab` completes the buffer to it. Returns `None` for every other key so the base
/// command-mode map still handles `Esc`, `Enter`, `Backspace`, cursor moves, and typing.
/// These bindings apply only while the buffer is a slash buffer, so plain URL editing keeps
/// its existing behavior.
fn map_palette_key(event: KeyEvent) -> Option<InputAction> {
    match event.code {
        KeyCode::Up => Some(InputAction::PaletteSelectPrev),
        KeyCode::Down => Some(InputAction::PaletteSelectNext),
        KeyCode::Tab => Some(InputAction::PaletteComplete),
        _ => None,
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
