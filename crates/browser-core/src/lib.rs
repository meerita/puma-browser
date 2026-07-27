//! @file crates/browser-core/src/lib.rs
//! @description Core crate root: tab and navigation domain types, error taxonomy, controller.
//! @layer core
//! @created meerita <meerita@icloud.com>

mod address_resolver;
mod current_page;
mod error;
mod frecency;
mod history_mode;
mod ids;
mod navigation_source;
mod navigation_target;
mod search_engine;
mod suggestion_index;
mod tab_id;
mod tab_state;

pub use address_resolver::resolve_address;
pub use browser_html::{Document, DocumentTitle};
pub use browser_layout::CellBuffer;
pub use browser_network::BrowserUrl;
pub use browser_storage::{HistoryEntry, HistoryStore, SuggestionEntry};
pub use error::CoreError;
pub use frecency::frecency;
pub use history_mode::{history_mode_from_str, HistoryMode, HistorySettings};
pub use ids::{BookmarkId, HistoryEntryId};
pub use navigation_source::NavigationSource;
pub use navigation_target::{classify_navigation, NavigationTarget, TrackingUnwrap};
pub use search_engine::SearchEngine;
pub use suggestion_index::SuggestionIndex;
pub use tab_id::TabId;
pub use tab_state::TabState;

use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use browser_storage::{NewVisit, StorageError};

use current_page::CurrentPage;

/// Orchestrates the fetch, parse, and render pipeline for the loaded page.
///
/// This is the application core the output adapters build on. This milestone holds a
/// single page: [`load`](Self::load) fetches and parses it, and [`render`](Self::render)
/// lays it out on demand for a given terminal width. Tabs, history, forms, and downloads
/// are not implemented yet.
#[derive(Default)]
pub struct NavigationController {
    current_page: Option<CurrentPage>,
    history_stack: Vec<CurrentPage>,
    history: Option<Arc<dyn HistoryStore + Send + Sync>>,
    history_settings: HistorySettings,
    suggestion_index: SuggestionIndex,
}

impl fmt::Debug for NavigationController {
    /// Hand-written because the injected [`HistoryStore`] is a trait object that carries
    /// no `Debug` bound. The store is shown as a presence flag, never its contents, so a
    /// controller stays printable in adapter diagnostics without leaking history.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NavigationController")
            .field("current_page", &self.current_page)
            .field("history_stack", &self.history_stack)
            .field("has_history_store", &self.history.is_some())
            .field("history_settings", &self.history_settings)
            .finish()
    }
}

/// The most pages the back history retains. Older pages are dropped when the stack
/// overflows so a long browsing session cannot grow memory without bound.
const MAX_HISTORY_DEPTH: usize = 50;

impl NavigationController {
    /// Builds a controller with no persisted history.
    ///
    /// The history mode defaults to [`HistoryMode::Disabled`], so this path records
    /// nothing and offers no suggestions. It serves the MCP server and any caller that
    /// does not inject a store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a controller wired to a history store, resolved settings, and the initial
    /// suggestion index loaded from that store.
    ///
    /// `history` is `None` when the resolved mode is [`HistoryMode::Disabled`]; recording
    /// and suggestions are then suppressed even though the settings are held.
    pub fn with_history(
        history: Option<Arc<dyn HistoryStore + Send + Sync>>,
        history_settings: HistorySettings,
        initial_suggestions: Vec<SuggestionEntry>,
    ) -> Self {
        Self {
            current_page: None,
            history_stack: Vec::new(),
            history,
            history_settings,
            suggestion_index: SuggestionIndex::from_entries(initial_suggestions),
        }
    }

    /// Fetch and parse the document at `url`, then hold it as the current page.
    ///
    /// The fetch follows redirects, so the stored URL is the one the request finally
    /// resolved to, not necessarily `url`. Layout is deliberately not run here; it runs
    /// in [`render`](Self::render) because it depends on the terminal width. Network and
    /// parse failures are mapped into [`CoreError`] by `?`.
    pub async fn load(
        &mut self,
        url: BrowserUrl,
        source: NavigationSource,
    ) -> Result<(), CoreError> {
        let (progress_tx, _) = tokio::sync::watch::channel(0usize);
        self.load_with_progress(url, progress_tx, source).await
    }

    /// Fetch and parse the document at `url`, streaming byte-count updates to `progress`.
    ///
    /// Behaves identically to [`load`](Self::load) but reports the running total of bytes
    /// received through `progress` after each chunk so callers can display live progress
    /// without depending on network internals.
    pub async fn load_with_progress(
        &mut self,
        url: BrowserUrl,
        progress: tokio::sync::watch::Sender<usize>,
        source: NavigationSource,
    ) -> Result<(), CoreError> {
        let fetched = browser_network::fetch_with_progress(&url, progress).await?;
        let body_byte_count = fetched.body_bytes().len();
        let wire_byte_count = fetched.wire_byte_count();
        let byte_count = if wire_byte_count > 0 {
            wire_byte_count
        } else {
            body_byte_count
        };
        let document = browser_html::parse_html_with_base(
            fetched.body_bytes(),
            fetched.charset(),
            Some(fetched.final_url().as_str()),
        )?;
        let title = document.title().cloned();
        // Push the current page to history before replacing it, so Backspace can restore
        // it without a second network round-trip.
        if let Some(previous) = self.current_page.take() {
            if self.history_stack.len() >= MAX_HISTORY_DEPTH {
                self.history_stack.remove(0);
            }
            self.history_stack.push(previous);
        }
        self.current_page = Some(CurrentPage::new(
            fetched.final_url().clone(),
            document,
            title,
            byte_count,
        ));
        self.record_current_visit(source).await;
        Ok(())
    }

    /// Returns up to `limit` ranked address-bar suggestions matching `input`.
    ///
    /// Empty when history is [`HistoryMode::Disabled`], so a controller with recording
    /// turned off never surfaces suggestions. Ranking uses the current clock, so recency
    /// reflects the moment of the query.
    pub fn suggest(&self, input: &str, limit: usize) -> Vec<SuggestionEntry> {
        if self.history_settings.mode() == HistoryMode::Disabled {
            return Vec::new();
        }
        self.suggestion_index
            .suggest(input, now_unix_seconds(), limit)
    }

    /// Returns up to `limit` recent history entries, most recent first.
    ///
    /// Empty when history is [`HistoryMode::Disabled`] or no store is wired. The
    /// synchronous store read runs on a blocking thread so the async caller is never
    /// blocked on SQLite, and any store failure maps to [`CoreError::Storage`].
    pub async fn recent_history(&self, limit: usize) -> Result<Vec<HistoryEntry>, CoreError> {
        let Some(store) = self.enabled_store() else {
            return Ok(Vec::new());
        };
        let read = tokio::task::spawn_blocking(move || store.recent_entries(limit)).await;
        flatten_storage_result(read)
    }

    /// Returns up to `limit` history entries whose URL or title contains `query`.
    ///
    /// Empty when history is [`HistoryMode::Disabled`] or no store is wired. Behaves like
    /// [`recent_history`](Self::recent_history) for blocking and error mapping.
    pub async fn search_history(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<HistoryEntry>, CoreError> {
        let Some(store) = self.enabled_store() else {
            return Ok(Vec::new());
        };
        let owned_query = query.to_string();
        let read =
            tokio::task::spawn_blocking(move || store.search_entries(&owned_query, limit)).await;
        flatten_storage_result(read)
    }

    /// Clears all recorded history and empties the in-memory suggestion index.
    ///
    /// A no-op when history is [`HistoryMode::Disabled`] or no store is wired. The index
    /// is emptied on success so cleared URLs stop surfacing as suggestions at once.
    pub async fn clear_history(&mut self) -> Result<(), CoreError> {
        let Some(store) = self.enabled_store() else {
            return Ok(());
        };
        let cleared = tokio::task::spawn_blocking(move || store.clear_all()).await;
        flatten_storage_result(cleared)?;
        self.suggestion_index = SuggestionIndex::default();
        Ok(())
    }

    /// Clears every recorded page for `host` and drops its entries from the index.
    ///
    /// A no-op when history is [`HistoryMode::Disabled`] or no store is wired.
    pub async fn clear_history_site(&mut self, host: &str) -> Result<(), CoreError> {
        let Some(store) = self.enabled_store() else {
            return Ok(());
        };
        let owned_host = host.to_string();
        let store_host = owned_host.clone();
        let cleared = tokio::task::spawn_blocking(move || store.clear_site(&store_host)).await;
        flatten_storage_result(cleared)?;
        self.suggestion_index.remove_host(&owned_host);
        Ok(())
    }

    /// Removes the history entry with the given identifier.
    ///
    /// A no-op when history is [`HistoryMode::Disabled`] or no store is wired. The raw id
    /// the store expects is taken from the [`HistoryEntryId`] newtype at this boundary.
    pub async fn remove_history_entry(&self, id: HistoryEntryId) -> Result<(), CoreError> {
        let Some(store) = self.enabled_store() else {
            return Ok(());
        };
        let raw_id = id.value();
        let removed = tokio::task::spawn_blocking(move || store.remove_entry(raw_id)).await;
        flatten_storage_result(removed)
    }

    /// The wired history store when recording is enabled, or `None` when history is
    /// disabled or no store was injected.
    ///
    /// A single gate so every history query suppresses under [`HistoryMode::Disabled`]
    /// without repeating the check at each call site.
    fn enabled_store(&self) -> Option<Arc<dyn HistoryStore + Send + Sync>> {
        if self.history_settings.mode() == HistoryMode::Disabled {
            return None;
        }
        self.history.clone()
    }

    /// Records the current page as a visit, then updates the suggestion index in place.
    ///
    /// A visit is recorded only when [`should_record`] passes, so a disabled mode or a
    /// non-web scheme writes nothing. The write runs on a blocking thread because the
    /// store is synchronous SQLite; the connection lock is never held across the await. A
    /// store failure is swallowed: a history write must never fail a navigation.
    async fn record_current_visit(&mut self, source: NavigationSource) {
        let Some(page) = self.current_page.as_ref() else {
            return;
        };
        if !should_record(page.final_url(), self.history_settings.mode()) {
            return;
        }
        let Some(store) = self.history.clone() else {
            return;
        };
        let visit = build_visit(page, source, self.history_settings.store_titles());
        let recorded = tokio::task::spawn_blocking(move || store.record_visit(visit)).await;
        let Ok(Ok(entry)) = recorded else {
            return;
        };
        self.suggestion_index.upsert(entry);
    }

    /// Returns `true` when there is at least one page in the history stack.
    pub fn can_go_back(&self) -> bool {
        !self.history_stack.is_empty()
    }

    /// Restores the most recently visited page from the history stack.
    ///
    /// Returns `true` if a page was restored, `false` when the stack was empty. The
    /// restored page becomes the current page without a network call.
    pub fn go_back(&mut self) -> bool {
        match self.history_stack.pop() {
            Some(page) => {
                self.current_page = Some(page);
                true
            }
            None => false,
        }
    }

    /// Lay the current page out into a cell buffer sized to `width` columns.
    ///
    /// With no page loaded this returns a blank buffer rather than an error, so the
    /// terminal can draw an empty page. With a page loaded it runs layout and maps a
    /// [`LayoutError`](browser_layout::LayoutError) into [`CoreError`] by `?`.
    pub fn render(&self, width: u16) -> Result<CellBuffer, CoreError> {
        let Some(page) = &self.current_page else {
            return Ok(CellBuffer::new(width, 0));
        };
        let buffer = browser_layout::render_document(
            page.document(),
            width,
            &browser_layout::WidthConfig::default(),
        )?;
        Ok(buffer)
    }

    /// The title of the current page, if one is loaded and it declared a title.
    pub fn current_title(&self) -> Option<&DocumentTitle> {
        self.current_page.as_ref().and_then(CurrentPage::title)
    }

    /// The URL the current page finally resolved to, if one is loaded.
    pub fn current_url(&self) -> Option<&BrowserUrl> {
        self.current_page.as_ref().map(CurrentPage::final_url)
    }

    /// The parsed document for the current page, or `None` when no page is loaded.
    pub fn current_document(&self) -> Option<&Document> {
        self.current_page.as_ref().map(|page| page.document())
    }

    /// Whether a page is currently loaded.
    pub fn has_page(&self) -> bool {
        self.current_page.is_some()
    }

    /// The byte count of the current page's raw response body, or zero when no page is loaded.
    pub fn page_byte_count(&self) -> usize {
        self.current_page
            .as_ref()
            .map_or(0, CurrentPage::byte_count)
    }

    /// How many `<script>` elements the current page suppressed, or zero when no page is
    /// loaded.
    pub fn script_count(&self) -> usize {
        match &self.current_page {
            Some(page) => page.document().script_count(),
            None => 0,
        }
    }

    /// Closes the tab with the given identifier.
    ///
    /// Not implemented in this milestone; returns [`CoreError::TabNotFound`].
    pub fn close_tab(&mut self, _tab: TabId) -> Result<(), CoreError> {
        Err(CoreError::TabNotFound)
    }
}

/// Flattens the nested result of a `spawn_blocking` storage call into a [`CoreError`].
///
/// A store error crosses as [`CoreError::Storage`]. A join failure means the blocking
/// task did not complete, which is treated as a failed query rather than a panic so a
/// history read or clear never brings the browser down.
fn flatten_storage_result<T>(
    result: Result<Result<T, StorageError>, tokio::task::JoinError>,
) -> Result<T, CoreError> {
    match result {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(error)) => Err(CoreError::Storage(error)),
        Err(_) => Err(CoreError::Storage(StorageError::QueryFailed)),
    }
}

/// Whether a page reached over `url` should be recorded under `mode`.
///
/// A single predicate so the recording gate has one home: a disabled mode records
/// nothing, and only `http`/`https` pages are recorded, never `file://`. A future
/// per-session opt-out slots in here without reshaping the call site.
fn should_record(url: &BrowserUrl, mode: HistoryMode) -> bool {
    if mode == HistoryMode::Disabled {
        return false;
    }
    scheme_is_recordable(url)
}

/// Whether `url` uses a scheme whose visits are recorded.
fn scheme_is_recordable(url: &BrowserUrl) -> bool {
    matches!(url.scheme(), "http" | "https")
}

/// Assembles a [`NewVisit`] from the current page.
///
/// The stored URL comes from `Display`, which strips any `user:pass@` userinfo, so
/// credentials in a URL never reach history. The host is taken from the parsed URL. The
/// title is stored only when title storage is enabled. `visited_at` is stamped now.
fn build_visit(page: &CurrentPage, source: NavigationSource, store_titles: bool) -> NewVisit {
    let url = page.final_url().to_string();
    let host = page.final_url().host_str().unwrap_or_default().to_string();
    let title = title_to_store(page, store_titles);
    NewVisit::new(url, host, title, source.was_typed(), now_unix_seconds())
}

/// The title to record for `page`, or `None` when title storage is disabled or the page
/// declared no title.
fn title_to_store(page: &CurrentPage, store_titles: bool) -> Option<String> {
    if !store_titles {
        return None;
    }
    page.title().map(|title| title.as_str().to_string())
}

/// The current time as Unix epoch seconds.
///
/// A clock before the Unix epoch is impossible on a healthy system; the `unwrap_or(0)`
/// keeps this total rather than panicking on a misconfigured clock, and a zero timestamp
/// only makes an entry rank as maximally stale.
fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}
