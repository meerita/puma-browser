// @file crates/browser-network/src/fetch_tests.rs
// @description Unit tests for private redirect resolution: scheme guard and downgrade guard.
// @layer network
// @created meerita <meerita@icloud.com>

use super::{redirect_is_downgrade, resolve_redirect, scheme_is_http};
use crate::error::NetworkError;
use url::Url;

fn parsed(input: &str) -> Url {
    Url::parse(input).expect("test URL must parse")
}

#[test]
fn relative_location_resolves_against_current_url() {
    let current = parsed("https://example.com/dir/page");
    let next = resolve_redirect(&current, "/other").expect("relative location must resolve");
    assert_eq!(next.as_str(), "https://example.com/other");
}

#[test]
fn https_to_http_redirect_is_rejected() {
    let current = parsed("https://example.com/");
    let outcome = resolve_redirect(&current, "http://example.com/");
    assert!(matches!(outcome, Err(NetworkError::RequestFailed)));
}

#[test]
fn http_to_https_redirect_is_allowed() {
    let current = parsed("http://example.com/");
    let next = resolve_redirect(&current, "https://example.com/").expect("upgrade must resolve");
    assert_eq!(next.scheme(), "https");
}

#[test]
fn redirect_to_non_http_scheme_is_rejected() {
    let current = parsed("https://example.com/");
    let outcome = resolve_redirect(&current, "ftp://example.com/file");
    assert!(matches!(outcome, Err(NetworkError::RequestFailed)));
}

#[test]
fn redirect_to_file_scheme_is_rejected() {
    let current = parsed("https://example.com/");
    let outcome = resolve_redirect(&current, "file:///etc/passwd");
    assert!(matches!(outcome, Err(NetworkError::RequestFailed)));
}

#[test]
fn downgrade_predicate_only_matches_https_to_http() {
    assert!(redirect_is_downgrade(
        &parsed("https://a/"),
        &parsed("http://a/")
    ));
    assert!(!redirect_is_downgrade(
        &parsed("http://a/"),
        &parsed("https://a/")
    ));
    assert!(!redirect_is_downgrade(
        &parsed("https://a/"),
        &parsed("https://a/")
    ));
}

#[test]
fn scheme_guard_accepts_only_http_and_https() {
    assert!(scheme_is_http("http"));
    assert!(scheme_is_http("https"));
    assert!(!scheme_is_http("file"));
    assert!(!scheme_is_http("ftp"));
}
