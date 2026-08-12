// @file crates/browser-mcp/src/error.rs
// @description MCP adapter error taxonomy; exposes only stable reason codes to clients.
// @layer mcp
// @created meerita <meerita@icloud.com>

use browser_core::CoreError;
use thiserror::Error;

/// Only the stable reason code from [`McpError::reason_code`] ever crosses to an MCP
/// client; wrapped sources, messages, and every internal detail stay in `tracing` at
/// `debug`/`trace` level so cookie values, tokens, and other secrets never escape.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("core error")]
    Core(#[source] CoreError),

    #[error("permission denied")]
    PermissionDenied,

    #[error("document not loaded")]
    DocumentNotLoaded,

    #[error("SSRF blocked: URL targets a private or loopback address")]
    SsrfBlocked,

    #[error("protocol error")]
    Protocol(#[source] anyhow::Error),
}

impl From<CoreError> for McpError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

impl McpError {
    pub fn reason_code(&self) -> &'static str {
        match self {
            Self::Core(error) => Self::core_reason_code(error),
            Self::PermissionDenied => "PERMISSION_DENIED",
            Self::DocumentNotLoaded => "DOCUMENT_NOT_LOADED",
            Self::SsrfBlocked => "SSRF_BLOCKED",
            Self::Protocol(_) => "PROTOCOL_ERROR",
        }
    }

    fn core_reason_code(error: &CoreError) -> &'static str {
        match error {
            CoreError::NavigationFailed => "NAVIGATION_FAILED",
            CoreError::TabNotFound => "TAB_NOT_FOUND",
            CoreError::Network(_) => "NETWORK_ERROR",
            // Local-file failures cannot arise through MCP: file access is disabled and
            // no MCP tool resolves a local path. They collapse to the generic acquisition
            // code so no client ever learns that local files exist as a resource.
            CoreError::LocalFileNotFound
            | CoreError::LocalPathIsDirectory
            | CoreError::LocalFileTooLarge
            | CoreError::LocalFileReadFailed => "NETWORK_ERROR",
            CoreError::Parse(_) => "DOCUMENT_NOT_LOADED",
            // A layout failure is an internal rendering fault, reported generically so
            // the client never learns which stage of display broke.
            CoreError::Layout(_) => "NAVIGATION_FAILED",
            // A storage failure surfaces as a generic operation failure; the client
            // never learns that local persistence was involved.
            CoreError::Storage(_) => "NAVIGATION_FAILED",
            // A privacy-policy rejection is reported to the client as a denial.
            CoreError::Privacy(_) => "PERMISSION_DENIED",
            CoreError::FieldNotFound => "FIELD_NOT_FOUND",
        }
    }
}
