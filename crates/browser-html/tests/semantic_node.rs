// @file crates/browser-html/tests/semantic_node.rs
// @description Construction smoke test covering every SemanticNode variant.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::SemanticNode;

#[test]
fn all_semantic_node_variants_construct() {
    let nodes = vec![
        SemanticNode::Document,
        SemanticNode::Heading {
            level: 1,
            text: "Title".to_string(),
        },
        SemanticNode::Paragraph {
            text: "Body".to_string(),
        },
        SemanticNode::Link {
            text: "Home".to_string(),
            href: "https://example.com".to_string(),
        },
        SemanticNode::List,
        SemanticNode::ListItem {
            text: "First".to_string(),
        },
        SemanticNode::Table,
        SemanticNode::TableRow,
        SemanticNode::TableCell,
        SemanticNode::Quote {
            text: "Quoted".to_string(),
        },
        SemanticNode::CodeBlock {
            text: "let x = 1;".to_string(),
        },
        SemanticNode::PreformattedBlock {
            text: "  indented".to_string(),
        },
        SemanticNode::ImagePlaceholder {
            alt: "Diagram".to_string(),
            title: Some("Architecture".to_string()),
            source: Some("https://example.com/diagram.png".to_string()),
        },
        SemanticNode::Form,
        SemanticNode::Input,
        SemanticNode::Select,
        SemanticNode::Button,
        SemanticNode::Separator,
        SemanticNode::Landmark,
        SemanticNode::Details,
        SemanticNode::Summary,
        SemanticNode::EmbeddedContent,
        SemanticNode::Warning {
            message: "script element ignored".to_string(),
        },
    ];

    assert_eq!(nodes.len(), 23);
}
