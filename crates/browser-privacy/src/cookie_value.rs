// @file crates/browser-privacy/src/cookie_value.rs
// @description Secret-safe wrapper for a cookie value with a redacted Debug.
// @layer privacy
// @created meerita <meerita@icloud.com>

use std::fmt;

/// The value half of a cookie, held so it can never be printed by accident.
///
/// The inner string is private and there is no `Display`. `Debug` prints a fixed
/// placeholder so the value cannot leak through a log line, an error, or a struct that
/// derives `Debug`. Only the session jar reads the value, through `reveal`.
#[derive(Clone)]
pub struct CookieValue(String);

impl CookieValue {
    /// Wraps a raw cookie value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the underlying value for the one caller that must send it back to the
    /// origin: the session jar building an outgoing `Cookie` header.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for CookieValue {
    /// Never prints the value. A cookie value is a secret and must not reach logs,
    /// error messages, or MCP responses through a derived `Debug`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CookieValue(REDACTED)")
    }
}
