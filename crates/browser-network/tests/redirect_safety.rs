// @file crates/browser-network/tests/redirect_safety.rs
// @description Behavior tests for resolve_redirect: scheme allowlist, downgrade guard, target resolution.
// @layer network
// @created meerita <meerita@icloud.com>

use browser_network::{resolve_redirect, BrowserUrl, NetworkError};

fn parsed(input: &str) -> BrowserUrl {
    BrowserUrl::parse(input).expect("test URL must parse")
}

#[test]
fn relative_location_resolves_against_the_current_url() {
    let current = parsed("https://example.com/dir/page");
    let next = resolve_redirect(&current, "/other").expect("relative location must resolve");
    assert_eq!(next.as_str(), "https://example.com/other");
}

#[test]
fn absolute_location_resolves_to_the_given_url() {
    let current = parsed("https://example.com/start");
    let next = resolve_redirect(&current, "https://other.example.com/end")
        .expect("absolute location must resolve");
    assert_eq!(next.as_str(), "https://other.example.com/end");
}

#[test]
fn https_to_http_downgrade_is_rejected() {
    let current = parsed("https://example.com/");
    let outcome = resolve_redirect(&current, "http://example.com/");
    assert!(matches!(outcome, Err(NetworkError::RequestFailed)));
}

#[test]
fn http_to_https_upgrade_is_allowed() {
    let current = parsed("http://example.com/");
    let next = resolve_redirect(&current, "https://example.com/").expect("an upgrade must resolve");
    assert_eq!(next.scheme(), "https");
}

#[test]
fn redirect_to_a_non_http_scheme_is_rejected() {
    let current = parsed("https://example.com/");
    let outcome = resolve_redirect(&current, "ftp://example.com/file");
    assert!(matches!(outcome, Err(NetworkError::RequestFailed)));
}

#[test]
fn redirect_to_a_file_scheme_is_rejected() {
    let current = parsed("https://example.com/");
    let outcome = resolve_redirect(&current, "file:///etc/passwd");
    assert!(matches!(outcome, Err(NetworkError::RequestFailed)));
}
