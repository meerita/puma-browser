// @file crates/browser-html/tests/semantic_node.rs
// @description Construction smoke test covering every SemanticNode variant.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{InlineRun, SemanticNode};

#[test]
fn all_semantic_node_variants_construct() {
    let nodes = vec![
        SemanticNode::Heading {
            level: 1,
            runs: vec![InlineRun::plain("Title".to_string())],
        },
        SemanticNode::Paragraph {
            runs: vec![InlineRun::plain("Body".to_string())],
        },
        SemanticNode::List {
            ordered: false,
            children: vec![SemanticNode::ListItem {
                children: vec![SemanticNode::Paragraph {
                    runs: vec![InlineRun::plain("First".to_string())],
                }],
            }],
        },
        SemanticNode::ListItem {
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("First".to_string())],
            }],
        },
        SemanticNode::Table,
        SemanticNode::TableRow,
        SemanticNode::TableCell,
        SemanticNode::Quote {
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("Quoted".to_string())],
            }],
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

    assert_eq!(nodes.len(), 21);
}
