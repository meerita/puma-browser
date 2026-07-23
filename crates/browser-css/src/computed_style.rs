// @file crates/browser-css/src/computed_style.rs
// @description Fixed per-node computed TextStyle standing in for the CSS cascade.
// @layer css
// @created meerita <meerita@icloud.com>

use browser_html::SemanticNode;

use crate::style_properties::{Emphasis, ListMarker, WhiteSpace};
use crate::text_style::TextStyle;

/// Compute the reduced text style for a single semantic node.
///
/// This is a fixed mapping from node kind to style, not a cascade: it reads no
/// stylesheet and depends only on the node's kind. It gives the layout stage a stable
/// per-node style until the real cascade lands, so a style change here never depends
/// on external CSS.
pub fn computed_style(node: &SemanticNode) -> TextStyle {
    match node {
        SemanticNode::Heading { .. } => heading_style(),
        SemanticNode::ListItem { .. } => list_item_style(),
        SemanticNode::CodeBlock { .. } | SemanticNode::PreformattedBlock { .. } => {
            preformatted_style()
        }
        SemanticNode::Link { .. } => link_style(),
        SemanticNode::Quote { .. } => quote_style(),
        _ => TextStyle::default(),
    }
}

/// Headings stand out with bold weight and one blank row of breathing room on each side.
fn heading_style() -> TextStyle {
    TextStyle {
        emphasis: Emphasis::Bold,
        spacing_before: 1,
        spacing_after: 1,
        ..TextStyle::default()
    }
}

fn list_item_style() -> TextStyle {
    TextStyle {
        list_marker: Some(ListMarker::Disc),
        ..TextStyle::default()
    }
}

/// Code keeps its source whitespace so it can be laid out verbatim without wrapping.
fn preformatted_style() -> TextStyle {
    TextStyle {
        white_space: WhiteSpace::Pre,
        ..TextStyle::default()
    }
}

fn link_style() -> TextStyle {
    TextStyle {
        underline: true,
        ..TextStyle::default()
    }
}

/// A quote is set apart from surrounding blocks with one blank row above and below.
fn quote_style() -> TextStyle {
    TextStyle {
        spacing_before: 1,
        spacing_after: 1,
        ..TextStyle::default()
    }
}
