// @file crates/browser-privacy/src/error.rs
// @description Privacy-layer error taxonomy; pure domain errors with no network or terminal types.
// @layer privacy
// @created meerita <meerita@icloud.com>

use thiserror::Error;

/// Errors produced by the privacy layer.
///
/// These are pure domain errors. They carry no `reqwest`, `ratatui`, or other
/// outer-crate types, and never include cookie values or other secrets.
#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("cookie rejected by policy")]
    CookieRejected,

    #[error("request blocked by policy")]
    RequestBlocked,
}
