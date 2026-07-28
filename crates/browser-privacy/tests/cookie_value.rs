// @file crates/browser-privacy/tests/cookie_value.rs
// @description Verifies CookieValue redacts its value in Debug and exposes it only via reveal.
// @layer privacy
// @created meerita <meerita@icloud.com>

use browser_privacy::CookieValue;

#[test]
fn debug_redacts_the_value() {
    let value = CookieValue::new("super-secret-token");
    let rendered = format!("{value:?}");

    assert!(rendered.contains("REDACTED"));
    assert!(!rendered.contains("super-secret-token"));
}

#[test]
fn reveal_returns_the_underlying_value() {
    let value = CookieValue::new("super-secret-token");

    assert_eq!(value.reveal(), "super-secret-token");
}
