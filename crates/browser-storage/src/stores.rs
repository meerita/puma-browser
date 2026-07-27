// @file crates/browser-storage/src/stores.rs
// @description Capability trait surface for the storage layer; signatures only until v0.4.
// @layer storage
// @created meerita <meerita@icloud.com>

use crate::error::StorageError;
use crate::history_records::{HistoryEntry, NewVisit, SuggestionEntry};

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

/// Records browsing history and reads it back for the history list and the suggestion
/// index.
///
/// URL validation and host extraction belong to the caller, so a [`NewVisit`] carries
/// already-resolved fields. The store never reads the clock: `visited_at` and the prune
/// `cutoff` are Unix epoch seconds supplied by the caller. It computes no ranking; it
/// returns the raw aggregates the core crate ranks.
pub trait HistoryStore {
    /// Records `visit`, upserting its page aggregate and appending a visit event, and
    /// returns the page's updated aggregate so the caller can refresh its index in place.
    fn record_visit(&self, visit: NewVisit) -> Result<SuggestionEntry, StorageError>;

    /// Returns up to `limit` visits, most recent first.
    fn recent_entries(&self, limit: usize) -> Result<Vec<HistoryEntry>, StorageError>;

    /// Returns up to `limit` visits whose URL or title contains `query`, most recent
    /// first. Wildcard characters in `query` match literally.
    fn search_entries(&self, query: &str, limit: usize) -> Result<Vec<HistoryEntry>, StorageError>;

    /// Returns the aggregate for every recorded page, for loading the suggestion index.
    fn load_suggestions(&self) -> Result<Vec<SuggestionEntry>, StorageError>;

    /// Removes the visit with the given raw id, and its page when no visit remains.
    fn remove_entry(&self, id: u64) -> Result<(), StorageError>;

    /// Removes every recorded page and visit.
    fn clear_all(&self) -> Result<(), StorageError>;

    /// Removes every page whose host equals `host`, cascading to their visits.
    fn clear_site(&self, host: &str) -> Result<(), StorageError>;

    /// Removes visits older than `cutoff`, and any page left with no remaining visit.
    fn prune_older_than(&self, cutoff: i64) -> Result<(), StorageError>;
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
