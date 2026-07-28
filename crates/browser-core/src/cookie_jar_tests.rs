// @file crates/browser-core/src/cookie_jar_tests.rs
// @description Unit tests for the session cookie jar: redacted Debug, first-party send, expiry drop.
// @layer core
// @created meerita <meerita@icloud.com>

use std::time::SystemTime;

use browser_network::BrowserUrl;
use browser_privacy::CookieValue;

use super::{CookieJar, StoredCookie};

fn url(input: &str) -> BrowserUrl {
    BrowserUrl::parse(input).expect("test URL must parse")
}

fn session_cookie(name: &str, value: &str) -> StoredCookie {
    StoredCookie::new(
        name.to_string(),
        CookieValue::new(value),
        "/".to_string(),
        false,
        None,
    )
}

#[test]
fn debug_never_prints_a_cookie_value() {
    let mut jar = CookieJar::default();
    jar.store(
        &url("https://example.com/"),
        session_cookie("sid", "super-secret-value"),
    );

    let rendered = format!("{jar:?}");

    assert!(
        !rendered.contains("super-secret-value"),
        "the jar Debug must never contain a cookie value"
    );
    assert!(rendered.contains("cookie_count"));
    assert!(rendered.contains("domain_count"));
}

#[test]
fn first_party_request_receives_the_stored_cookie() {
    let mut jar = CookieJar::default();
    jar.store(&url("https://example.com/"), session_cookie("sid", "abc"));

    let header = jar.cookie_header_for(&url("https://www.example.com/dashboard"));

    assert_eq!(header.as_deref(), Some("sid=abc"));
}

#[test]
fn request_to_a_different_registrable_domain_receives_nothing() {
    let mut jar = CookieJar::default();
    jar.store(&url("https://example.com/"), session_cookie("sid", "abc"));

    let header = jar.cookie_header_for(&url("https://other.org/"));

    assert_eq!(header, None);
}

#[test]
fn a_secure_cookie_is_withheld_from_an_insecure_request() {
    let mut jar = CookieJar::default();
    jar.store(
        &url("https://example.com/"),
        StoredCookie::new(
            "sid".to_string(),
            CookieValue::new("abc"),
            "/".to_string(),
            true,
            None,
        ),
    );

    let header = jar.cookie_header_for(&url("http://example.com/"));

    assert_eq!(header, None);
}

#[test]
fn an_expired_cookie_is_not_sent() {
    let mut jar = CookieJar::default();
    jar.store(
        &url("https://example.com/"),
        StoredCookie::new(
            "sid".to_string(),
            CookieValue::new("abc"),
            "/".to_string(),
            false,
            Some(SystemTime::UNIX_EPOCH),
        ),
    );

    let header = jar.cookie_header_for(&url("https://example.com/"));

    assert_eq!(header, None);
}

#[test]
fn clear_domain_drops_only_that_sites_cookies() {
    let mut jar = CookieJar::default();
    jar.store(&url("https://example.com/"), session_cookie("sid", "abc"));
    jar.store(&url("https://other.org/"), session_cookie("tid", "xyz"));

    jar.clear_domain("example.com");

    assert_eq!(jar.cookie_header_for(&url("https://example.com/")), None);
    assert_eq!(
        jar.cookie_header_for(&url("https://other.org/")).as_deref(),
        Some("tid=xyz")
    );
}

#[test]
fn clear_empties_the_whole_jar() {
    let mut jar = CookieJar::default();
    jar.store(&url("https://example.com/"), session_cookie("sid", "abc"));

    jar.clear();

    assert_eq!(jar.cookie_header_for(&url("https://example.com/")), None);
}
