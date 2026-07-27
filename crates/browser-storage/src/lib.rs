//! @file crates/browser-storage/src/lib.rs
//! @description Storage crate root: the SQLite persistence error taxonomy and capability traits.
//! @layer storage
//! @created meerita <meerita@icloud.com>

mod database;
mod error;
mod migrations;
mod paths;
mod stores;

pub use database::SqliteStorage;
pub use error::StorageError;
pub use paths::default_database_path;
pub use stores::{BookmarkStore, ConfigStore, HistoryStore};
