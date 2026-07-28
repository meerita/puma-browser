// @file crates/browser-network/src/fetched_document_tests.rs
// @description Tests for FetchedDocument: wire_byte_count and set_cookie_lines accessors.
// @layer network
// @created meerita <meerita@icloud.com>

use super::FetchedDocument;
use crate::browser_url::BrowserUrl;

fn test_url() -> BrowserUrl {
    BrowserUrl::parse("https://example.com/").expect("test URL must parse")
}

#[test]
fn wire_byte_count_returns_constructed_value() {
    let document = FetchedDocument::new(
        test_url(),
        "text/html".to_string(),
        None,
        b"<html></html>".to_vec(),
        1024,
        Vec::new(),
    );
    assert_eq!(document.wire_byte_count(), 1024);
}

#[test]
fn wire_byte_count_zero_signals_absent_content_length() {
    let document = FetchedDocument::new(
        test_url(),
        "text/html".to_string(),
        None,
        b"<html></html>".to_vec(),
        0,
        Vec::new(),
    );
    assert_eq!(document.wire_byte_count(), 0);
}

#[test]
fn set_cookie_lines_returns_the_constructed_lines_in_order() {
    let document = FetchedDocument::new(
        test_url(),
        "text/html".to_string(),
        None,
        b"<html></html>".to_vec(),
        0,
        vec!["a=1".to_string(), "b=2".to_string()],
    );
    assert_eq!(
        document.set_cookie_lines(),
        &["a=1".to_string(), "b=2".to_string()]
    );
}

#[test]
fn set_cookie_lines_is_empty_when_none_were_captured() {
    let document = FetchedDocument::new(
        test_url(),
        "text/html".to_string(),
        None,
        b"<html></html>".to_vec(),
        0,
        Vec::new(),
    );
    assert!(document.set_cookie_lines().is_empty());
}
