// @file crates/browser-privacy/tests/parsed_cookie.rs
// @description Verifies Set-Cookie parsing maps every attribute and flags malformed lines.
// @layer privacy
// @created meerita <meerita@icloud.com>

use browser_privacy::{parse, PrivacyError, SameSite};

#[test]
fn a_full_set_cookie_line_maps_every_field() {
    let cookie = parse(
        "sid=abc123; Expires=Wed, 09 Jun 2027 10:18:14 GMT; Path=/account; \
         Secure; HttpOnly; SameSite=Lax",
    )
    .expect("a well-formed Set-Cookie line must parse");

    assert_eq!(cookie.name(), "sid");
    assert_eq!(cookie.value().reveal(), "abc123");
    assert_eq!(cookie.path(), Some("/account"));
    assert!(cookie.has_expiry());
    assert_eq!(cookie.same_site(), SameSite::Lax);
    assert!(cookie.secure());
    assert!(cookie.http_only());
}

#[test]
fn a_cookie_without_expires_or_max_age_is_a_session_cookie() {
    let cookie = parse("sid=abc123").expect("a bare name=value cookie must parse");

    assert!(!cookie.has_expiry());
    assert_eq!(cookie.same_site(), SameSite::Unset);
    assert!(!cookie.secure());
    assert!(!cookie.http_only());
}

#[test]
fn a_max_age_cookie_reports_an_expiry() {
    let cookie = parse("sid=abc123; Max-Age=3600").expect("a Max-Age cookie must parse");

    assert!(cookie.has_expiry());
}

#[test]
fn a_line_without_a_name_value_pair_is_malformed() {
    let outcome = parse("no-pair-here");

    assert!(matches!(outcome, Err(PrivacyError::CookieMalformed)));
}
