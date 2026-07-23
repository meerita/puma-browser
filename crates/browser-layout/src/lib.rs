//! @file crates/browser-layout/src/lib.rs
//! @description Layout crate root: terminal cell-buffer types and the layout error taxonomy.
//! @layer layout
//! @created meerita <meerita@icloud.com>

mod cell;
mod error;

pub use cell::{Cell, CellBuffer};
pub use error::LayoutError;
