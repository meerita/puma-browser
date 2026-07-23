// @file crates/browser-storage/src/stores.rs
// @description Capability trait surface for the storage layer; signatures only until v0.4.
// @layer storage
// @created meerita <meerita@icloud.com>

use crate::error::StorageError;

/// Reads and writes persisted configuration values keyed by name.
///
/// Implementations land in a later milestone; this defines the capability surface the
/// storage backend must satisfy. A deliberately narrow trait, not a generic key-value
/// store, so each capability the application needs stays explicit at its call sites.
pub trait ConfigStore {
    /// Returns the stored value for `key`, or `None` when no value is set.
    fn config_value(&self, key: &str) -> Result<Option<String>, StorageError>;

    /// Stores `value` under `key`, replacing any existing value.
    fn set_config_value(&self, key: &str, value: &str) -> Result<(), StorageError>;
}

/// Records browsing history and reads it back in recency order.
///
/// Implementations land in a later milestone. The visited location is passed as text
/// because URL validation belongs to the network layer, not to storage.
pub trait HistoryStore {
    /// Records a visit to `url` with the page `title`.
    fn record_visit(&self, url: &str, title: &str) -> Result<(), StorageError>;

    /// Returns up to `limit` visited URLs, most recent first.
    fn recent_visits(&self, limit: usize) -> Result<Vec<String>, StorageError>;
}

/// Stores and reads back user bookmarks.
///
/// Implementations land in a later milestone. The bookmarked location is passed as
/// text for the same reason as [`HistoryStore`]: URL validation is the network
/// layer's responsibility.
pub trait BookmarkStore {
    /// Adds a bookmark for `url` with the given `title`.
    fn add_bookmark(&self, url: &str, title: &str) -> Result<(), StorageError>;

    /// Returns all bookmarked URLs in insertion order.
    fn bookmarks(&self) -> Result<Vec<String>, StorageError>;
}
