// @file crates/browser-html/tests/document_title.rs
// @description Behavior tests for DocumentTitle sanitization and length bounding.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::DocumentTitle;

#[test]
fn document_title_strips_control_characters() {
    let title = DocumentTitle::new("Hello\x1b[31m\r\n\x00World");
    assert_eq!(title.as_str(), "Hello[31mWorld");
}

#[test]
fn document_title_truncates_to_maximum_length() {
    let overlong = "a".repeat(1000);
    let title = DocumentTitle::new(&overlong);
    assert_eq!(title.as_str().chars().count(), 256);
}

#[test]
fn document_title_preserves_plain_text() {
    let title = DocumentTitle::new("Example Domain");
    assert_eq!(title.as_str(), "Example Domain");
}
