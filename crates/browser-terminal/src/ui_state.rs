// @file crates/browser-terminal/src/ui_state.rs
// @description UiState struct and InteractionMode enum centralising mutable chrome state.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::collections::HashSet;
use std::time::{Duration, Instant};

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
    transient_hint: Option<&'static str>,
    transient_set_at: Option<Instant>,
    command_buffer: String,
    cursor_byte_offset: usize,
}

impl UiState {
    pub(crate) fn new() -> Self {
        Self {
            interaction_mode: InteractionMode::Reading,
            quit_armed: false,
            refresh_armed: false,
            focused_link_index: None,
            visited_urls: HashSet::new(),
            hint_index: 0,
            last_hint_advance: Instant::now(),
            transient_hint: None,
            transient_set_at: None,
            command_buffer: String::new(),
            cursor_byte_offset: 0,
        }
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
    }

    pub(crate) fn cancel_command_mode(&mut self) {
        self.interaction_mode = InteractionMode::Reading;
        self.command_buffer.clear();
        self.cursor_byte_offset = 0;
    }

    pub(crate) fn take_command_buffer(&mut self) -> String {
        let buffer = std::mem::take(&mut self.command_buffer);
        self.cursor_byte_offset = 0;
        self.interaction_mode = InteractionMode::Reading;
        buffer
    }

    pub(crate) fn current_hint(&self) -> &str {
        self.transient_hint
            .unwrap_or(READING_HINTS[self.hint_index])
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
            self.transient_hint = None;
            self.transient_set_at = None;
        }
    }

    pub(crate) fn set_transient_hint(&mut self, hint: &'static str, now: Instant) {
        self.transient_hint = Some(hint);
        self.transient_set_at = Some(now);
    }

    pub(crate) fn clear_transient(&mut self) {
        self.transient_hint = None;
        self.transient_set_at = None;
    }
}

#[cfg(test)]
#[path = "ui_state_tests.rs"]
mod tests;
