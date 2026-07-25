// @file crates/browser-network/src/fetched_document_tests.rs
// @description Tests for FetchedDocument: wire_byte_count accessor.
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
    );
    assert_eq!(document.wire_byte_count(), 0);
}
