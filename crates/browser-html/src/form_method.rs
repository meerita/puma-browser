// @file crates/browser-html/src/form_method.rs
// @description The HTTP method a parsed <form> element submits with.
// @layer html
// @created meerita <meerita@icloud.com>

/// The HTTP method a `<form>` submits with.
///
/// An absent or unrecognized `method` attribute defaults to `Get`, matching HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormMethod {
    Get,
    Post,
}
