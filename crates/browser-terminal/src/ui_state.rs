// @file crates/browser-terminal/src/ui_state.rs
// @description UiState struct and InteractionMode enum centralising mutable chrome state.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::collections::HashSet;
use std::time::{Duration, Instant};

use browser_core::{CookiePolicy, HistoryEntry, NodeId, SelectOption};

use crate::command::{self, CommandKind, CommandMatch};
use crate::settings_view::{text_field_id, CycleDirection, SettingId, SettingsModel, SettingsRow};

pub(crate) const READING_HINTS: &[&str] = &[
    "Type a URL or press / for commands",
    "j · k or ↑ · ↓ to scroll  ·  Space / b to page",
    "g to jump to top  ·  G to jump to bottom",
    "Esc to quit  ·  r to refresh the page",
];

const HINT_ROTATION_INTERVAL: Duration = Duration::from_secs(30);
const TRANSIENT_HINT_DURATION: Duration = Duration::from_secs(5);

/// How long a settings text field must sit idle after the last keystroke before its value is
/// validated, applied, and persisted. Long enough that typing a URL does not trigger a save
/// mid-word, short enough that the change lands without a manual save key.
pub(crate) const SETTINGS_AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(1500);

/// Whether a dirty settings text field has been idle long enough to auto-save.
///
/// Pure so the debounce is testable without real time: a caller passes the current instant and
/// the field's last-keystroke instant. A clean field never saves; a dirty field saves once the
/// idle gap reaches `debounce`.
pub(crate) fn should_save(
    now: Instant,
    last_keystroke: Instant,
    dirty: bool,
    debounce: Duration,
) -> bool {
    dirty && now.duration_since(last_keystroke) >= debounce
}

/// The in-progress edit of a settings text field: its identity, the draft editor, and the
/// bookkeeping the debounced auto-save reads.
struct SettingsTextEdit {
    id: SettingId,
    editor: TextEditor,
    // True once the draft differs from the saved value, so only a changed field is saved.
    dirty: bool,
    last_keystroke: Instant,
    // When true, auto-save is held off until the next edit, so a value the controller rejected
    // is not retried on every tick. Cleared by the next keystroke, which also clears the error.
    autosave_suppressed: bool,
    // The inline error shown on the row after a rejected save, or `None` when the field is clean
    // or its last save succeeded.
    error: Option<String>,
}

/// What an `Esc` press does while a settings text field is focused: revert an unsaved draft, or
/// close the panel when there is nothing to revert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsEscOutcome {
    Reverted,
    ClosePanel,
}

/// The persist request a due auto-save or a focus-leave produces: which field to save and the
/// draft value to validate and store. The controller-facing save lives in the app, so this
/// carries only owned data across that boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SettingsTextSave {
    pub(crate) id: SettingId,
    pub(crate) value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InteractionMode {
    Reading,
    Command,
    InteractiveNavigation,
    History,
    Cookies,
    Settings,
    SubmitConfirmation,
}

/// Which choice is highlighted in the `POST` submission confirmation view.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubmitChoice {
    Submit,
    Cancel,
}

impl SubmitChoice {
    /// The other choice, so `Up`/`Down` toggles between exactly two options.
    fn toggled(self) -> Self {
        match self {
            SubmitChoice::Submit => SubmitChoice::Cancel,
            SubmitChoice::Cancel => SubmitChoice::Submit,
        }
    }
}

/// The pending `POST` submission the confirmation view holds while it is open: which
/// button would be activated, where the request would go, and which choice is
/// highlighted. `destination` is already the resolved, sanitized action URL, safe to
/// render as-is.
pub(crate) struct SubmitConfirmation {
    pub(crate) submit_button: NodeId,
    pub(crate) destination: String,
    choice: SubmitChoice,
}

/// A form field's active editing sub-mode, entered from interactive navigation by
/// activating a text-like control or a multi-select.
pub(crate) enum FieldEditState {
    Text {
        node_id: NodeId,
        editor: TextEditor,
        sensitive: bool,
    },
    MultiSelect {
        node_id: NodeId,
        options: Vec<SelectOption>,
        cursor: usize,
    },
}

/// A single-line text buffer with a byte-offset cursor and UTF-8-safe edits.
///
/// Both the command bar and the settings panel's text fields edit through this, so cursor
/// movement over multibyte input behaves identically in each and the boundary arithmetic
/// lives in one place instead of being duplicated per field.
#[derive(Debug, Clone, Default)]
pub(crate) struct TextEditor {
    buffer: String,
    cursor_byte_offset: usize,
}

impl TextEditor {
    /// An empty editor with the cursor at the start.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// An editor holding `value` with the cursor at its end, for seeding a field from an
    /// existing value the user then edits.
    pub(crate) fn seeded(value: &str) -> Self {
        Self {
            buffer: value.to_string(),
            cursor_byte_offset: value.len(),
        }
    }

    pub(crate) fn buffer(&self) -> &str {
        &self.buffer
    }

    pub(crate) fn cursor_byte_offset(&self) -> usize {
        self.cursor_byte_offset
    }

    /// Empties the buffer and returns the cursor to the start.
    pub(crate) fn clear(&mut self) {
        self.buffer.clear();
        self.cursor_byte_offset = 0;
    }

    /// Replaces the whole buffer with `value` and moves the cursor to its end.
    pub(crate) fn set_buffer(&mut self, value: &str) {
        self.buffer.clear();
        self.buffer.push_str(value);
        self.cursor_byte_offset = self.buffer.len();
    }

    /// Inserts `character` at the cursor and advances the cursor past it.
    pub(crate) fn insert_char(&mut self, character: char) {
        self.buffer.insert(self.cursor_byte_offset, character);
        self.cursor_byte_offset += character.len_utf8();
    }

    /// Moves the cursor one character left, stopping at the start.
    pub(crate) fn move_left(&mut self) {
        if self.cursor_byte_offset == 0 {
            return;
        }
        let before = &self.buffer[..self.cursor_byte_offset];
        let prev_char_len = before.chars().next_back().map_or(0, char::len_utf8);
        self.cursor_byte_offset -= prev_char_len;
    }

    /// Moves the cursor one character right, stopping at the end.
    pub(crate) fn move_right(&mut self) {
        if self.cursor_byte_offset >= self.buffer.len() {
            return;
        }
        let after = &self.buffer[self.cursor_byte_offset..];
        let next_char_len = after.chars().next().map_or(0, char::len_utf8);
        self.cursor_byte_offset += next_char_len;
    }

    /// Deletes the character before the cursor, moving the cursor back over it.
    pub(crate) fn delete_before_cursor(&mut self) {
        if self.cursor_byte_offset == 0 {
            return;
        }
        let before = &self.buffer[..self.cursor_byte_offset];
        let prev_char_len = before.chars().next_back().map_or(0, char::len_utf8);
        let new_offset = self.cursor_byte_offset - prev_char_len;
        self.buffer.drain(new_offset..self.cursor_byte_offset);
        self.cursor_byte_offset = new_offset;
    }
}

pub(crate) struct UiState {
    pub(crate) interaction_mode: InteractionMode,
    pub(crate) quit_armed: bool,
    pub(crate) refresh_armed: bool,
    pub(crate) focused_interactive_index: Option<usize>,
    pub(crate) visited_urls: HashSet<String>,
    focused_field_edit: Option<FieldEditState>,
    submit_confirmation: Option<SubmitConfirmation>,
    hint_index: usize,
    last_hint_advance: Instant,
    transient_message: Option<String>,
    transient_set_at: Option<Instant>,
    command_editor: TextEditor,
    palette_matches: Vec<CommandMatch>,
    palette_selected_index: usize,
    address_suggestions: Vec<String>,
    suggestion_selected: Option<usize>,
    citation_preview: Option<String>,
    history_entries: Vec<HistoryEntry>,
    history_selected_index: usize,
    cookie_lines: Vec<String>,
    cookie_selected_index: usize,
    settings_model: Option<SettingsModel>,
    settings_focus_index: usize,
    settings_text_edit: Option<SettingsTextEdit>,
    settings_return_mode: InteractionMode,
    pending_fragment: Option<String>,
    /// Viewport offsets to return to, one per same-page anchor jump not yet undone.
    ///
    /// A stack rather than one slot: a reader who follows three table-of-contents links in
    /// a row expects three Backs to retrace each jump in turn, the way every mainstream
    /// browser behaves. The offsets are rows in one particular rendered buffer, so they are
    /// meaningless once another page loads and are cleared when the page changes.
    anchor_return_offsets: Vec<u16>,
    search_enabled: bool,
}

impl UiState {
    pub(crate) fn new(search_enabled: bool) -> Self {
        Self {
            interaction_mode: InteractionMode::Reading,
            quit_armed: false,
            refresh_armed: false,
            focused_interactive_index: None,
            visited_urls: HashSet::new(),
            focused_field_edit: None,
            submit_confirmation: None,
            hint_index: 0,
            last_hint_advance: Instant::now(),
            transient_message: None,
            transient_set_at: None,
            command_editor: TextEditor::new(),
            palette_matches: Vec::new(),
            palette_selected_index: 0,
            address_suggestions: Vec::new(),
            suggestion_selected: None,
            citation_preview: None,
            history_entries: Vec::new(),
            history_selected_index: 0,
            cookie_lines: Vec::new(),
            cookie_selected_index: 0,
            settings_model: None,
            settings_focus_index: 0,
            settings_text_edit: None,
            settings_return_mode: InteractionMode::Reading,
            pending_fragment: None,
            anchor_return_offsets: Vec::new(),
            search_enabled,
        }
    }

    /// Remembers the fragment to position the viewport on once the page being loaded
    /// finishes rendering. Cleared by [`take_pending_fragment`](Self::take_pending_fragment)
    /// when it is applied, or overwritten when a new load starts.
    pub(crate) fn set_pending_fragment(&mut self, fragment: Option<String>) {
        self.pending_fragment = fragment;
    }

    /// Whether a fragment is waiting to be applied after the current load completes.
    pub(crate) fn has_pending_fragment(&self) -> bool {
        self.pending_fragment.is_some()
    }

    /// Takes the fragment waiting to be applied, leaving none behind.
    pub(crate) fn take_pending_fragment(&mut self) -> Option<String> {
        self.pending_fragment.take()
    }

    /// Remembers the viewport offset an anchor jump is leaving, so Back can return to it.
    pub(crate) fn push_anchor_return(&mut self, offset: u16) {
        self.anchor_return_offsets.push(offset);
    }

    /// Whether a same-page anchor jump is waiting to be undone.
    pub(crate) fn has_anchor_return(&self) -> bool {
        !self.anchor_return_offsets.is_empty()
    }

    /// Takes the offset of the most recent anchor jump, or `None` when none is outstanding.
    pub(crate) fn pop_anchor_return(&mut self) -> Option<u16> {
        self.anchor_return_offsets.pop()
    }

    /// Forgets every outstanding anchor jump, called when the rendered page changes.
    pub(crate) fn clear_anchor_returns(&mut self) {
        self.anchor_return_offsets.clear();
    }

    pub(crate) fn is_in_command_mode(&self) -> bool {
        matches!(self.interaction_mode, InteractionMode::Command)
    }

    pub(crate) fn is_in_interactive_navigation(&self) -> bool {
        matches!(
            self.interaction_mode,
            InteractionMode::InteractiveNavigation
        )
    }

    pub(crate) fn is_in_submit_confirmation(&self) -> bool {
        matches!(self.interaction_mode, InteractionMode::SubmitConfirmation)
    }

    /// Whether a form field's text-edit sub-mode is active, so the key router sends
    /// printable keys to the draft instead of dispatching interactive-navigation keys.
    pub(crate) fn is_in_field_text_edit(&self) -> bool {
        matches!(self.focused_field_edit, Some(FieldEditState::Text { .. }))
    }

    /// Whether a multi-select's expanded option list is open.
    pub(crate) fn is_in_field_multi_select(&self) -> bool {
        matches!(
            self.focused_field_edit,
            Some(FieldEditState::MultiSelect { .. })
        )
    }

    pub(crate) fn is_in_history_mode(&self) -> bool {
        matches!(self.interaction_mode, InteractionMode::History)
    }

    pub(crate) fn is_in_cookies_mode(&self) -> bool {
        matches!(self.interaction_mode, InteractionMode::Cookies)
    }

    pub(crate) fn is_in_settings_mode(&self) -> bool {
        matches!(self.interaction_mode, InteractionMode::Settings)
    }

    /// Replaces the address-bar suggestions with `suggestions`, resetting the selection so
    /// nothing is highlighted until the user moves into the list. An empty list clears the
    /// popup.
    pub(crate) fn set_address_suggestions(&mut self, suggestions: Vec<String>) {
        self.address_suggestions = suggestions;
        self.suggestion_selected = None;
    }

    /// Clears the address-bar suggestions and their selection.
    pub(crate) fn clear_address_suggestions(&mut self) {
        self.address_suggestions.clear();
        self.suggestion_selected = None;
    }

    /// The current address-bar suggestions, most relevant first.
    pub(crate) fn address_suggestions(&self) -> &[String] {
        &self.address_suggestions
    }

    /// Whether any address-bar suggestion is currently offered.
    pub(crate) fn has_address_suggestions(&self) -> bool {
        !self.address_suggestions.is_empty()
    }

    /// Shows the citation preview popup with the given `cite` URL.
    pub(crate) fn set_citation_preview(&mut self, url: String) {
        self.citation_preview = Some(url);
    }

    /// Dismisses the citation preview popup.
    pub(crate) fn clear_citation_preview(&mut self) {
        self.citation_preview = None;
    }

    /// The URL shown in the citation preview popup, or `None` when it is not shown.
    pub(crate) fn citation_preview(&self) -> Option<&str> {
        self.citation_preview.as_deref()
    }

    /// Whether the citation preview popup should currently draw: a URL is set, and no
    /// higher-precedence overlay (address suggestions, history, cookies, settings, or
    /// command mode) is active. Reading and link-navigation are the only modes the
    /// citation popup may show under.
    pub(crate) fn citation_preview_visible(&self) -> bool {
        self.citation_preview.is_some()
            && !self.has_address_suggestions()
            && !self.is_in_command_mode()
            && !self.is_in_history_mode()
            && !self.is_in_cookies_mode()
            && !self.is_in_settings_mode()
    }

    /// The index of the highlighted suggestion, or `None` when none is highlighted and
    /// Enter would submit the typed text instead.
    pub(crate) fn selected_suggestion(&self) -> Option<usize> {
        self.suggestion_selected
    }

    /// Moves the suggestion highlight to the next row, entering the list at the first row
    /// when none is highlighted and wrapping from the last back to the first.
    pub(crate) fn suggestion_select_next(&mut self) {
        if self.address_suggestions.is_empty() {
            return;
        }
        self.suggestion_selected = Some(match self.suggestion_selected {
            Some(current) => (current + 1) % self.address_suggestions.len(),
            None => 0,
        });
    }

    /// Moves the suggestion highlight to the previous row, entering the list at the last row
    /// when none is highlighted and wrapping from the first back to the last.
    pub(crate) fn suggestion_select_prev(&mut self) {
        if self.address_suggestions.is_empty() {
            return;
        }
        let last = self.address_suggestions.len() - 1;
        self.suggestion_selected = Some(match self.suggestion_selected {
            Some(current) => current.checked_sub(1).unwrap_or(last),
            None => last,
        });
    }

    /// Takes the highlighted suggestion URL, leaving command mode and clearing the buffer,
    /// palette, and suggestions. Returns `None` when no suggestion is highlighted, so the
    /// caller falls back to submitting the typed buffer.
    pub(crate) fn take_selected_suggestion(&mut self) -> Option<String> {
        let index = self.suggestion_selected?;
        let url = self.address_suggestions.get(index)?.clone();
        self.command_editor.clear();
        self.interaction_mode = InteractionMode::Reading;
        self.clear_palette();
        self.clear_address_suggestions();
        Some(url)
    }

    /// Opens the history list on `entries`, selecting the first row.
    pub(crate) fn enter_history_mode(&mut self, entries: Vec<HistoryEntry>) {
        self.interaction_mode = InteractionMode::History;
        self.history_entries = entries;
        self.history_selected_index = 0;
    }

    /// Closes the history list and returns to reading mode.
    pub(crate) fn exit_history_mode(&mut self) {
        self.interaction_mode = InteractionMode::Reading;
        self.history_entries.clear();
        self.history_selected_index = 0;
    }

    /// The entries shown in the open history list.
    pub(crate) fn history_entries(&self) -> &[HistoryEntry] {
        &self.history_entries
    }

    /// The index of the highlighted history row.
    pub(crate) fn history_selected(&self) -> usize {
        self.history_selected_index
    }

    /// The highlighted history entry, or `None` when the list is empty.
    pub(crate) fn selected_history_entry(&self) -> Option<&HistoryEntry> {
        self.history_entries.get(self.history_selected_index)
    }

    /// Moves the history highlight to the next row, wrapping at the end.
    pub(crate) fn history_select_next(&mut self) {
        if self.history_entries.is_empty() {
            return;
        }
        self.history_selected_index =
            (self.history_selected_index + 1) % self.history_entries.len();
    }

    /// Moves the history highlight to the previous row, wrapping at the start.
    pub(crate) fn history_select_prev(&mut self) {
        if self.history_entries.is_empty() {
            return;
        }
        let last = self.history_entries.len() - 1;
        self.history_selected_index = self.history_selected_index.checked_sub(1).unwrap_or(last);
    }

    /// Removes the highlighted history entry from the list and clamps the selection to a
    /// remaining row, closing the list once it empties.
    pub(crate) fn remove_selected_history_entry(&mut self) {
        if self.history_selected_index >= self.history_entries.len() {
            return;
        }
        self.history_entries.remove(self.history_selected_index);
        if self.history_entries.is_empty() {
            self.exit_history_mode();
            return;
        }
        let last = self.history_entries.len() - 1;
        self.history_selected_index = self.history_selected_index.min(last);
    }

    /// Opens the cookie inspection popup on `lines`, selecting the first row. The lines are
    /// already sanitized and composed by the caller; this only holds them for display.
    pub(crate) fn enter_cookies_mode(&mut self, lines: Vec<String>) {
        self.interaction_mode = InteractionMode::Cookies;
        self.cookie_lines = lines;
        self.cookie_selected_index = 0;
    }

    /// Closes the cookie inspection popup and returns to reading mode.
    pub(crate) fn exit_cookies_mode(&mut self) {
        self.interaction_mode = InteractionMode::Reading;
        self.cookie_lines.clear();
        self.cookie_selected_index = 0;
    }

    /// The lines shown in the open cookie inspection popup.
    pub(crate) fn cookie_lines(&self) -> &[String] {
        &self.cookie_lines
    }

    /// The index of the highlighted cookie row, so the popup can keep it in view.
    pub(crate) fn cookie_selected(&self) -> usize {
        self.cookie_selected_index
    }

    /// Moves the cookie highlight to the next row, wrapping at the end.
    pub(crate) fn cookie_select_next(&mut self) {
        if self.cookie_lines.is_empty() {
            return;
        }
        self.cookie_selected_index = (self.cookie_selected_index + 1) % self.cookie_lines.len();
    }

    /// Moves the cookie highlight to the previous row, wrapping at the start.
    pub(crate) fn cookie_select_prev(&mut self) {
        if self.cookie_lines.is_empty() {
            return;
        }
        let last = self.cookie_lines.len() - 1;
        self.cookie_selected_index = self.cookie_selected_index.checked_sub(1).unwrap_or(last);
    }

    /// Opens the settings panel on `model`, focusing the first row and remembering the mode
    /// to return to. The model is built by the caller from live settings and controller
    /// state; this only holds it for display and focus.
    pub(crate) fn enter_settings_mode(&mut self, model: SettingsModel) {
        self.settings_return_mode = self.interaction_mode;
        self.interaction_mode = InteractionMode::Settings;
        self.settings_model = Some(model);
        self.settings_focus_index = 0;
    }

    /// Closes the settings panel, dropping its model and restoring the mode that was active
    /// when it opened.
    pub(crate) fn exit_settings_mode(&mut self) {
        self.interaction_mode = self.settings_return_mode;
        self.settings_model = None;
        self.settings_focus_index = 0;
        self.settings_text_edit = None;
    }

    /// The model shown in the open settings panel, or `None` when it is closed.
    pub(crate) fn settings_model(&self) -> Option<&SettingsModel> {
        self.settings_model.as_ref()
    }

    /// The index of the focused settings row within the flattened row list.
    pub(crate) fn settings_focus(&self) -> usize {
        self.settings_focus_index
    }

    /// The focused settings row, or `None` when the panel is closed or has no rows. The caller
    /// reads its identity and environment-override flag before applying a change.
    pub(crate) fn focused_settings_row(&self) -> Option<&SettingsRow> {
        self.settings_model
            .as_ref()?
            .row_at(self.settings_focus_index)
    }

    /// Flips the focused checkbox in the open panel and returns its identity and new state, or
    /// `None` when the panel is closed or the focused row is not a checkbox.
    pub(crate) fn toggle_focused_checkbox(&mut self) -> Option<(SettingId, bool)> {
        let focus = self.settings_focus_index;
        self.settings_model.as_mut()?.toggle_checkbox(focus)
    }

    /// Moves the focused radio group's selection one option in `direction` and returns its
    /// identity and the newly selected policy, or `None` when the panel is closed or the
    /// focused row is not a radio group.
    pub(crate) fn cycle_focused_radio(
        &mut self,
        direction: CycleDirection,
    ) -> Option<(SettingId, CookiePolicy)> {
        let focus = self.settings_focus_index;
        self.settings_model.as_mut()?.cycle_radio(focus, direction)
    }

    /// The id of the focused row when it is a text input, or `None` for a checkbox, radio, or
    /// closed panel. The key router and the focus reconciler read this to decide whether typing
    /// edits a field or acts on a control.
    pub(crate) fn focused_settings_text_id(&self) -> Option<SettingId> {
        self.focused_settings_row().and_then(text_field_id)
    }

    /// Whether the focused row is an editable text input, so the key router sends printable keys
    /// to the draft instead of treating them as control shortcuts.
    pub(crate) fn is_settings_text_field_focused(&self) -> bool {
        self.focused_settings_text_id().is_some()
    }

    /// The id of the field currently being edited, or `None` when no text field has an active
    /// draft. The reconciler compares this against the focused text id to detect focus leaving
    /// or entering a field.
    pub(crate) fn settings_text_edit_id(&self) -> Option<SettingId> {
        self.settings_text_edit.as_ref().map(|edit| edit.id)
    }

    /// Starts editing the text field `id`, seeding its draft from the value the panel currently
    /// shows so the cursor lands at the end of the existing text.
    pub(crate) fn begin_settings_text_edit(&mut self, id: SettingId, now: Instant) {
        let seed = self
            .settings_model
            .as_ref()
            .and_then(|model| model.text_value(id))
            .unwrap_or("")
            .to_string();
        self.settings_text_edit = Some(SettingsTextEdit {
            id,
            editor: TextEditor::seeded(&seed),
            dirty: false,
            last_keystroke: now,
            autosave_suppressed: false,
            error: None,
        });
    }

    /// Drops the active text edit without saving. Called after a focus-leave commit and whenever
    /// the panel closes, so a stale draft never outlives the field it belonged to.
    pub(crate) fn clear_settings_text_edit(&mut self) {
        self.settings_text_edit = None;
    }

    /// Inserts `character` into the active draft, marking it dirty, stamping the keystroke time
    /// that drives the debounce, and clearing any prior rejection so the next idle window retries
    /// the save. A no-op when no field is being edited.
    pub(crate) fn settings_text_input(&mut self, character: char, now: Instant) {
        let Some(edit) = self.settings_text_edit.as_mut() else {
            return;
        };
        edit.editor.insert_char(character);
        edit.dirty = true;
        edit.last_keystroke = now;
        edit.autosave_suppressed = false;
        edit.error = None;
    }

    /// Deletes the character before the cursor in the active draft, with the same dirty and
    /// debounce bookkeeping as an insertion. A no-op when no field is being edited.
    pub(crate) fn settings_text_delete_back(&mut self, now: Instant) {
        let Some(edit) = self.settings_text_edit.as_mut() else {
            return;
        };
        edit.editor.delete_before_cursor();
        edit.dirty = true;
        edit.last_keystroke = now;
        edit.autosave_suppressed = false;
        edit.error = None;
    }

    /// Moves the draft cursor one character left. Cursor moves do not touch dirtiness or the
    /// debounce, so drifting through a value never triggers or delays a save.
    pub(crate) fn settings_text_move_left(&mut self) {
        if let Some(edit) = self.settings_text_edit.as_mut() {
            edit.editor.move_left();
        }
    }

    /// Moves the draft cursor one character right, with the same no-save semantics as a left
    /// move.
    pub(crate) fn settings_text_move_right(&mut self) {
        if let Some(edit) = self.settings_text_edit.as_mut() {
            edit.editor.move_right();
        }
    }

    /// Handles `Esc` on a text field: an unsaved draft reverts to the saved value and the panel
    /// stays open; a clean field asks the caller to close the panel. This makes the first `Esc`
    /// discard an in-progress edit and a second `Esc` leave the panel, so a mistyped value is
    /// never persisted on the way out.
    pub(crate) fn settings_text_cancel(&mut self) -> SettingsEscOutcome {
        let Some((id, dirty)) = self
            .settings_text_edit
            .as_ref()
            .map(|edit| (edit.id, edit.dirty))
        else {
            return SettingsEscOutcome::ClosePanel;
        };
        if !dirty {
            return SettingsEscOutcome::ClosePanel;
        }
        let saved = self
            .settings_model
            .as_ref()
            .and_then(|model| model.text_value(id))
            .unwrap_or("")
            .to_string();
        if let Some(edit) = self.settings_text_edit.as_mut() {
            edit.editor.set_buffer(&saved);
            edit.dirty = false;
            edit.autosave_suppressed = false;
            edit.error = None;
        }
        SettingsEscOutcome::Reverted
    }

    /// The save a due debounce asks for: `Some` only when a field is dirty, its idle window has
    /// elapsed, and its last save was not rejected. The controller-facing save runs in the app.
    pub(crate) fn settings_text_due_save(&self, now: Instant) -> Option<SettingsTextSave> {
        let edit = self.settings_text_edit.as_ref()?;
        if edit.autosave_suppressed {
            return None;
        }
        if !should_save(
            now,
            edit.last_keystroke,
            edit.dirty,
            SETTINGS_AUTOSAVE_DEBOUNCE,
        ) {
            return None;
        }
        Some(SettingsTextSave {
            id: edit.id,
            value: edit.editor.buffer().to_string(),
        })
    }

    /// The save a focus-leave asks for: `Some` for any dirty field, ignoring the debounce and a
    /// prior rejection, so moving off a field commits a valid edit at once.
    pub(crate) fn settings_text_pending_save(&self) -> Option<SettingsTextSave> {
        let edit = self.settings_text_edit.as_ref()?;
        if !edit.dirty {
            return None;
        }
        Some(SettingsTextSave {
            id: edit.id,
            value: edit.editor.buffer().to_string(),
        })
    }

    /// Records a successful save: the stored search values replace the shown values on both
    /// search text rows, and the active draft is marked clean with no error.
    pub(crate) fn mark_settings_text_saved(&mut self, base_url: &str, query_parameter: &str) {
        if let Some(model) = self.settings_model.as_mut() {
            model.set_text_value(SettingId::SearchBaseUrl, base_url);
            model.set_text_value(SettingId::SearchQueryParameter, query_parameter);
        }
        if let Some(edit) = self.settings_text_edit.as_mut() {
            edit.dirty = false;
            edit.autosave_suppressed = false;
            edit.error = None;
        }
    }

    /// Records a rejected save: the field stays dirty and shows `message` inline, and auto-save
    /// is held off until the next keystroke so the rejection is not retried every tick.
    pub(crate) fn mark_settings_text_save_failed(&mut self, message: String) {
        if let Some(edit) = self.settings_text_edit.as_mut() {
            edit.autosave_suppressed = true;
            edit.error = Some(message);
        }
    }

    /// The active draft buffer for the focused text field, or `None` when no field is being
    /// edited, so the renderer can show the draft in place of the saved value.
    pub(crate) fn settings_text_draft(&self) -> Option<&str> {
        self.settings_text_edit
            .as_ref()
            .map(|edit| edit.editor.buffer())
    }

    /// The draft buffer and cursor byte offset for the focused text field, for placing the
    /// terminal cursor. `None` when no field is being edited.
    pub(crate) fn settings_text_cursor(&self) -> Option<(&str, usize)> {
        self.settings_text_edit
            .as_ref()
            .map(|edit| (edit.editor.buffer(), edit.editor.cursor_byte_offset()))
    }

    /// The inline error shown on the focused text field after a rejected save, or `None` when
    /// the field is clean or its last save succeeded.
    pub(crate) fn settings_text_error(&self) -> Option<&str> {
        self.settings_text_edit
            .as_ref()
            .and_then(|edit| edit.error.as_deref())
    }

    /// Updates the live search-enabled flag the palette filter reads, so toggling the setting
    /// adds or removes `/search` from the palette without a restart.
    pub(crate) fn set_search_enabled(&mut self, search_enabled: bool) {
        self.search_enabled = search_enabled;
    }

    /// Moves the settings focus to the next row, wrapping at the end. A no-op when the panel
    /// is closed or has no rows.
    pub(crate) fn settings_focus_next(&mut self) {
        let row_count = self.settings_row_count();
        if row_count == 0 {
            return;
        }
        self.settings_focus_index = (self.settings_focus_index + 1) % row_count;
    }

    /// Moves the settings focus to the previous row, wrapping at the start. A no-op when the
    /// panel is closed or has no rows.
    pub(crate) fn settings_focus_prev(&mut self) {
        let row_count = self.settings_row_count();
        if row_count == 0 {
            return;
        }
        let last = row_count - 1;
        self.settings_focus_index = self.settings_focus_index.checked_sub(1).unwrap_or(last);
    }

    /// The number of focusable rows in the open panel, or zero when it is closed.
    fn settings_row_count(&self) -> usize {
        self.settings_model
            .as_ref()
            .map_or(0, SettingsModel::row_count)
    }

    /// Enters interactive navigation mode and focuses the target at `index`.
    pub(crate) fn enter_interactive_navigation(&mut self, index: usize) {
        self.interaction_mode = InteractionMode::InteractiveNavigation;
        self.focused_interactive_index = Some(index);
    }

    /// Exits interactive navigation mode and clears the focused target and citation
    /// preview.
    pub(crate) fn exit_interactive_navigation(&mut self) {
        self.interaction_mode = InteractionMode::Reading;
        self.focused_interactive_index = None;
        self.clear_citation_preview();
    }

    /// Advances focus to the next interactive target, wrapping at the end.
    pub(crate) fn focus_next_interactive(&mut self, target_count: usize) {
        if target_count == 0 {
            return;
        }
        self.focused_interactive_index = Some(match self.focused_interactive_index {
            Some(current) => (current + 1) % target_count,
            None => 0,
        });
    }

    /// Moves focus to the previous interactive target, wrapping at the start.
    pub(crate) fn focus_previous_interactive(&mut self, target_count: usize) {
        if target_count == 0 {
            return;
        }
        self.focused_interactive_index = Some(match self.focused_interactive_index {
            Some(current) => current.checked_sub(1).unwrap_or(target_count - 1),
            None => target_count - 1,
        });
    }

    /// Enters the text-edit sub-mode for `node_id`, seeded with `initial_value`. A
    /// sensitive field seeds from an empty buffer regardless of `initial_value`, since a
    /// password's typed characters are never restored once the page has loaded.
    pub(crate) fn enter_field_text_edit(
        &mut self,
        node_id: NodeId,
        initial_value: &str,
        sensitive: bool,
    ) {
        let seed = if sensitive { "" } else { initial_value };
        self.focused_field_edit = Some(FieldEditState::Text {
            node_id,
            editor: TextEditor::seeded(seed),
            sensitive,
        });
    }

    /// The node id and sensitivity of the field being text-edited, or `None` when no
    /// text-edit sub-mode is active.
    pub(crate) fn field_text_edit_target(&self) -> Option<(NodeId, bool)> {
        match &self.focused_field_edit {
            Some(FieldEditState::Text {
                node_id, sensitive, ..
            }) => Some((*node_id, *sensitive)),
            _ => None,
        }
    }

    /// The draft buffer and cursor byte offset for the active field text edit, for
    /// drawing the field and placing the terminal cursor. `None` when no text-edit
    /// sub-mode is active.
    pub(crate) fn field_text_edit_cursor(&self) -> Option<(&str, usize)> {
        match &self.focused_field_edit {
            Some(FieldEditState::Text { editor, .. }) => {
                Some((editor.buffer(), editor.cursor_byte_offset()))
            }
            _ => None,
        }
    }

    /// Inserts `character` into the active field text-edit draft. A no-op when no
    /// text-edit sub-mode is active.
    pub(crate) fn field_text_input(&mut self, character: char) {
        if let Some(FieldEditState::Text { editor, .. }) = &mut self.focused_field_edit {
            editor.insert_char(character);
        }
    }

    /// Deletes the character before the cursor in the active field text-edit draft. A
    /// no-op when no text-edit sub-mode is active.
    pub(crate) fn field_text_delete_back(&mut self) {
        if let Some(FieldEditState::Text { editor, .. }) = &mut self.focused_field_edit {
            editor.delete_before_cursor();
        }
    }

    pub(crate) fn field_text_move_left(&mut self) {
        if let Some(FieldEditState::Text { editor, .. }) = &mut self.focused_field_edit {
            editor.move_left();
        }
    }

    pub(crate) fn field_text_move_right(&mut self) {
        if let Some(FieldEditState::Text { editor, .. }) = &mut self.focused_field_edit {
            editor.move_right();
        }
    }

    /// Takes the field text-edit draft's node id, sensitivity, and buffer, and leaves
    /// the sub-mode. Used by both commit and cancel: the caller decides whether to write
    /// the buffer back through the controller.
    pub(crate) fn take_field_text_edit(&mut self) -> Option<(NodeId, bool, String)> {
        match self.focused_field_edit.take() {
            Some(FieldEditState::Text {
                node_id,
                editor,
                sensitive,
            }) => Some((node_id, sensitive, editor.buffer().to_string())),
            other => {
                self.focused_field_edit = other;
                None
            }
        }
    }

    /// Enters the multi-select expansion sub-mode for `node_id`, snapshotting `options`
    /// as they are when the mode is entered.
    pub(crate) fn enter_field_multi_select(&mut self, node_id: NodeId, options: Vec<SelectOption>) {
        self.focused_field_edit = Some(FieldEditState::MultiSelect {
            node_id,
            options,
            cursor: 0,
        });
    }

    /// The multi-select sub-mode's node id, option snapshot, and highlighted cursor, or
    /// `None` when the sub-mode is not active.
    pub(crate) fn field_multi_select(&self) -> Option<(NodeId, &[SelectOption], usize)> {
        match &self.focused_field_edit {
            Some(FieldEditState::MultiSelect {
                node_id,
                options,
                cursor,
            }) => Some((*node_id, options.as_slice(), *cursor)),
            _ => None,
        }
    }

    /// Moves the multi-select cursor up by one, clamped at the first option (a bounded
    /// list, not a cycle).
    pub(crate) fn field_multi_select_move_up(&mut self) {
        if let Some(FieldEditState::MultiSelect { cursor, .. }) = &mut self.focused_field_edit {
            *cursor = cursor.saturating_sub(1);
        }
    }

    /// Moves the multi-select cursor down by one, clamped at the last option.
    pub(crate) fn field_multi_select_move_down(&mut self) {
        if let Some(FieldEditState::MultiSelect {
            cursor, options, ..
        }) = &mut self.focused_field_edit
        {
            let last = options.len().saturating_sub(1);
            *cursor = (*cursor + 1).min(last);
        }
    }

    /// Leaves the multi-select expansion sub-mode. Every toggle already applied
    /// instantly, so there is nothing to discard.
    pub(crate) fn exit_field_multi_select(&mut self) {
        if matches!(
            self.focused_field_edit,
            Some(FieldEditState::MultiSelect { .. })
        ) {
            self.focused_field_edit = None;
        }
    }

    /// Opens the `POST` submission confirmation view for `submit_button`, highlighting
    /// `Cancel` first so an accidental `Enter` never sends the request.
    pub(crate) fn enter_submit_confirmation(&mut self, submit_button: NodeId, destination: String) {
        self.interaction_mode = InteractionMode::SubmitConfirmation;
        self.submit_confirmation = Some(SubmitConfirmation {
            submit_button,
            destination,
            choice: SubmitChoice::Cancel,
        });
    }

    /// Closes the confirmation view and returns to interactive navigation with the same
    /// field still focused.
    pub(crate) fn exit_submit_confirmation(&mut self) {
        self.interaction_mode = InteractionMode::InteractiveNavigation;
        self.submit_confirmation = None;
    }

    /// The open confirmation view's pending button, destination, and highlighted
    /// choice, or `None` when it is closed.
    pub(crate) fn submit_confirmation(&self) -> Option<(NodeId, &str, SubmitChoice)> {
        self.submit_confirmation.as_ref().map(|state| {
            (
                state.submit_button,
                state.destination.as_str(),
                state.choice,
            )
        })
    }

    /// Toggles the confirmation view's highlighted choice between `Submit` and
    /// `Cancel`. A no-op when the view is closed.
    pub(crate) fn submit_confirmation_toggle(&mut self) {
        if let Some(state) = self.submit_confirmation.as_mut() {
            state.choice = state.choice.toggled();
        }
    }

    /// Records `url` as visited for this session.
    pub(crate) fn mark_visited(&mut self, url: &str) {
        self.visited_urls.insert(url.to_string());
    }

    /// Returns `true` when `url` has been visited this session.
    pub(crate) fn is_visited(&self, url: &str) -> bool {
        self.visited_urls.contains(url)
    }

    pub(crate) fn enter_command_mode(&mut self, first_char: char) {
        self.interaction_mode = InteractionMode::Command;
        self.command_editor.clear();
        self.command_editor.insert_char(first_char);
        self.refresh_palette();
    }

    pub(crate) fn command_buffer(&self) -> &str {
        self.command_editor.buffer()
    }

    pub(crate) fn cursor_byte_offset(&self) -> usize {
        self.command_editor.cursor_byte_offset()
    }

    pub(crate) fn command_append_char(&mut self, ch: char) {
        self.command_editor.insert_char(ch);
        self.refresh_palette();
    }

    pub(crate) fn command_move_left(&mut self) {
        self.command_editor.move_left();
    }

    pub(crate) fn command_move_right(&mut self) {
        self.command_editor.move_right();
    }

    pub(crate) fn command_delete_before_cursor(&mut self) {
        self.command_editor.delete_before_cursor();
        self.refresh_palette();
    }

    pub(crate) fn cancel_command_mode(&mut self) {
        self.interaction_mode = InteractionMode::Reading;
        self.command_editor.clear();
        self.clear_palette();
        self.clear_address_suggestions();
    }

    /// Handles a backspace in command mode. Deleting the leading `/` exits command mode
    /// entirely, the same as cancelling, since a buffer without the `/` is no longer a
    /// command. Any other backspace deletes the character before the cursor and refreshes
    /// the palette.
    pub(crate) fn command_delete_or_exit(&mut self) {
        if self.deletes_leading_slash() {
            self.cancel_command_mode();
            return;
        }
        self.command_delete_before_cursor();
    }

    /// True when the next backspace would remove the leading `/`, that is, the buffer is a
    /// slash buffer and the cursor sits just after that `/`.
    fn deletes_leading_slash(&self) -> bool {
        self.is_palette_active() && self.command_editor.cursor_byte_offset() == '/'.len_utf8()
    }

    /// The buffer to dispatch on Enter, resolving the palette selection. When the palette
    /// is active, the typed token is not itself an exact command, and a row is highlighted,
    /// the highlighted command's canonical name replaces the typed token (its argument
    /// remainder is preserved). Otherwise the raw buffer is dispatched, so an exactly typed
    /// command runs directly and an empty match list falls through to the unknown-command
    /// path. Leaves command mode and clears the palette as it hands the buffer back.
    pub(crate) fn take_submit_buffer(&mut self) -> String {
        let resolved = self.resolved_submit_buffer();
        self.command_editor.clear();
        self.interaction_mode = InteractionMode::Reading;
        self.clear_palette();
        self.clear_address_suggestions();
        resolved
    }

    fn resolved_submit_buffer(&self) -> String {
        let buffer = self.command_editor.buffer();
        if !self.is_palette_active() {
            return buffer.to_string();
        }
        let (token, remainder) = command::parse_command_input(buffer);
        if command::resolve(token).is_some() {
            return buffer.to_string();
        }
        let Some(selected) = self.palette_matches.get(self.palette_selected_index) else {
            return buffer.to_string();
        };
        if remainder.is_empty() {
            return format!("/{}", selected.spec.name);
        }
        format!("/{} {remainder}", selected.spec.name)
    }

    /// True when the command buffer begins with `/`, meaning the slash-command palette is
    /// filtering rather than the bar accepting a URL. Derived from the buffer so there is
    /// no separate flag to keep in sync.
    pub(crate) fn is_palette_active(&self) -> bool {
        self.command_editor.buffer().starts_with('/')
    }

    /// The ranked command matches shown in the palette for the current buffer. Empty when
    /// the palette is inactive or nothing matches.
    pub(crate) fn palette_matches(&self) -> &[CommandMatch] {
        &self.palette_matches
    }

    /// Index of the highlighted palette row within `palette_matches`.
    pub(crate) fn palette_selected(&self) -> usize {
        self.palette_selected_index
    }

    /// Moves the palette highlight to the next row, wrapping from the last back to the
    /// first. A no-op when nothing matches.
    pub(crate) fn palette_select_next(&mut self) {
        if self.palette_matches.is_empty() {
            return;
        }
        self.palette_selected_index =
            (self.palette_selected_index + 1) % self.palette_matches.len();
    }

    /// Moves the palette highlight to the previous row, wrapping from the first back to the
    /// last. A no-op when nothing matches.
    pub(crate) fn palette_select_prev(&mut self) {
        if self.palette_matches.is_empty() {
            return;
        }
        self.palette_selected_index = self
            .palette_selected_index
            .checked_sub(1)
            .unwrap_or(self.palette_matches.len() - 1);
    }

    /// Completes the buffer to the highlighted command: `/` plus its canonical name, cursor
    /// at the end, and a trailing space when the command takes an argument so the user can
    /// type it. A no-op when nothing matches.
    pub(crate) fn palette_complete(&mut self) {
        let Some(selected) = self.palette_matches.get(self.palette_selected_index) else {
            return;
        };
        let mut completed = format!("/{}", selected.spec.name);
        if selected.spec.takes_argument {
            completed.push(' ');
        }
        self.command_editor.set_buffer(&completed);
        self.refresh_palette();
    }

    /// Recomputes the filtered command list from the command token (the run after the
    /// leading `/` up to the first space) and resets the selection to the first row. Using
    /// the token, not the whole buffer, keeps the palette focused on the command while the
    /// user types its argument. A changed filter always starts the selection at the top; the
    /// selection only moves via `palette_select_next`/`palette_select_prev`.
    fn refresh_palette(&mut self) {
        if !self.is_palette_active() {
            self.clear_palette();
            return;
        }
        let (token, _remainder) = command::parse_command_input(self.command_editor.buffer());
        let query = token.to_string();
        self.palette_matches = command::filter(&query);
        if !self.search_enabled {
            self.palette_matches
                .retain(|found| found.spec.kind != CommandKind::Search);
        }
        self.palette_selected_index = 0;
    }

    fn clear_palette(&mut self) {
        self.palette_matches.clear();
        self.palette_selected_index = 0;
    }

    pub(crate) fn current_hint(&self) -> &str {
        self.transient_message
            .as_deref()
            .unwrap_or(READING_HINTS[self.hint_index])
    }

    /// The active transient message, if one is set and unexpired. Feeds the hints bar's
    /// system-message slot; `None` leaves that slot empty.
    pub(crate) fn transient_message(&self) -> Option<&str> {
        self.transient_message.as_deref()
    }

    pub(crate) fn advance_hint_if_due(&mut self, now: Instant) {
        if now.duration_since(self.last_hint_advance) >= HINT_ROTATION_INTERVAL {
            self.hint_index = (self.hint_index + 1) % READING_HINTS.len();
            self.last_hint_advance = now;
        }
    }

    pub(crate) fn clear_transient_if_expired(&mut self, now: Instant) {
        let Some(set_at) = self.transient_set_at else {
            return;
        };
        if now.duration_since(set_at) >= TRANSIENT_HINT_DURATION {
            self.quit_armed = false;
            self.refresh_armed = false;
            self.transient_message = None;
            self.transient_set_at = None;
        }
    }

    pub(crate) fn set_transient_hint(&mut self, hint: &'static str, now: Instant) {
        self.transient_message = Some(hint.to_string());
        self.transient_set_at = Some(now);
    }

    /// Sets an owned transient message with the same five-second expiry as the static
    /// arm hints, used for runtime confirmations like the copy-count message.
    pub(crate) fn set_transient_message(&mut self, message: String, now: Instant) {
        self.transient_message = Some(message);
        self.transient_set_at = Some(now);
    }

    pub(crate) fn clear_transient(&mut self) {
        self.transient_message = None;
        self.transient_set_at = None;
    }
}

#[cfg(test)]
#[path = "ui_state_tests.rs"]
mod tests;
