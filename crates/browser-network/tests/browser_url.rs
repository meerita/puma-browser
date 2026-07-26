// @file crates/browser-network/tests/browser_url.rs
// @description Behavior tests for BrowserUrl parsing, scheme rules, and credential safety.
// @layer network
// @created meerita <meerita@icloud.com>

use browser_network::{BrowserUrl, NetworkError};

#[test]
fn http_url_parses_and_reports_scheme() {
    let url = BrowserUrl::parse("http://example.com/path").expect("http URL must parse");
    assert_eq!(url.scheme(), "http");
    assert_eq!(url.host_str(), Some("example.com"));
}

#[test]
fn https_url_parses_and_reports_scheme() {
    let url = BrowserUrl::parse("https://example.com").expect("https URL must parse");
    assert_eq!(url.scheme(), "https");
}

#[test]
fn file_url_is_accepted() {
    let url = BrowserUrl::parse("file:///etc/hosts").expect("file URL must parse");
    assert_eq!(url.scheme(), "file");
}

#[test]
fn schemeless_host_defaults_to_https() {
    let url = BrowserUrl::parse("minid.net").expect("bare host must parse");
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.host_str(), Some("minid.net"));
}

#[test]
fn schemeless_www_host_with_path_defaults_to_https() {
    let url = BrowserUrl::parse("www.minid.net/login").expect("bare host with path must parse");
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.host_str(), Some("www.minid.net"));
}

#[test]
fn explicit_http_scheme_is_not_upgraded_to_https() {
    let url = BrowserUrl::parse("http://example.com").expect("explicit http URL must parse");
    assert_eq!(url.scheme(), "http");
}

#[test]
fn surrounding_whitespace_is_trimmed_before_parsing() {
    let url = BrowserUrl::parse("  https://example.com  ").expect("padded URL must parse");
    assert_eq!(url.scheme(), "https");
    assert_eq!(url.host_str(), Some("example.com"));
}

#[test]
fn javascript_scheme_is_rejected_as_unsupported() {
    let error =
        BrowserUrl::parse("javascript:alert(1)").expect_err("javascript scheme must be rejected");
    assert!(matches!(error, NetworkError::UnsupportedScheme { .. }));
}

#[test]
fn ftp_scheme_is_rejected_as_unsupported() {
    let error =
        BrowserUrl::parse("ftp://example.com/file").expect_err("ftp scheme must be rejected");
    assert!(matches!(error, NetworkError::UnsupportedScheme { .. }));
}

#[test]
fn malformed_input_returns_invalid_url() {
    let error = BrowserUrl::parse("not a url").expect_err("malformed input must be rejected");
    assert!(matches!(error, NetworkError::InvalidUrl));
}

#[test]
fn fragment_returns_the_part_after_the_hash() {
    let url = BrowserUrl::parse("https://example.com/page#section")
        .expect("URL with a fragment must parse");
    assert_eq!(url.fragment(), Some("section"));
}

#[test]
fn fragment_is_none_when_the_url_carries_no_hash() {
    let url = BrowserUrl::parse("https://example.com/page").expect("URL must parse");
    assert_eq!(url.fragment(), None);
}

#[test]
fn without_fragment_drops_the_fragment_and_keeps_the_base() {
    let url = BrowserUrl::parse("https://example.com/page#section")
        .expect("URL with a fragment must parse");
    let base = url.without_fragment();
    assert_eq!(base.as_str(), "https://example.com/page");
    assert_eq!(base.fragment(), None);
}

#[test]
fn debug_output_omits_url_credentials() {
    let url = BrowserUrl::parse("https://user:secret@example.com/")
        .expect("URL with credentials must parse");
    let debug_output = format!("{url:?}");
    let display_output = format!("{url}");
    assert!(!debug_output.contains("secret"));
    assert!(!debug_output.contains("user"));
    assert!(!display_output.contains("secret"));
    assert!(!display_output.contains("user"));
}
