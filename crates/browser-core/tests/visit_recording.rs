// @file crates/browser-core/tests/visit_recording.rs
// @description Tests the visit-recording gate and index update against a fake history store.
// @layer core
// @created meerita <meerita@icloud.com>

use std::sync::{Arc, Mutex};

use browser_core::{
    resolve_address, BrowserUrl, HistoryMode, HistorySettings, HistoryStore, NavigationController,
    NavigationSource, SuggestionEntry,
};
use browser_storage::{HistoryEntry, NewVisit, StorageError};
use tempfile::tempdir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// An in-memory history store that captures every recorded visit, standing in for SQLite.
#[derive(Default)]
struct RecordingStore {
    visits: Mutex<Vec<NewVisit>>,
}

impl RecordingStore {
    fn recorded(&self) -> Vec<NewVisit> {
        self.visits
            .lock()
            .expect("the store mutex must not be poisoned")
            .clone()
    }
}

impl HistoryStore for RecordingStore {
    fn record_visit(&self, visit: NewVisit) -> Result<SuggestionEntry, StorageError> {
        let entry = SuggestionEntry::new(
            visit.url().to_string(),
            visit.host().to_string(),
            1,
            u32::from(visit.was_typed()),
            visit.visited_at(),
        );
        self.visits
            .lock()
            .expect("the store mutex must not be poisoned")
            .push(visit);
        Ok(entry)
    }

    fn recent_entries(&self, _limit: usize) -> Result<Vec<HistoryEntry>, StorageError> {
        Ok(Vec::new())
    }

    fn search_entries(
        &self,
        _query: &str,
        _limit: usize,
    ) -> Result<Vec<HistoryEntry>, StorageError> {
        Ok(Vec::new())
    }

    fn load_suggestions(&self) -> Result<Vec<SuggestionEntry>, StorageError> {
        Ok(Vec::new())
    }

    fn remove_entry(&self, _id: u64) -> Result<(), StorageError> {
        Ok(())
    }

    fn clear_all(&self) -> Result<(), StorageError> {
        Ok(())
    }

    fn clear_site(&self, _host: &str) -> Result<(), StorageError> {
        Ok(())
    }

    fn prune_older_than(&self, _cutoff: i64) -> Result<(), StorageError> {
        Ok(())
    }
}

/// Mounts a single `GET /` page returning `body`.
async fn mount_page(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_string()))
        .mount(server)
        .await;
}

/// Builds a controller wired to `store` under `settings`, with an empty initial index.
fn controller_with(store: Arc<RecordingStore>, settings: HistorySettings) -> NavigationController {
    let history: Option<Arc<dyn HistoryStore + Send + Sync>> = Some(store);
    NavigationController::with_history(history, settings, Vec::new())
}

fn settings(mode: HistoryMode, store_titles: bool) -> HistorySettings {
    HistorySettings::new(mode, 90, store_titles)
}

#[tokio::test]
async fn a_successful_http_load_records_one_visit_and_updates_the_index() {
    let server = MockServer::start().await;
    mount_page(&server, "<html><body><p>Hello</p></body></html>").await;
    let store = Arc::new(RecordingStore::default());
    let mut controller = controller_with(store.clone(), settings(HistoryMode::Persistent, true));
    let url = BrowserUrl::parse(&server.uri()).expect("the wiremock URI must parse");

    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("loading a page must succeed");

    assert_eq!(store.recorded().len(), 1);
    let suggestions = controller.suggest("127", 8);
    assert_eq!(
        suggestions.len(),
        1,
        "the recorded visit must reach the index"
    );
}

#[tokio::test]
async fn a_file_url_load_records_no_visit() {
    let working_directory = tempdir().expect("a temporary directory must be created");
    std::fs::write(
        working_directory.path().join("page.html"),
        "<html><body><p>Local</p></body></html>",
    )
    .expect("the temporary HTML file must be written");
    let url = resolve_address("page.html", working_directory.path())
        .expect("a local HTML file must resolve to a file URL");
    let store = Arc::new(RecordingStore::default());
    let mut controller = controller_with(store.clone(), settings(HistoryMode::Persistent, true));

    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("loading a local file must succeed");

    assert!(
        store.recorded().is_empty(),
        "a file URL must never be recorded"
    );
}

#[tokio::test]
async fn disabled_mode_records_nothing_and_offers_no_suggestions() {
    let server = MockServer::start().await;
    mount_page(&server, "<html><body><p>Hello</p></body></html>").await;
    let store = Arc::new(RecordingStore::default());
    let mut controller = controller_with(store.clone(), settings(HistoryMode::Disabled, true));
    let url = BrowserUrl::parse(&server.uri()).expect("the wiremock URI must parse");

    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("loading a page must succeed");

    assert!(
        store.recorded().is_empty(),
        "disabled mode must record nothing"
    );
    assert!(
        controller.suggest("127", 8).is_empty(),
        "disabled mode must offer no suggestions"
    );
}

#[tokio::test]
async fn a_typed_navigation_records_was_typed_true() {
    let server = MockServer::start().await;
    mount_page(&server, "<html><body><p>Hello</p></body></html>").await;
    let store = Arc::new(RecordingStore::default());
    let mut controller = controller_with(store.clone(), settings(HistoryMode::Persistent, true));
    let url = BrowserUrl::parse(&server.uri()).expect("the wiremock URI must parse");

    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("loading a page must succeed");

    let recorded = store.recorded();
    assert_eq!(recorded.len(), 1);
    assert!(recorded[0].was_typed(), "an address-bar visit is typed");
}

#[tokio::test]
async fn a_followed_link_navigation_records_was_typed_false() {
    let server = MockServer::start().await;
    mount_page(&server, "<html><body><p>Hello</p></body></html>").await;
    let store = Arc::new(RecordingStore::default());
    let mut controller = controller_with(store.clone(), settings(HistoryMode::Persistent, true));
    let url = BrowserUrl::parse(&server.uri()).expect("the wiremock URI must parse");

    controller
        .load(url, NavigationSource::Link)
        .await
        .expect("loading a page must succeed");

    let recorded = store.recorded();
    assert_eq!(recorded.len(), 1);
    assert!(
        !recorded[0].was_typed(),
        "a followed-link visit is not typed"
    );
}

#[tokio::test]
async fn with_title_storage_disabled_the_recorded_title_is_none() {
    let server = MockServer::start().await;
    mount_page(
        &server,
        "<html><head><title>Example Title</title></head><body><p>x</p></body></html>",
    )
    .await;
    let store = Arc::new(RecordingStore::default());
    let mut controller = controller_with(store.clone(), settings(HistoryMode::Persistent, false));
    let url = BrowserUrl::parse(&server.uri()).expect("the wiremock URI must parse");

    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("loading a page must succeed");

    let recorded = store.recorded();
    assert_eq!(recorded.len(), 1);
    assert!(
        recorded[0].title().is_none(),
        "no title is stored when title storage is disabled"
    );
}

#[tokio::test]
async fn with_title_storage_enabled_the_page_title_is_recorded() {
    let server = MockServer::start().await;
    mount_page(
        &server,
        "<html><head><title>Example Title</title></head><body><p>x</p></body></html>",
    )
    .await;
    let store = Arc::new(RecordingStore::default());
    let mut controller = controller_with(store.clone(), settings(HistoryMode::Persistent, true));
    let url = BrowserUrl::parse(&server.uri()).expect("the wiremock URI must parse");

    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("loading a page must succeed");

    let recorded = store.recorded();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].title(), Some("Example Title"));
}
