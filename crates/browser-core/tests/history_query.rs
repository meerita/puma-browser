// @file crates/browser-core/tests/history_query.rs
// @description Tests the controller's async history query API against a fake history store.
// @layer core
// @created meerita <meerita@icloud.com>

use std::sync::{Arc, Mutex};

use browser_core::{
    HistoryEntry, HistoryEntryId, HistoryMode, HistorySettings, HistoryStore, NavigationController,
    SuggestionEntry,
};
use browser_storage::{NewVisit, StorageError};

/// A history store that returns fixed rows and records the mutations made against it.
#[derive(Default)]
struct QueryStore {
    recent: Vec<HistoryEntry>,
    search: Vec<HistoryEntry>,
    cleared_all: Mutex<bool>,
    cleared_sites: Mutex<Vec<String>>,
    removed_ids: Mutex<Vec<u64>>,
}

impl QueryStore {
    fn cleared_all(&self) -> bool {
        *self.cleared_all.lock().expect("mutex must not be poisoned")
    }

    fn cleared_sites(&self) -> Vec<String> {
        self.cleared_sites
            .lock()
            .expect("mutex must not be poisoned")
            .clone()
    }

    fn removed_ids(&self) -> Vec<u64> {
        self.removed_ids
            .lock()
            .expect("mutex must not be poisoned")
            .clone()
    }
}

impl HistoryStore for QueryStore {
    fn record_visit(&self, visit: NewVisit) -> Result<SuggestionEntry, StorageError> {
        Ok(SuggestionEntry::new(
            visit.url().to_string(),
            visit.host().to_string(),
            1,
            0,
            visit.visited_at(),
        ))
    }

    fn recent_entries(&self, limit: usize) -> Result<Vec<HistoryEntry>, StorageError> {
        Ok(self.recent.iter().take(limit).cloned().collect())
    }

    fn search_entries(
        &self,
        _query: &str,
        limit: usize,
    ) -> Result<Vec<HistoryEntry>, StorageError> {
        Ok(self.search.iter().take(limit).cloned().collect())
    }

    fn load_suggestions(&self) -> Result<Vec<SuggestionEntry>, StorageError> {
        Ok(Vec::new())
    }

    fn remove_entry(&self, id: u64) -> Result<(), StorageError> {
        self.removed_ids
            .lock()
            .expect("mutex must not be poisoned")
            .push(id);
        Ok(())
    }

    fn clear_all(&self) -> Result<(), StorageError> {
        *self.cleared_all.lock().expect("mutex must not be poisoned") = true;
        Ok(())
    }

    fn clear_site(&self, host: &str) -> Result<(), StorageError> {
        self.cleared_sites
            .lock()
            .expect("mutex must not be poisoned")
            .push(host.to_string());
        Ok(())
    }

    fn prune_older_than(&self, _cutoff: i64) -> Result<(), StorageError> {
        Ok(())
    }
}

fn entry(id: u64, url: &str) -> HistoryEntry {
    HistoryEntry::new(id, url.to_string(), None, 100)
}

fn controller_with(store: Arc<QueryStore>, mode: HistoryMode) -> NavigationController {
    let history: Option<Arc<dyn HistoryStore + Send + Sync>> = Some(store);
    NavigationController::with_history(history, HistorySettings::new(mode, 90, true), Vec::new())
}

#[tokio::test]
async fn recent_history_returns_the_stores_recent_rows() {
    let store = Arc::new(QueryStore {
        recent: vec![entry(1, "https://a.test/"), entry(2, "https://b.test/")],
        ..QueryStore::default()
    });
    let controller = controller_with(store, HistoryMode::Persistent);

    let entries = controller
        .recent_history(10)
        .await
        .expect("recent history must read");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].url(), "https://a.test/");
}

#[tokio::test]
async fn search_history_returns_the_stores_matching_rows() {
    let store = Arc::new(QueryStore {
        search: vec![entry(3, "https://example.com/rust")],
        ..QueryStore::default()
    });
    let controller = controller_with(store, HistoryMode::Persistent);

    let entries = controller
        .search_history("rust", 10)
        .await
        .expect("search history must read");

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].url(), "https://example.com/rust");
}

#[tokio::test]
async fn clear_history_calls_through_to_the_store() {
    let store = Arc::new(QueryStore::default());
    let mut controller = controller_with(store.clone(), HistoryMode::Persistent);

    controller
        .clear_history()
        .await
        .expect("clear must succeed");

    assert!(store.cleared_all());
}

#[tokio::test]
async fn clear_history_site_calls_through_with_the_host() {
    let store = Arc::new(QueryStore::default());
    let mut controller = controller_with(store.clone(), HistoryMode::Persistent);

    controller
        .clear_history_site("example.com")
        .await
        .expect("clear site must succeed");

    assert_eq!(store.cleared_sites(), vec!["example.com".to_string()]);
}

#[tokio::test]
async fn remove_history_entry_passes_the_raw_id_to_the_store() {
    let store = Arc::new(QueryStore::default());
    let controller = controller_with(store.clone(), HistoryMode::Persistent);

    controller
        .remove_history_entry(HistoryEntryId::new(42))
        .await
        .expect("remove must succeed");

    assert_eq!(store.removed_ids(), vec![42]);
}

#[tokio::test]
async fn disabled_mode_returns_empty_recent_history_without_touching_the_store() {
    let store = Arc::new(QueryStore {
        recent: vec![entry(1, "https://a.test/")],
        ..QueryStore::default()
    });
    let controller = controller_with(store, HistoryMode::Disabled);

    let entries = controller
        .recent_history(10)
        .await
        .expect("recent history must read");

    assert!(entries.is_empty());
}

#[tokio::test]
async fn disabled_mode_searches_nothing() {
    let store = Arc::new(QueryStore {
        search: vec![entry(1, "https://a.test/")],
        ..QueryStore::default()
    });
    let controller = controller_with(store, HistoryMode::Disabled);

    let entries = controller
        .search_history("a", 10)
        .await
        .expect("search history must read");

    assert!(entries.is_empty());
}

#[tokio::test]
async fn disabled_mode_clears_nothing() {
    let store = Arc::new(QueryStore::default());
    let mut controller = controller_with(store.clone(), HistoryMode::Disabled);

    controller
        .clear_history()
        .await
        .expect("clear must be a no-op");
    controller
        .clear_history_site("example.com")
        .await
        .expect("clear site must be a no-op");

    assert!(!store.cleared_all());
    assert!(store.cleared_sites().is_empty());
}

#[tokio::test]
async fn disabled_mode_removes_nothing() {
    let store = Arc::new(QueryStore::default());
    let controller = controller_with(store.clone(), HistoryMode::Disabled);

    controller
        .remove_history_entry(HistoryEntryId::new(9))
        .await
        .expect("remove must be a no-op");

    assert!(store.removed_ids().is_empty());
}
