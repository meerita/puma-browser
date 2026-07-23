// @file crates/browser-html/tests/parse_html.rs
// @description Behavior tests for parse_html: block mapping, sanitization, scripts, and limits.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, HtmlError, SemanticNode};

fn parse(source: &str) -> Vec<SemanticNode> {
    parse_html(source)
        .expect("well-formed HTML must parse")
        .nodes()
        .to_vec()
}

#[test]
fn headings_map_to_heading_nodes_with_their_level_and_text() {
    let nodes =
        parse("<h1>One</h1><h2>Two</h2><h3>Three</h3><h4>Four</h4><h5>Five</h5><h6>Six</h6>");

    let headings: Vec<(u8, &str)> = nodes
        .iter()
        .filter_map(|node| match node {
            SemanticNode::Heading { level, text } => Some((*level, text.as_str())),
            _ => None,
        })
        .collect();

    assert_eq!(
        headings,
        vec![
            (1, "One"),
            (2, "Two"),
            (3, "Three"),
            (4, "Four"),
            (5, "Five"),
            (6, "Six"),
        ]
    );
}

#[test]
fn heading_text_is_stripped_of_control_characters() {
    let nodes = parse("<h1>Clean\u{1b}[31mText</h1>");

    assert_eq!(
        nodes,
        vec![SemanticNode::Heading {
            level: 1,
            text: "Clean[31mText".to_string(),
        }]
    );
}

#[test]
fn paragraph_with_inline_anchor_keeps_the_anchor_text_inside_the_paragraph() {
    let nodes = parse(r#"<p>See <a href="/docs">the docs</a> for details</p>"#);

    assert_eq!(
        nodes,
        vec![SemanticNode::Paragraph {
            text: "See the docs for details".to_string(),
        }]
    );
}

#[test]
fn standalone_anchor_becomes_a_link_with_its_text_and_href() {
    let nodes = parse(r#"<a href="https://example.com/home">Home</a>"#);

    assert_eq!(
        nodes,
        vec![SemanticNode::Link {
            text: "Home".to_string(),
            href: "https://example.com/home".to_string(),
        }]
    );
}

#[test]
fn list_items_become_list_item_nodes_carrying_their_text() {
    let nodes = parse("<ul><li>Alpha</li><li>Beta</li></ul>");

    let items: Vec<&str> = nodes
        .iter()
        .filter_map(|node| match node {
            SemanticNode::ListItem { text } => Some(text.as_str()),
            _ => None,
        })
        .collect();

    assert_eq!(items, vec!["Alpha", "Beta"]);
}

#[test]
fn preformatted_block_preserves_its_internal_newlines() {
    let nodes = parse("<pre>first\nsecond</pre>");

    assert_eq!(
        nodes,
        vec![SemanticNode::PreformattedBlock {
            text: "first\nsecond".to_string(),
        }]
    );
}

#[test]
fn standalone_code_element_maps_to_a_code_block() {
    let nodes = parse("<code>let value = 1;</code>");

    assert_eq!(
        nodes,
        vec![SemanticNode::CodeBlock {
            text: "let value = 1;".to_string(),
        }]
    );
}

#[test]
fn blockquote_maps_to_a_quote_node() {
    let nodes = parse("<blockquote>A remembered line</blockquote>");

    assert_eq!(
        nodes,
        vec![SemanticNode::Quote {
            text: "A remembered line".to_string(),
        }]
    );
}

#[test]
fn horizontal_rule_maps_to_a_separator() {
    let nodes = parse("<hr>");

    assert_eq!(nodes, vec![SemanticNode::Separator]);
}

#[test]
fn image_maps_to_an_image_placeholder_with_alt_title_and_source() {
    let nodes = parse(r#"<img alt="A diagram" title="Overview" src="/diagram.png">"#);

    assert_eq!(
        nodes,
        vec![SemanticNode::ImagePlaceholder {
            alt: "A diagram".to_string(),
            title: Some("Overview".to_string()),
            source: Some("/diagram.png".to_string()),
        }]
    );
}

#[test]
fn script_element_is_never_emitted_is_counted_and_yields_a_warning() {
    let document = parse_html("<head><script>alert('x')</script></head><body><p>Body</p></body>")
        .expect("HTML with a script must parse");

    assert_eq!(document.script_count(), 1);

    let has_warning = document
        .nodes()
        .iter()
        .any(|node| matches!(node, SemanticNode::Warning { .. }));
    assert!(has_warning, "a suppressed script must surface a warning");

    let leaks_script = document.nodes().iter().any(|node| match node {
        SemanticNode::Paragraph { text }
        | SemanticNode::CodeBlock { text }
        | SemanticNode::PreformattedBlock { text } => text.contains("alert"),
        _ => false,
    });
    assert!(!leaks_script, "script content must never reach a node");
}

#[test]
fn paragraph_text_and_title_are_stripped_of_escape_and_control_characters() {
    let document =
        parse_html("<title>Ti\u{1b}tle</title><body><p>Hello\u{1b}[31m\r\u{0}World</p></body>")
            .expect("HTML with control characters must parse");

    assert_eq!(document.title().map(|title| title.as_str()), Some("Title"));

    // The parser normalizes the carriage return to a newline before the sanitizer runs,
    // and block text collapses that newline to a single space. The escape and NUL are
    // gone; what matters is that no control character survives into the node.
    assert_eq!(
        document.nodes(),
        &[SemanticNode::Paragraph {
            text: "Hello[31m World".to_string(),
        }]
    );

    let SemanticNode::Paragraph { text } = &document.nodes()[0] else {
        panic!("expected a single paragraph node");
    };
    assert!(
        !text.chars().any(|character| character.is_control()),
        "no control character may survive sanitization"
    );
}

#[test]
fn document_exceeding_the_node_count_limit_returns_that_error() {
    let source = "<hr>".repeat(50_001);

    let error = parse_html(&source).expect_err("too many nodes must fail");

    assert!(matches!(error, HtmlError::MaxNodeCountExceeded));
}

#[test]
fn document_exceeding_the_depth_limit_returns_that_error() {
    let source = "<div>".repeat(300);

    let error = parse_html(&source).expect_err("too much nesting must fail");

    assert!(matches!(error, HtmlError::MaxDepthExceeded));
}
