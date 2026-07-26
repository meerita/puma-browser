//! @file crates/browser-layout/src/lib.rs
//! @description Layout crate root: terminal cell-buffer types and the layout error taxonomy.
//! @layer layout
//! @created meerita <meerita@icloud.com>

mod cell;
mod error;
mod render;
mod table;
mod width;

pub use cell::{Cell, CellBuffer, CellPosition, LinkSpan};
pub use error::LayoutError;
pub use render::render_document;
pub use width::{AmbiguousWidth, EmojiWidth, WidthConfig};
