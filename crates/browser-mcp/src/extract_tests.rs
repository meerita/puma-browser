// @file crates/browser-mcp/src/extract_tests.rs
// @description Unit tests for extract_text and extract_links.
// @layer mcp
// @created meerita <meerita@icloud.com>

use browser_html::{InlineEmphasis, InlineRun, SemanticNode};

use super::{extract_links, extract_text, LinkKind};

fn plain_run(text: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: None,
        citation: None,
        anchors: Vec::new(),
    }
}

fn linked_run(text: &str, url: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: Some(url.to_string()),
        citation: None,
        anchors: Vec::new(),
    }
}

fn cited_run(text: &str, cite_url: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: None,
        citation: Some(cite_url.to_string()),
        anchors: Vec::new(),
    }
}

fn linked_and_cited_run(text: &str, url: &str, cite_url: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: Some(url.to_string()),
        citation: Some(cite_url.to_string()),
        anchors: Vec::new(),
    }
}

#[test]
fn heading_text_includes_hash_prefix() {
    let node = SemanticNode::Heading {
        level: 2,
        runs: vec![plain_run("Title")],
        inline_style: None,
    };
    let text = extract_text(&[node]);
    assert!(text.starts_with("## Title"), "got: {text:?}");
}

#[test]
fn paragraph_text_joins_runs_with_space() {
    let node = SemanticNode::Paragraph {
        runs: vec![plain_run("Hello"), plain_run("world")],
        inline_style: None,
    };
    let text = extract_text(&[node]);
    assert_eq!(text, "Hello world");
}

#[test]
fn nested_list_text_is_extracted() {
    let inner = SemanticNode::Paragraph {
        runs: vec![plain_run("item")],
        inline_style: None,
    };
    let list_item = SemanticNode::ListItem {
        children: vec![inner],
        inline_style: None,
    };
    let list = SemanticNode::List {
        ordered: false,
        children: vec![list_item],
        inline_style: None,
    };
    let text = extract_text(&[list]);
    assert!(text.contains("item"), "got: {text:?}");
}

#[test]
fn separator_produces_dashes() {
    let text = extract_text(&[SemanticNode::Separator]);
    assert_eq!(text, "---");
}

#[test]
fn empty_image_alt_produces_no_output() {
    let node = SemanticNode::ImagePlaceholder {
        alt: String::new(),
        title: None,
        source: None,
    };
    let text = extract_text(&[node]);
    assert!(text.is_empty(), "got: {text:?}");
}

#[test]
fn links_are_extracted_from_paragraph() {
    let node = SemanticNode::Paragraph {
        runs: vec![linked_run("Click here", "https://example.com")],
        inline_style: None,
    };
    let links = extract_links(&[node]);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].url, "https://example.com");
    assert_eq!(links[0].text, "Click here");
    assert_eq!(links[0].kind, LinkKind::Hyperlink);
}

#[test]
fn runs_without_links_produce_no_entries() {
    let node = SemanticNode::Paragraph {
        runs: vec![plain_run("No links here")],
        inline_style: None,
    };
    let links = extract_links(&[node]);
    assert!(links.is_empty(), "got: {links:?}");
}

#[test]
fn a_citation_run_produces_a_citation_kind_entry_with_the_wire_value_citation() {
    let node = SemanticNode::Paragraph {
        runs: vec![cited_run("Hello", "https://example.com/source")],
        inline_style: None,
    };
    let links = extract_links(&[node]);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].url, "https://example.com/source");
    assert_eq!(links[0].kind, LinkKind::Citation);
    assert_eq!(
        serde_json::to_value(links[0].kind).expect("LinkKind must serialize"),
        serde_json::json!("citation")
    );
}

#[test]
fn a_run_with_both_link_and_citation_produces_two_entries_one_of_each_kind() {
    let node = SemanticNode::Paragraph {
        runs: vec![linked_and_cited_run(
            "Hello",
            "https://example.com/article",
            "https://example.com/source",
        )],
        inline_style: None,
    };
    let links = extract_links(&[node]);
    assert_eq!(links.len(), 2);
    assert!(links.iter().any(
        |entry| entry.kind == LinkKind::Hyperlink && entry.url == "https://example.com/article"
    ));
    assert!(
        links
            .iter()
            .any(|entry| entry.kind == LinkKind::Citation
                && entry.url == "https://example.com/source")
    );
}

#[test]
fn extract_text_includes_the_literal_quote_marks_synthesized_in_run_text() {
    let node = SemanticNode::Paragraph {
        runs: vec![
            plain_run("\u{201C}"),
            plain_run("Hello"),
            plain_run("\u{201D}"),
        ],
        inline_style: None,
    };
    let text = extract_text(&[node]);
    assert!(
        text.contains('\u{201C}') && text.contains('\u{201D}'),
        "got: {text:?}"
    );
}

#[test]
fn an_anchor_target_contributes_no_text() {
    let nodes = vec![
        SemanticNode::AnchorTarget {
            names: vec!["secret-target-name".to_string()],
        },
        SemanticNode::Paragraph {
            runs: vec![plain_run("Body")],
            inline_style: None,
        },
    ];

    assert_eq!(extract_text(&nodes), "Body");
}

#[test]
fn an_anchor_target_adds_no_blank_line_between_blocks() {
    let with_target = vec![
        SemanticNode::Paragraph {
            runs: vec![plain_run("First")],
            inline_style: None,
        },
        SemanticNode::AnchorTarget {
            names: vec!["between".to_string()],
        },
        SemanticNode::Paragraph {
            runs: vec![plain_run("Second")],
            inline_style: None,
        },
    ];
    let without_target = vec![
        SemanticNode::Paragraph {
            runs: vec![plain_run("First")],
            inline_style: None,
        },
        SemanticNode::Paragraph {
            runs: vec![plain_run("Second")],
            inline_style: None,
        },
    ];

    assert_eq!(extract_text(&with_target), extract_text(&without_target));
}

#[test]
fn an_anchor_target_contributes_no_link() {
    let nodes = vec![SemanticNode::AnchorTarget {
        names: vec!["target".to_string()],
    }];

    assert!(extract_links(&nodes).is_empty());
}
