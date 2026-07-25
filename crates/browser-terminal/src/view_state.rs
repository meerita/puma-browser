// @file crates/browser-terminal/src/view_state.rs
// @description The view state the terminal app tracks across the session: page, blank, or error.
// @layer terminal
// @created meerita <meerita@icloud.com>

/// The current view state of the terminal session.
///
/// `Page` means the navigation core holds a loaded document to render. `Blank`
/// means no URL has been navigated to. `Error` carries the safe, already-sanitized
/// status message from a failed load; it is display text only and never raw error detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewState {
    Page,
    Blank,
    Error(String),
}
