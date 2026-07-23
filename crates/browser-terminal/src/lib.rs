//! @file crates/browser-terminal/src/lib.rs
//! @description Terminal adapter crate root: app skeleton and error taxonomy over browser-core.
//! @layer terminal
//! @created meerita <meerita@icloud.com>

mod error;

pub use error::TerminalError;

use browser_core::NavigationController;

/// Drives the terminal user interface over the navigation core.
///
/// This is the output adapter the terminal binary builds on. It is a placeholder in
/// this milestone: Ratatui rendering, the command bar, themes, and input handling are
/// not implemented yet, so [`TerminalApp::run`] reports a typed error rather than
/// drawing anything.
#[derive(Debug)]
pub struct TerminalApp {
    controller: NavigationController,
}

impl TerminalApp {
    pub fn new(controller: NavigationController) -> Self {
        Self { controller }
    }

    /// Borrows the navigation core this adapter drives.
    pub fn controller(&self) -> &NavigationController {
        &self.controller
    }

    /// Runs the terminal event loop until the user exits.
    ///
    /// Not implemented in this milestone; returns [`TerminalError::RenderFailed`].
    pub fn run(&mut self) -> Result<(), TerminalError> {
        Err(TerminalError::RenderFailed)
    }
}
