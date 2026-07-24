// @file crates/browser-css/tests/computed_style.rs
// @description Verifies computed_style maps each node kind to its expected reduced style.
// @layer css
// @created meerita <meerita@icloud.com>

use browser_css::{computed_style, Emphasis, ListMarker, TextStyle, WhiteSpace};
use browser_html::{InlineRun, SemanticNode};

#[test]
fn heading_is_bold_with_surrounding_spacing() {
    let style = computed_style(&SemanticNode::Heading {
        level: 1,
        runs: vec![InlineRun::plain(String::from("Title"))],
        inline_style: None,
    });

    assert_eq!(style.emphasis, Emphasis::Bold);
    assert_eq!(style.spacing_before, 1);
    assert_eq!(style.spacing_after, 1);
}

#[test]
fn list_item_uses_a_bullet_marker() {
    let style = computed_style(&SemanticNode::ListItem {
        children: vec![SemanticNode::Paragraph {
            runs: vec![InlineRun::plain(String::from("item"))],
            inline_style: None,
        }],
        inline_style: None,
    });

    assert_eq!(style.list_marker, Some(ListMarker::Disc));
}

#[test]
fn plain_paragraph_uses_the_default_style() {
    let style = computed_style(&SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(String::from("body text"))],
        inline_style: None,
    });

    assert_eq!(style, TextStyle::default());
}

#[test]
fn code_block_preserves_whitespace() {
    let style = computed_style(&SemanticNode::CodeBlock {
        text: String::from("let x = 1;"),
    });

    assert_eq!(style.white_space, WhiteSpace::Pre);
}

#[test]
fn quote_has_surrounding_spacing() {
    let style = computed_style(&SemanticNode::Quote {
        children: vec![SemanticNode::Paragraph {
            runs: vec![InlineRun::plain(String::from("quoted"))],
            inline_style: None,
        }],
        inline_style: None,
    });

    assert_eq!(style.spacing_before, 1);
    assert_eq!(style.spacing_after, 1);
}
