// @file crates/browser-terminal/src/ui_state.rs
// @description UiState struct and InteractionMode enum centralising mutable chrome state.
// @layer terminal
// @created meerita <meerita@icloud.com>

pub(crate) enum InteractionMode {
    Reading,
    // Command variant added in Phase 3
}

pub(crate) struct UiState {
    // Used in Phase 3 when Command mode is introduced.
    #[allow(dead_code)]
    pub(crate) interaction_mode: InteractionMode,
    pub(crate) quit_armed: bool,
}

impl UiState {
    pub(crate) fn new() -> Self {
        Self {
            interaction_mode: InteractionMode::Reading,
            quit_armed: false,
        }
    }
}
