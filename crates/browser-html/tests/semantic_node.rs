// @file crates/browser-html/tests/semantic_node.rs
// @description Construction smoke test covering every SemanticNode variant.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{
    ButtonElement, ButtonKind, FormElement, FormMethod, InlineRun, InputElement, InputKind,
    LandmarkRole, NodeId, SelectElement, SelectOption, SemanticNode, TextareaElement,
};

#[test]
fn all_semantic_node_variants_construct() {
    let nodes = vec![
        SemanticNode::Heading {
            level: 1,
            runs: vec![InlineRun::plain("Title".to_string())],
            inline_style: None,
        },
        SemanticNode::Paragraph {
            runs: vec![InlineRun::plain("Body".to_string())],
            inline_style: None,
        },
        SemanticNode::List {
            ordered: false,
            children: vec![SemanticNode::ListItem {
                children: vec![SemanticNode::Paragraph {
                    runs: vec![InlineRun::plain("First".to_string())],
                    inline_style: None,
                }],
                inline_style: None,
            }],
            inline_style: None,
        },
        SemanticNode::ListItem {
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("First".to_string())],
                inline_style: None,
            }],
            inline_style: None,
        },
        SemanticNode::Table {
            children: vec![SemanticNode::TableRow {
                children: vec![SemanticNode::TableCell {
                    header: true,
                    children: vec![SemanticNode::Paragraph {
                        runs: vec![InlineRun::plain("Name".to_string())],
                        inline_style: None,
                    }],
                    inline_style: None,
                }],
            }],
        },
        SemanticNode::TableRow {
            children: vec![SemanticNode::TableCell {
                header: false,
                children: vec![SemanticNode::Paragraph {
                    runs: vec![InlineRun::plain("Alice".to_string())],
                    inline_style: None,
                }],
                inline_style: None,
            }],
        },
        SemanticNode::TableCell {
            header: false,
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("Madrid".to_string())],
                inline_style: None,
            }],
            inline_style: None,
        },
        SemanticNode::Quote {
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("Quoted".to_string())],
                inline_style: None,
            }],
            inline_style: None,
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
        SemanticNode::Form(FormElement {
            id: NodeId::new(0),
            action: "https://example.com/".to_string(),
            method: FormMethod::Get,
            children: vec![SemanticNode::Button(ButtonElement {
                id: NodeId::new(1),
                kind: ButtonKind::Submit,
                name: None,
                value: None,
                runs: vec![InlineRun::plain("Send".to_string())],
                inline_style: None,
            })],
        }),
        SemanticNode::Input(InputElement {
            id: NodeId::new(2),
            kind: InputKind::Password,
            name: None,
            value: String::new(),
            checked: false,
            label: Some("Password".to_string()),
            sensitive: true,
        }),
        SemanticNode::Select(SelectElement {
            id: NodeId::new(3),
            name: None,
            label: Some("Country".to_string()),
            multiple: false,
            options: vec![
                SelectOption {
                    value: "es".to_string(),
                    label: "Spain".to_string(),
                    selected: true,
                },
                SelectOption {
                    value: "uk".to_string(),
                    label: "United Kingdom".to_string(),
                    selected: false,
                },
            ],
        }),
        SemanticNode::Textarea(TextareaElement {
            id: NodeId::new(4),
            name: None,
            value: "Bio text".to_string(),
            label: Some("Bio".to_string()),
        }),
        SemanticNode::Button(ButtonElement {
            id: NodeId::new(5),
            kind: ButtonKind::Button,
            name: None,
            value: None,
            runs: vec![InlineRun::plain("Submit".to_string())],
            inline_style: None,
        }),
        SemanticNode::Separator,
        SemanticNode::Landmark {
            role: LandmarkRole::Navigation,
            children: vec![SemanticNode::Paragraph {
                runs: vec![InlineRun::plain("Menu".to_string())],
                inline_style: None,
            }],
        },
        SemanticNode::Details {
            open: false,
            children: vec![SemanticNode::Summary {
                runs: vec![InlineRun::plain("More".to_string())],
                inline_style: None,
            }],
        },
        SemanticNode::Summary {
            runs: vec![InlineRun::plain("Details".to_string())],
            inline_style: None,
        },
        SemanticNode::EmbeddedContent {
            label: "video".to_string(),
        },
        SemanticNode::Warning {
            message: "script element ignored".to_string(),
        },
    ];

    assert_eq!(nodes.len(), 23);
}
