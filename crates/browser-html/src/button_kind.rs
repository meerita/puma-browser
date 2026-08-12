// @file crates/browser-html/src/button_kind.rs
// @description The submission behavior of a button-like control.
// @layer html
// @created meerita <meerita@icloud.com>

/// The submission behavior of a `<button>` or a normalized
/// `<input type=submit|reset|button>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Submit,
    Reset,
    Button,
}
