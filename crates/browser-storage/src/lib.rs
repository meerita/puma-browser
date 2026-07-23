//! @file crates/browser-storage/src/lib.rs
//! @description Storage crate root: the SQLite persistence error taxonomy and capability traits.
//! @layer storage
//! @created meerita <meerita@icloud.com>

mod error;
mod stores;

pub use error::StorageError;
pub use stores::{BookmarkStore, ConfigStore, HistoryStore};
