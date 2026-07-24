// @file crates/browser-css/src/text_style.rs
// @description Reduced computed text style produced per SemanticNode for the layout engine.
// @layer css
// @created meerita <meerita@icloud.com>

use crate::style_properties::{
    Color, DisplayMode, Emphasis, ListMarker, ReadingOrder, TextTransform, WhiteSpace,
};

/// The reduced set of computed style properties the layout engine needs to render a
/// single node as terminal text.
///
/// This is the output type of the CSS stage: the cascade computes one `TextStyle` per
/// node from the user-agent defaults, the inherited style of the parent, and the node's
/// inline declarations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextStyle {
    pub visible: bool,
    pub display_mode: DisplayMode,
    pub spacing_before: u16,
    pub spacing_after: u16,
    pub emphasis: Emphasis,
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub underline: bool,
    pub strike: bool,
    pub list_marker: Option<ListMarker>,
    pub white_space: WhiteSpace,
    pub text_transform: TextTransform,
    pub reading_order: ReadingOrder,
}

impl Default for TextStyle {
    fn default() -> Self {
        TextStyle {
            visible: true,
            display_mode: DisplayMode::Block,
            spacing_before: 0,
            spacing_after: 0,
            emphasis: Emphasis::None,
            foreground: None,
            background: None,
            underline: false,
            strike: false,
            list_marker: None,
            white_space: WhiteSpace::Normal,
            text_transform: TextTransform::None,
            reading_order: ReadingOrder::Source,
        }
    }
}
