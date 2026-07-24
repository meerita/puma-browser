// @file crates/browser-network/src/error.rs
// @description Network-layer error taxonomy; maps failures without leaking driver types.
// @layer network
// @created meerita <meerita@icloud.com>

use thiserror::Error;

/// Errors produced by the network layer.
///
/// Raw `reqwest` and `rustls` errors never appear in any variant; they are mapped to
/// these crate-local variants at the boundary so inner driver details do not cross
/// outward.
#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("invalid URL")]
    InvalidUrl,

    #[error("unsupported URL scheme: {scheme}")]
    UnsupportedScheme { scheme: String },

    #[error("DNS lookup failed")]
    DnsFailure,

    #[error("TLS error")]
    TlsError,

    #[error("request timed out")]
    Timeout,

    #[error("too many redirects")]
    TooManyRedirects,

    #[error("response too large")]
    ResponseTooLarge,

    #[error("request failed")]
    RequestFailed,

    #[error("failed to decode response body")]
    Decode,

    #[error("file not found")]
    FileNotFound,

    #[error("path is a directory")]
    PathIsDirectory,

    #[error("file too large")]
    FileTooLarge,

    #[error("file read failed")]
    FileReadFailed,
}
