// @file crates/browser-html/tests/html_error.rs
// @description Display-message tests for each HtmlError variant.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::HtmlError;

// No parser produces these variants in v0.1, so each is verified through its safe,
// internals-free display message.

#[test]
fn empty_input_reports_a_readable_message() {
    assert_eq!(HtmlError::EmptyInput.to_string(), "empty input");
}

#[test]
fn too_large_reports_a_readable_message() {
    assert_eq!(HtmlError::TooLarge.to_string(), "document too large");
}

#[test]
fn max_depth_exceeded_reports_a_readable_message() {
    assert_eq!(
        HtmlError::MaxDepthExceeded.to_string(),
        "maximum nesting depth exceeded"
    );
}

#[test]
fn max_node_count_exceeded_reports_a_readable_message() {
    assert_eq!(
        HtmlError::MaxNodeCountExceeded.to_string(),
        "maximum node count exceeded"
    );
}
