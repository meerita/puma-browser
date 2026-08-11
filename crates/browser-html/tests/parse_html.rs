// @file crates/browser-html/tests/parse_html.rs
// @description Behavior tests for parse_html: tree mapping, sanitization, scripts, and limits.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, HtmlError, InlineEmphasis, InlineRun, SemanticNode};

fn parse(source: &str) -> Vec<SemanticNode> {
    parse_html(source.as_bytes(), None)
        .expect("well-formed HTML must parse")
        .children()
        .to_vec()
}

/// The text of a text block that holds exactly one plain run, for terse assertions.
fn single_run_text(node: &SemanticNode) -> &str {
    match node {
        SemanticNode::Heading { runs, .. } | SemanticNode::Paragraph { runs, .. } => {
            assert_eq!(runs.len(), 1, "expected exactly one run");
            &runs[0].text
        }
        _ => panic!("expected a text block with runs"),
    }
}

#[test]
fn headings_map_to_heading_nodes_with_their_level_and_text() {
    let nodes =
        parse("<h1>One</h1><h2>Two</h2><h3>Three</h3><h4>Four</h4><h5>Five</h5><h6>Six</h6>");

    let headings: Vec<(u8, &str)> = nodes
        .iter()
        .filter_map(|node| match node {
            SemanticNode::Heading { level, runs, .. } => Some((*level, runs[0].text.as_str())),
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
            runs: vec![InlineRun::plain("Clean[31mText".to_string())],
            inline_style: None,
        }]
    );
}

#[test]
fn paragraph_with_inline_anchor_splits_the_link_into_its_own_run() {
    let nodes = parse(r#"<p>See <a href="/docs">the docs</a> for details</p>"#);

    assert_eq!(
        nodes,
        vec![SemanticNode::Paragraph {
            runs: vec![
                InlineRun::plain("See ".to_string()),
                InlineRun {
                    text: "the docs".to_string(),
                    emphasis: InlineEmphasis::none(),
                    link: Some("/docs".to_string()),
                    citation: None,
                    anchors: Vec::new(),
                },
                InlineRun::plain(" for details".to_string()),
            ],
            inline_style: None,
        }]
    );
}

#[test]
fn standalone_anchor_produces_no_block_node() {
    let nodes = parse(r#"<a href="https://example.com/home">Home</a>"#);

    assert!(
        nodes.is_empty(),
        "a standalone anchor must not become a block node"
    );
}

#[test]
fn list_maps_to_a_list_of_items_each_holding_a_paragraph_run() {
    let nodes = parse("<ul><li>Alpha</li><li>Beta</li></ul>");

    let SemanticNode::List {
        ordered, children, ..
    } = &nodes[0]
    else {
        panic!("expected a list node");
    };
    assert!(!ordered, "an unordered list must not be marked ordered");
    assert_eq!(children.len(), 2);

    let items: Vec<&str> = children
        .iter()
        .map(|item| match item {
            SemanticNode::ListItem { children, .. } => single_run_text(&children[0]),
            _ => panic!("expected a list item"),
        })
        .collect();

    assert_eq!(items, vec!["Alpha", "Beta"]);
}

#[test]
fn ordered_list_is_marked_ordered() {
    let nodes = parse("<ol><li>First</li></ol>");

    let SemanticNode::List { ordered, .. } = &nodes[0] else {
        panic!("expected a list node");
    };
    assert!(ordered, "an ordered list must be marked ordered");
}

/// The block children of a list item, panicking when the node is not a list item.
fn list_item_children(item: &SemanticNode) -> &[SemanticNode] {
    match item {
        SemanticNode::ListItem { children, .. } => children,
        _ => panic!("expected a list item"),
    }
}

/// The single list item of a list holding exactly one item, with the ordered flag.
fn sole_item(node: &SemanticNode) -> (bool, &SemanticNode) {
    match node {
        SemanticNode::List {
            ordered, children, ..
        } => {
            assert_eq!(children.len(), 1, "expected exactly one list item");
            (*ordered, &children[0])
        }
        _ => panic!("expected a list node"),
    }
}

#[test]
fn nested_list_becomes_a_list_child_of_its_list_item() {
    let nodes = parse("<ul><li>a<ul><li>b</li></ul></li></ul>");

    let (outer_ordered, outer_item) = sole_item(&nodes[0]);
    assert!(!outer_ordered, "the outer list is unordered");

    let outer_children = list_item_children(outer_item);
    assert_eq!(
        outer_children.len(),
        2,
        "the item keeps its bare text and the nested list"
    );
    assert_eq!(single_run_text(&outer_children[0]), "a");

    let (inner_ordered, inner_item) = sole_item(&outer_children[1]);
    assert!(!inner_ordered, "the nested list is unordered");
    assert_eq!(single_run_text(&list_item_children(inner_item)[0]), "b");
}

#[test]
fn list_item_with_a_paragraph_and_a_nested_list_keeps_source_order() {
    let nodes = parse("<ul><li><p>intro</p><ul><li>nested</li></ul></li></ul>");

    let (_, item) = sole_item(&nodes[0]);
    let children = list_item_children(item);

    assert_eq!(
        children.len(),
        2,
        "the paragraph and the nested list survive"
    );
    assert!(
        matches!(children[0], SemanticNode::Paragraph { .. }),
        "the paragraph comes first"
    );
    assert_eq!(single_run_text(&children[0]), "intro");
    assert!(
        matches!(children[1], SemanticNode::List { .. }),
        "the nested list comes second"
    );
}

#[test]
fn a_nested_ordered_list_keeps_its_own_ordered_flag() {
    let nodes = parse("<ul><li>x<ol><li>y</li></ol></li></ul>");

    let (outer_ordered, outer_item) = sole_item(&nodes[0]);
    assert!(!outer_ordered, "the outer list stays unordered");

    let nested_list = &list_item_children(outer_item)[1];
    let (inner_ordered, _) = sole_item(nested_list);
    assert!(inner_ordered, "the nested ordered list is marked ordered");
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
fn blockquote_maps_to_a_quote_node_with_a_paragraph_child() {
    let nodes = parse("<blockquote>A remembered line</blockquote>");

    assert_eq!(
        nodes,
        vec![SemanticNode::Quote {
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("A remembered line".to_string())],
                inline_style: None,
            }],
            inline_style: None,
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
    let document = parse_html(
        "<head><script>alert('x')</script></head><body><p>Body</p></body>".as_bytes(),
        None,
    )
    .expect("HTML with a script must parse");

    assert_eq!(document.script_count(), 1);

    let has_warning = document
        .children()
        .iter()
        .any(|node| matches!(node, SemanticNode::Warning { .. }));
    assert!(has_warning, "a suppressed script must surface a warning");

    let leaks_script = document.children().iter().any(|node| match node {
        SemanticNode::Paragraph { runs, .. } => runs.iter().any(|run| run.text.contains("alert")),
        SemanticNode::CodeBlock { text } | SemanticNode::PreformattedBlock { text } => {
            text.contains("alert")
        }
        _ => false,
    });
    assert!(!leaks_script, "script content must never reach a node");
}

#[test]
fn paragraph_text_and_title_are_stripped_of_escape_and_control_characters() {
    let document = parse_html(
        "<title>Ti\u{1b}tle</title><body><p>Hello\u{1b}[31m\r\u{0}World</p></body>".as_bytes(),
        None,
    )
    .expect("HTML with control characters must parse");

    assert_eq!(document.title().map(|title| title.as_str()), Some("Title"));

    // The parser normalizes the carriage return to a newline before the sanitizer runs,
    // and block text collapses that newline to a single space. The escape and NUL are
    // gone; what matters is that no control character survives into the node.
    assert_eq!(
        document.children(),
        &[SemanticNode::Paragraph {
            runs: vec![InlineRun::plain("Hello[31m World".to_string())],
            inline_style: None,
        }]
    );

    let SemanticNode::Paragraph { runs, .. } = &document.children()[0] else {
        panic!("expected a single paragraph node");
    };
    assert!(
        !runs[0].text.chars().any(|character| character.is_control()),
        "no control character may survive sanitization"
    );
}

#[test]
fn document_exceeding_the_node_count_limit_returns_that_error() {
    let source = "<hr>".repeat(50_001);

    let error = parse_html(source.as_bytes(), None).expect_err("too many nodes must fail");

    assert!(matches!(error, HtmlError::MaxNodeCountExceeded));
}

#[test]
fn document_exceeding_the_depth_limit_returns_that_error() {
    let source = "<div>".repeat(300);

    let error = parse_html(source.as_bytes(), None).expect_err("too much nesting must fail");

    assert!(matches!(error, HtmlError::MaxDepthExceeded));
}

#[test]
fn pathologically_nested_lists_hit_the_depth_limit() {
    let source = "<ul><li>".repeat(200);

    let error = parse_html(source.as_bytes(), None).expect_err("too much list nesting must fail");

    assert!(matches!(error, HtmlError::MaxDepthExceeded));
}
