// @file crates/browser-terminal/src/initial_view.rs
// @description The initial page state the CLI hands the terminal app: page, blank, or error.
// @layer terminal
// @created meerita <meerita@icloud.com>

/// What the terminal shows when it starts, decided by the composition root before the
/// event loop runs.
///
/// `Page` means the navigation core already holds a loaded document to render. `Blank`
/// means no URL was given. `Error` carries the safe, already-sanitized status message
/// from a failed load; it is display text only and never raw error detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialView {
    Page,
    Blank,
    Error(String),
}
