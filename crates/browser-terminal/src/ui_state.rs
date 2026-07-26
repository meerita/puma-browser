// @file crates/browser-terminal/src/ui_state.rs
// @description UiState struct and InteractionMode enum centralising mutable chrome state.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::collections::HashSet;
use std::time::{Duration, Instant};

use crate::command::{self, CommandKind, CommandMatch};

pub(crate) const READING_HINTS: &[&str] = &[
    "Type a URL or press / for commands",
    "j · k or ↑ · ↓ to scroll  ·  Space / b to page",
    "g to jump to top  ·  G to jump to bottom",
    "Esc to quit  ·  r to refresh the page",
];

const HINT_ROTATION_INTERVAL: Duration = Duration::from_secs(30);
const TRANSIENT_HINT_DURATION: Duration = Duration::from_secs(5);

pub(crate) enum InteractionMode {
    Reading,
    Command,
    LinkNavigation,
}

pub(crate) struct UiState {
    pub(crate) interaction_mode: InteractionMode,
    pub(crate) quit_armed: bool,
    pub(crate) refresh_armed: bool,
    pub(crate) focused_link_index: Option<usize>,
    pub(crate) visited_urls: HashSet<String>,
    hint_index: usize,
    last_hint_advance: Instant,
    transient_message: Option<String>,
    transient_set_at: Option<Instant>,
    command_buffer: String,
    cursor_byte_offset: usize,
    palette_matches: Vec<CommandMatch>,
    palette_selected_index: usize,
    pending_fragment: Option<String>,
    search_enabled: bool,
}

impl UiState {
    pub(crate) fn new(search_enabled: bool) -> Self {
        Self {
            interaction_mode: InteractionMode::Reading,
            quit_armed: false,
            refresh_armed: false,
            focused_link_index: None,
            visited_urls: HashSet::new(),
            hint_index: 0,
            last_hint_advance: Instant::now(),
            transient_message: None,
            transient_set_at: None,
            command_buffer: String::new(),
            cursor_byte_offset: 0,
            palette_matches: Vec::new(),
            palette_selected_index: 0,
            pending_fragment: None,
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

    pub(crate) fn is_in_command_mode(&self) -> bool {
        matches!(self.interaction_mode, InteractionMode::Command)
    }

    pub(crate) fn is_in_link_navigation(&self) -> bool {
        matches!(self.interaction_mode, InteractionMode::LinkNavigation)
    }

    /// Enters link navigation mode and focuses the link at `index`.
    pub(crate) fn enter_link_navigation(&mut self, index: usize) {
        self.interaction_mode = InteractionMode::LinkNavigation;
        self.focused_link_index = Some(index);
    }

    /// Exits link navigation mode and clears the focused link.
    pub(crate) fn exit_link_navigation(&mut self) {
        self.interaction_mode = InteractionMode::Reading;
        self.focused_link_index = None;
    }

    /// Advances focus to the next link, wrapping at the end.
    pub(crate) fn focus_next_link(&mut self, link_count: usize) {
        if link_count == 0 {
            return;
        }
        self.focused_link_index = Some(match self.focused_link_index {
            Some(current) => (current + 1) % link_count,
            None => 0,
        });
    }

    /// Moves focus to the previous link, wrapping at the start.
    pub(crate) fn focus_previous_link(&mut self, link_count: usize) {
        if link_count == 0 {
            return;
        }
        self.focused_link_index = Some(match self.focused_link_index {
            Some(current) => current.checked_sub(1).unwrap_or(link_count - 1),
            None => link_count - 1,
        });
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
        self.command_buffer.clear();
        self.cursor_byte_offset = 0;
        self.command_buffer.push(first_char);
        self.cursor_byte_offset = first_char.len_utf8();
        self.refresh_palette();
    }

    pub(crate) fn command_buffer(&self) -> &str {
        &self.command_buffer
    }

    pub(crate) fn cursor_byte_offset(&self) -> usize {
        self.cursor_byte_offset
    }

    pub(crate) fn command_append_char(&mut self, ch: char) {
        self.command_buffer.insert(self.cursor_byte_offset, ch);
        self.cursor_byte_offset += ch.len_utf8();
        self.refresh_palette();
    }

    pub(crate) fn command_move_left(&mut self) {
        if self.cursor_byte_offset == 0 {
            return;
        }
        let before = &self.command_buffer[..self.cursor_byte_offset];
        let prev_char_len = before.chars().next_back().map_or(0, |c| c.len_utf8());
        self.cursor_byte_offset -= prev_char_len;
    }

    pub(crate) fn command_move_right(&mut self) {
        if self.cursor_byte_offset >= self.command_buffer.len() {
            return;
        }
        let after = &self.command_buffer[self.cursor_byte_offset..];
        let next_char_len = after.chars().next().map_or(0, |c| c.len_utf8());
        self.cursor_byte_offset += next_char_len;
    }

    pub(crate) fn command_delete_before_cursor(&mut self) {
        if self.cursor_byte_offset == 0 {
            return;
        }
        let before = &self.command_buffer[..self.cursor_byte_offset];
        let prev_char_len = before.chars().next_back().map_or(0, |c| c.len_utf8());
        let new_offset = self.cursor_byte_offset - prev_char_len;
        self.command_buffer
            .drain(new_offset..self.cursor_byte_offset);
        self.cursor_byte_offset = new_offset;
        self.refresh_palette();
    }

    pub(crate) fn cancel_command_mode(&mut self) {
        self.interaction_mode = InteractionMode::Reading;
        self.command_buffer.clear();
        self.cursor_byte_offset = 0;
        self.clear_palette();
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
        self.is_palette_active() && self.cursor_byte_offset == '/'.len_utf8()
    }

    /// The buffer to dispatch on Enter, resolving the palette selection. When the palette
    /// is active, the typed token is not itself an exact command, and a row is highlighted,
    /// the highlighted command's canonical name replaces the typed token (its argument
    /// remainder is preserved). Otherwise the raw buffer is dispatched, so an exactly typed
    /// command runs directly and an empty match list falls through to the unknown-command
    /// path. Leaves command mode and clears the palette as it hands the buffer back.
    pub(crate) fn take_submit_buffer(&mut self) -> String {
        let resolved = self.resolved_submit_buffer();
        self.command_buffer.clear();
        self.cursor_byte_offset = 0;
        self.interaction_mode = InteractionMode::Reading;
        self.clear_palette();
        resolved
    }

    fn resolved_submit_buffer(&self) -> String {
        if !self.is_palette_active() {
            return self.command_buffer.clone();
        }
        let (token, remainder) = command::parse_command_input(&self.command_buffer);
        if command::resolve(token).is_some() {
            return self.command_buffer.clone();
        }
        let Some(selected) = self.palette_matches.get(self.palette_selected_index) else {
            return self.command_buffer.clone();
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
        self.command_buffer.starts_with('/')
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
        self.command_buffer = completed;
        self.cursor_byte_offset = self.command_buffer.len();
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
        let (token, _remainder) = command::parse_command_input(&self.command_buffer);
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
