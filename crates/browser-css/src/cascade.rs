// @file crates/browser-css/src/cascade.rs
// @description Reduced CSS cascade: user-agent defaults, inheritance, and inline overrides.
// @layer css
// @created meerita <meerita@icloud.com>

use browser_html::SemanticNode;

use crate::declaration::{parse_inline_style, Declarations};
use crate::style_properties::{DisplayMode, Emphasis, ListMarker, ReadingOrder, WhiteSpace};
use crate::text_style::TextStyle;

/// Compute the reduced text style for one node given its inherited context.
///
/// This is the tree-aware entry point the layout engine calls at each node as it walks
/// the document: `inherited` is the computed style of the node's parent, from which
/// inherited properties (color, emphasis, white-space, text transform, visibility) flow
/// down. Three layers combine in increasing precedence: the inherited style, the
/// user-agent defaults for the node's kind, and the node's own inline `style` attribute.
/// Non-inherited properties (spacing, display, background, decoration, list marker) reset
/// to the user-agent default at each node rather than inheriting.
pub fn cascade(inherited: &TextStyle, node: &SemanticNode) -> TextStyle {
    let user_agent = user_agent_declarations(node);
    let inline = node_inline_style(node)
        .map(parse_inline_style)
        .unwrap_or_default();
    resolve(inherited, &user_agent, &inline)
}

/// Compute the user-agent text style for a node with no inheritance and no inline style.
///
/// This is the user-agent layer on its own: a fixed mapping from node kind to style,
/// reading no inherited context and no `style` attribute. It gives callers that only want
/// the default presentation of a node kind a stable answer.
pub fn computed_style(node: &SemanticNode) -> TextStyle {
    resolve(
        &TextStyle::default(),
        &user_agent_declarations(node),
        &Declarations::default(),
    )
}

/// Fold the three cascade layers into one computed style, property by property.
///
/// Each property takes the first layer that sets it, inline first, then user-agent.
/// Inherited properties fall back to the parent's computed value; non-inherited
/// properties fall back to their initial value.
fn resolve(inherited: &TextStyle, user_agent: &Declarations, inline: &Declarations) -> TextStyle {
    TextStyle {
        visible: inline
            .visible
            .or(user_agent.visible)
            .unwrap_or(inherited.visible),
        white_space: inline
            .white_space
            .or(user_agent.white_space)
            .unwrap_or(inherited.white_space),
        emphasis: inline
            .emphasis
            .or(user_agent.emphasis)
            .unwrap_or(inherited.emphasis),
        text_transform: inline
            .text_transform
            .or(user_agent.text_transform)
            .unwrap_or(inherited.text_transform),
        foreground: inline
            .foreground
            .or(user_agent.foreground)
            .or(inherited.foreground),
        display_mode: inline
            .display
            .or(user_agent.display)
            .unwrap_or(DisplayMode::Block),
        spacing_before: inline
            .spacing_before
            .or(user_agent.spacing_before)
            .unwrap_or(0),
        spacing_after: inline
            .spacing_after
            .or(user_agent.spacing_after)
            .unwrap_or(0),
        background: inline.background.or(user_agent.background),
        underline: inline.underline.or(user_agent.underline).unwrap_or(false),
        strike: inline.strike.or(user_agent.strike).unwrap_or(false),
        list_marker: inline.list_marker.or(user_agent.list_marker),
        reading_order: inline
            .reading_order
            .or(user_agent.reading_order)
            .unwrap_or(ReadingOrder::Source),
    }
}

/// The user-agent default declarations for a node kind.
///
/// Only the properties the default presentation actually sets are present; everything
/// else stays unset so it inherits or falls back to its initial value. Headings and
/// quotes gain one blank row of spacing on each side, list items gain a bullet marker,
/// code and preformatted blocks keep their source whitespace, and all other block
/// elements gain one blank row after their content. List items are excluded from the
/// trailing blank row so consecutive items in a list run tight.
fn user_agent_declarations(node: &SemanticNode) -> Declarations {
    match node {
        SemanticNode::Heading { .. } => heading_declarations(),
        SemanticNode::ListItem { .. } => list_item_declarations(),
        SemanticNode::CodeBlock { .. } | SemanticNode::PreformattedBlock { .. } => {
            preformatted_declarations()
        }
        SemanticNode::Quote { .. } => quote_declarations(),
        SemanticNode::Paragraph { .. }
        | SemanticNode::List { .. }
        | SemanticNode::Table { .. }
        | SemanticNode::Figure { .. }
        | SemanticNode::Details { .. }
        | SemanticNode::Landmark { .. }
        | SemanticNode::Form { .. } => block_declarations(),
        _ => Declarations::default(),
    }
}

fn heading_declarations() -> Declarations {
    Declarations {
        emphasis: Some(Emphasis::Bold),
        spacing_after: Some(1),
        ..Declarations::default()
    }
}

fn list_item_declarations() -> Declarations {
    Declarations {
        list_marker: Some(ListMarker::Disc),
        ..Declarations::default()
    }
}

fn preformatted_declarations() -> Declarations {
    Declarations {
        white_space: Some(WhiteSpace::Pre),
        spacing_after: Some(1),
        ..Declarations::default()
    }
}

fn quote_declarations() -> Declarations {
    Declarations {
        spacing_after: Some(1),
        ..Declarations::default()
    }
}

fn block_declarations() -> Declarations {
    Declarations {
        spacing_after: Some(1),
        ..Declarations::default()
    }
}

/// The raw inline `style` string a node carries, or `None` for a node kind that has no
/// `style` attribute in the tree.
fn node_inline_style(node: &SemanticNode) -> Option<&str> {
    let inline_style = match node {
        SemanticNode::Heading { inline_style, .. }
        | SemanticNode::Paragraph { inline_style, .. }
        | SemanticNode::List { inline_style, .. }
        | SemanticNode::ListItem { inline_style, .. }
        | SemanticNode::TableCell { inline_style, .. }
        | SemanticNode::Quote { inline_style, .. }
        | SemanticNode::Summary { inline_style, .. }
        | SemanticNode::Button { inline_style, .. } => inline_style,
        _ => return None,
    };
    inline_style.as_deref()
}
