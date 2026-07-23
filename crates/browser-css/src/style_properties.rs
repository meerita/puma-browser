// @file crates/browser-css/src/style_properties.rs
// @description Enumerated text-style properties consumed by the layout engine.
// @layer css
// @created meerita <meerita@icloud.com>

/// How a node participates in the text layout flow.
///
/// Domain enum, deliberately not `serde`-derived; adapters own any wire or storage
/// representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Block,
    Inline,
    ListItem,
    Hidden,
}

/// Character-weight emphasis applied to a node's text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emphasis {
    None,
    Bold,
    Italic,
}

/// How whitespace in the source text is collapsed and wrapped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    Pre,
    NoWrap,
}

/// The order in which sibling content is read out during layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadingOrder {
    Source,
    Reversed,
}

/// The marker rendered before each item of a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMarker {
    Disc,
    Decimal,
    None,
}

/// A terminal-oriented color, limited to the standard ANSI palette for v0.1.
///
/// This is enough to carry foreground and background intent from the reduced style to
/// the layout engine; richer color models can be added when the cascade lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}
