// @file crates/browser-privacy/src/cookie_policy.rs
// @description Cookie handling policy enum for the privacy layer.
// @layer privacy
// @created meerita <meerita@icloud.com>

/// How the browser treats cookies offered by a response.
///
/// This is a domain enum and is deliberately not `serde`-derived; adapters own the
/// wire and storage representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookiePolicy {
    Allow,
    Session,
    Ask,
    Reject,
}

impl Default for CookiePolicy {
    /// Cookies are rejected by default; the user must opt in before any cookie is
    /// accepted. This default is a privacy invariant, not a convenience.
    fn default() -> Self {
        CookiePolicy::Reject
    }
}
