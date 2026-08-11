// @file crates/browser-core/src/error.rs
// @description Core navigation error taxonomy; maps inner-crate errors into core vocabulary.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_html::HtmlError;
use browser_layout::LayoutError;
use browser_network::NetworkError;
use browser_privacy::PrivacyError;
use browser_storage::StorageError;
use thiserror::Error;

/// Errors produced by the navigation core.
///
/// Inner-crate errors are wrapped and re-described in core vocabulary before they
/// cross outward. Callers never see a raw `reqwest`, `rusqlite`, or other inner
/// driver type through these variants.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("navigation failed")]
    NavigationFailed,

    #[error("tab not found")]
    TabNotFound,

    #[error("network error")]
    Network(#[source] NetworkError),

    #[error("local file not found")]
    LocalFileNotFound,

    #[error("local path is a directory")]
    LocalPathIsDirectory,

    #[error("local file too large")]
    LocalFileTooLarge,

    #[error("local file read failed")]
    LocalFileReadFailed,

    #[error("document parse error")]
    Parse(#[source] HtmlError),

    #[error("layout error")]
    Layout(#[source] LayoutError),

    #[error("storage error")]
    Storage(#[source] StorageError),

    #[error("privacy policy error")]
    Privacy(#[source] PrivacyError),
}

impl From<NetworkError> for CoreError {
    fn from(error: NetworkError) -> Self {
        // Local-file failures get distinct variants so the terminal can render a correct,
        // path-free message instead of collapsing them into a generic connection failure.
        // Every genuine network failure keeps mapping to `Network`.
        match error {
            NetworkError::FileNotFound => Self::LocalFileNotFound,
            NetworkError::PathIsDirectory => Self::LocalPathIsDirectory,
            NetworkError::FileTooLarge => Self::LocalFileTooLarge,
            NetworkError::FileReadFailed => Self::LocalFileReadFailed,
            NetworkError::InvalidUrl
            | NetworkError::UnsupportedScheme { .. }
            | NetworkError::DnsFailure
            | NetworkError::TlsError
            | NetworkError::Timeout
            | NetworkError::TooManyRedirects
            | NetworkError::ResponseTooLarge
            | NetworkError::ResponseHeadersTooLarge
            | NetworkError::RequestFailed
            | NetworkError::Decode => Self::Network(error),
        }
    }
}

impl From<HtmlError> for CoreError {
    fn from(error: HtmlError) -> Self {
        Self::Parse(error)
    }
}

impl From<LayoutError> for CoreError {
    fn from(error: LayoutError) -> Self {
        Self::Layout(error)
    }
}

impl From<StorageError> for CoreError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<PrivacyError> for CoreError {
    fn from(error: PrivacyError) -> Self {
        Self::Privacy(error)
    }
}
