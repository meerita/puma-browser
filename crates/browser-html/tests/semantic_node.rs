// @file crates/browser-html/tests/semantic_node.rs
// @description Construction smoke test covering every SemanticNode variant.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{InlineRun, InputKind, LandmarkRole, SemanticNode};

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
        SemanticNode::Table {
            children: vec![SemanticNode::TableRow {
                children: vec![SemanticNode::TableCell {
                    header: true,
                    children: vec![SemanticNode::Paragraph {
                        runs: vec![InlineRun::plain("Name".to_string())],
                    }],
                }],
            }],
        },
        SemanticNode::TableRow {
            children: vec![SemanticNode::TableCell {
                header: false,
                children: vec![SemanticNode::Paragraph {
                    runs: vec![InlineRun::plain("Alice".to_string())],
                }],
            }],
        },
        SemanticNode::TableCell {
            header: false,
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("Madrid".to_string())],
            }],
        },
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
        SemanticNode::Figure {
            children: vec![SemanticNode::ImagePlaceholder {
                alt: "Chart".to_string(),
                title: None,
                source: None,
            }],
            caption: Some(vec![InlineRun::plain("Figure 1".to_string())]),
        },
        SemanticNode::Form {
            children: vec![SemanticNode::Button {
                runs: vec![InlineRun::plain("Send".to_string())],
            }],
        },
        SemanticNode::Input {
            kind: InputKind::Password,
            label: Some("Password".to_string()),
            sensitive: true,
        },
        SemanticNode::Select {
            label: Some("Country".to_string()),
            options: vec!["Spain".to_string(), "United Kingdom".to_string()],
        },
        SemanticNode::Button {
            runs: vec![InlineRun::plain("Submit".to_string())],
        },
        SemanticNode::Separator,
        SemanticNode::Landmark {
            role: LandmarkRole::Navigation,
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("Menu".to_string())],
            }],
        },
        SemanticNode::Details {
            open: false,
            children: vec![SemanticNode::Summary {
                runs: vec![InlineRun::plain("More".to_string())],
            }],
        },
        SemanticNode::Summary {
            runs: vec![InlineRun::plain("Details".to_string())],
        },
        SemanticNode::EmbeddedContent {
            label: "video".to_string(),
        },
        SemanticNode::Warning {
            message: "script element ignored".to_string(),
        },
    ];

    assert_eq!(nodes.len(), 22);
}
