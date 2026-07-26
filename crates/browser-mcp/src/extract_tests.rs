// @file crates/browser-mcp/src/extract_tests.rs
// @description Unit tests for extract_text and extract_links.
// @layer mcp
// @created meerita <meerita@icloud.com>

use browser_html::{InlineEmphasis, InlineRun, SemanticNode};

use super::{extract_links, extract_text};

fn plain_run(text: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: None,
        anchors: Vec::new(),
    }
}

fn linked_run(text: &str, url: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: Some(url.to_string()),
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
