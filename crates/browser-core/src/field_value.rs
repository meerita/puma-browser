// @file crates/browser-core/src/field_value.rs
// @description Secret-safe wrapper for a form field's typed value with a redacted Debug.
// @layer core
// @created meerita <meerita@icloud.com>

use std::fmt;

/// The live value of a sensitive form field (a `type="password"` input), held so it can
/// never be printed by accident.
///
/// The inner string is private and there is no `Display`. `Debug` prints a fixed
/// placeholder so the value cannot leak through a log line, an error, or a struct that
/// derives `Debug`. Only form submission reads the value, through `reveal`.
pub struct FieldValue(String);

impl FieldValue {
    /// Wraps a raw field value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the underlying value for the one caller that must send it back to the
    /// origin: building an outgoing form submission body.
    pub fn reveal(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for FieldValue {
    /// Never prints the value. A sensitive field value must not reach logs, error
    /// messages, or MCP responses through a derived `Debug`.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("FieldValue(REDACTED)")
    }
}
