// @file crates/browser-terminal/src/ui_state.rs
// @description UiState struct and InteractionMode enum centralising mutable chrome state.
// @layer terminal
// @created meerita <meerita@icloud.com>

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
    // Command variant added in Phase 3
}

pub(crate) struct UiState {
    // Used in Phase 3 when Command mode is introduced.
    #[allow(dead_code)]
    pub(crate) interaction_mode: InteractionMode,
    pub(crate) quit_armed: bool,
    pub(crate) refresh_armed: bool,
    hint_index: usize,
    last_hint_advance: Instant,
    transient_hint: Option<&'static str>,
    transient_set_at: Option<Instant>,
}

impl UiState {
    pub(crate) fn new() -> Self {
        Self {
            interaction_mode: InteractionMode::Reading,
            quit_armed: false,
            refresh_armed: false,
            hint_index: 0,
            last_hint_advance: Instant::now(),
            transient_hint: None,
            transient_set_at: None,
        }
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
