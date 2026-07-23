//! @file crates/browser-css/src/lib.rs
//! @description CSS crate root: reduced text style, style properties, and the CSS error taxonomy.
//! @layer css
//! @created meerita <meerita@icloud.com>

mod error;
mod style_properties;
mod text_style;

pub use error::CssError;
pub use style_properties::{Color, DisplayMode, Emphasis, ListMarker, ReadingOrder, WhiteSpace};
pub use text_style::TextStyle;
