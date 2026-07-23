// @file crates/browser-html/src/error.rs
// @description HTML-layer error taxonomy for parse-time input and resource-limit failures.
// @layer html
// @created meerita <meerita@icloud.com>

use thiserror::Error;

/// Errors produced while turning HTML into the semantic document tree.
///
/// The resource-limit variants exist so untrusted remote content cannot exhaust memory
/// during parsing; they are enforced at the point input is received.
#[derive(Debug, Error)]
pub enum HtmlError {
    #[error("empty input")]
    EmptyInput,

    #[error("document too large")]
    TooLarge,

    #[error("maximum nesting depth exceeded")]
    MaxDepthExceeded,

    #[error("maximum node count exceeded")]
    MaxNodeCountExceeded,
}
