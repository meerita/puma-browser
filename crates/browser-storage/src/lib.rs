//! @file crates/browser-storage/src/lib.rs
//! @description Storage crate root: the SQLite persistence error taxonomy and capability traits.
//! @layer storage
//! @created meerita <meerita@icloud.com>

mod config;
mod database;
mod error;
mod migrations;
mod paths;
mod stores;

pub use config::{default_config_path, load_config, BrowserConfig};
pub use database::SqliteStorage;
pub use error::StorageError;
pub use paths::default_database_path;
pub use stores::{BookmarkStore, ConfigStore, HistoryStore};
