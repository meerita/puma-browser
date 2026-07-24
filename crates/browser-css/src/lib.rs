//! @file crates/browser-css/src/lib.rs
//! @description CSS crate root: reduced text style, the cascade, style properties, and errors.
//! @layer css
//! @created meerita <meerita@icloud.com>

mod cascade;
mod computed_style;
mod declaration;
mod error;
mod style_properties;
mod text_style;

pub use cascade::{cascade, computed_style};
pub use computed_style::computed_run_style;
pub use error::CssError;
pub use style_properties::{
    Color, DisplayMode, Emphasis, ListMarker, ReadingOrder, TextTransform, WhiteSpace,
};
pub use text_style::TextStyle;
