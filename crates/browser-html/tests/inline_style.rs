// @file crates/browser-html/tests/inline_style.rs
// @description Verifies the parser captures the raw, control-stripped style attribute on nodes.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, SemanticNode};

/// The inline style captured on the first paragraph of a parsed document.
fn first_paragraph_style(source: &str) -> Option<String> {
    let document = parse_html(source.as_bytes(), None).expect("valid HTML must parse");
    for node in document.children() {
        if let SemanticNode::Paragraph { inline_style, .. } = node {
            return inline_style.clone();
        }
    }
    panic!("expected a paragraph node");
}

#[test]
fn paragraph_style_attribute_is_captured() {
    let style = first_paragraph_style(r#"<p style="color: red">hello</p>"#);

    assert_eq!(style.as_deref(), Some("color: red"));
}

#[test]
fn paragraph_without_style_attribute_stores_none() {
    let style = first_paragraph_style("<p>hello</p>");

    assert_eq!(style, None);
}

#[test]
fn control_characters_are_stripped_from_the_style_value() {
    let style = first_paragraph_style("<p style=\"color:\x1b[31m red\">hello</p>");

    assert_eq!(style.as_deref(), Some("color:[31m red"));
    let captured = style.expect("style must be captured");
    assert!(!captured.contains('\x1b'));
}

#[test]
fn heading_style_attribute_is_captured() {
    let document = parse_html(r#"<h1 style="display: none">Title</h1>"#.as_bytes(), None)
        .expect("valid HTML must parse");
    let heading = document
        .children()
        .iter()
        .find_map(|node| match node {
            SemanticNode::Heading { inline_style, .. } => Some(inline_style.clone()),
            _ => None,
        })
        .expect("expected a heading node");

    assert_eq!(heading.as_deref(), Some("display: none"));
}

#[test]
fn blockquote_style_attribute_is_captured() {
    let document = parse_html(
        r#"<blockquote style="color: blue"><p>quoted</p></blockquote>"#.as_bytes(),
        None,
    )
    .expect("valid HTML must parse");
    let quote = document
        .children()
        .iter()
        .find_map(|node| match node {
            SemanticNode::Quote { inline_style, .. } => Some(inline_style.clone()),
            _ => None,
        })
        .expect("expected a quote node");

    assert_eq!(quote.as_deref(), Some("color: blue"));
}
