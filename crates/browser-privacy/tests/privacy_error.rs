// @file crates/browser-privacy/tests/privacy_error.rs
// @description Verifies PrivacyError Display strings are user-facing and leak no secrets.
// @layer privacy
// @created meerita <meerita@icloud.com>

use browser_privacy::PrivacyError;

/// Renders every variant through an exhaustive match, so a new variant added without a
/// test fails to compile here rather than silently escaping the safety check.
fn display_of(error: &PrivacyError) -> String {
    match error {
        PrivacyError::CookieRejected | PrivacyError::RequestBlocked => error.to_string(),
    }
}

fn all_variants() -> [PrivacyError; 2] {
    [PrivacyError::CookieRejected, PrivacyError::RequestBlocked]
}

#[test]
fn every_variant_has_a_non_empty_message() {
    for variant in all_variants() {
        assert!(
            !display_of(&variant).is_empty(),
            "variant must render a user-facing message"
        );
    }
}

#[test]
fn cookie_rejected_reports_a_rejected_cookie() {
    assert_eq!(
        PrivacyError::CookieRejected.to_string(),
        "cookie rejected by policy"
    );
}

#[test]
fn request_blocked_reports_a_blocked_request() {
    assert_eq!(
        PrivacyError::RequestBlocked.to_string(),
        "request blocked by policy"
    );
}
