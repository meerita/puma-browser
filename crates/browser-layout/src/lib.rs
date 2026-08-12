//! @file crates/browser-layout/src/lib.rs
//! @description Layout crate root: terminal cell-buffer types and the layout error taxonomy.
//! @layer layout
//! @created meerita <meerita@icloud.com>

mod cell;
mod error;
mod field_overlay;
mod field_span;
mod render;
mod table;
mod width;

pub use cell::{AnchorSpan, Cell, CellBuffer, CellPosition, LinkKind, LinkSpan};
pub use error::LayoutError;
pub use field_overlay::{FieldOverlay, FieldRenderValue};
pub use field_span::{FieldSpan, FieldSpanKind};
pub use render::render_document;
pub use width::{AmbiguousWidth, EmojiWidth, WidthConfig};
