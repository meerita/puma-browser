// @file crates/browser-layout/src/error.rs
// @description Layout-layer error taxonomy for the terminal cell-buffer stage.
// @layer layout
// @created meerita <meerita@icloud.com>

use thiserror::Error;

/// Errors produced by the layout stage.
///
/// The layout algorithm lands in a later milestone; this taxonomy stays small until
/// then and never exposes rendering internals to callers.
#[derive(Debug, Error)]
pub enum LayoutError {
    #[error("content width must be greater than zero")]
    ZeroWidth,

    #[error("requested buffer dimensions overflow the addressable cell range")]
    DimensionOverflow,
}
