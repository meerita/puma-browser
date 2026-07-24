// @file crates/browser-terminal/src/error.rs
// @description Terminal adapter error taxonomy; maps CoreError into safe user-facing status text.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::CoreError;
use thiserror::Error;

/// Errors produced by the terminal output adapter.
///
/// Wraps [`CoreError`] and adds display-specific variants. The wrapped source is kept
/// for logging only; it never reaches the terminal as text. Use [`TerminalError::user_message`]
/// to obtain the string shown to the user.
#[derive(Debug, Error)]
pub enum TerminalError {
    #[error("core error")]
    Core(#[source] CoreError),

    #[error("render failed")]
    RenderFailed,
}

impl From<CoreError> for TerminalError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

impl TerminalError {
    /// Returns a short, factual status message safe to write to the terminal.
    ///
    /// The returned string contains no file paths, crate names, SQL, raw driver text,
    /// or internal variant names, and never carries an escape sequence from the wrapped
    /// error. Full diagnostic detail belongs in `tracing` at `debug`/`trace` level.
    pub fn user_message(&self) -> String {
        match self {
            Self::Core(error) => Self::core_message(error).to_string(),
            Self::RenderFailed => "Could not render the page".to_string(),
        }
    }

    fn core_message(error: &CoreError) -> &'static str {
        match error {
            CoreError::NavigationFailed => "Navigation failed",
            CoreError::TabNotFound => "Tab not found",
            CoreError::Network(_) => "Connection failed",
            CoreError::LocalFileNotFound => "File not found",
            CoreError::LocalPathIsDirectory => "Path is a directory, not a file",
            CoreError::LocalFileTooLarge => "File is too large",
            CoreError::LocalFileReadFailed => "Could not read the file",
            CoreError::Parse(_) => "Could not read the page",
            CoreError::Layout(_) => "Could not display the page",
            CoreError::Storage(_) => "Could not access local data",
            CoreError::Privacy(_) => "Blocked by privacy policy",
        }
    }
}
