// @file crates/browser-css/src/error.rs
// @description CSS-layer error taxonomy for the reduced style stage.
// @layer css
// @created meerita <meerita@icloud.com>

use thiserror::Error;

/// Errors produced by the CSS stage.
///
/// The cascade lands in a later milestone; this taxonomy stays small until then and
/// never exposes parser-internal detail to callers.
#[derive(Debug, Error)]
pub enum CssError {
    #[error("failed to parse CSS")]
    ParseFailed,

    #[error("invalid value for property: {property}")]
    InvalidValue { property: String },
}
