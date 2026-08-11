//! @file crates/browser-core/src/lib.rs
//! @description Core crate root: tab and navigation domain types, error taxonomy, controller.
//! @layer core
//! @created meerita <meerita@icloud.com>

mod address_resolver;
mod cookie_jar;
mod cookie_record;
mod cookie_settings;
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
pub use browser_network::{BrowserUrl, RequestHeaders};
pub use browser_privacy::{CookiePolicy, CookieScope, RejectionReason, SameSite};
pub use browser_storage::{
    ConfigStore, HistoryEntry, HistoryStore, SitePolicyStore, StorageError, SuggestionEntry,
};
pub use cookie_record::CookieRecord;
pub use cookie_settings::{parse_policy, CookiePolicyPair};
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

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use browser_network::{
    fetch_once, resolve_redirect, FetchedDocument, HopOutcome, NetworkError, MAX_REDIRECT_COUNT,
};
use browser_privacy::{
    classify, decide, registrable_domain, CookieContext, CookieDecision, ParsedCookie,
};
use browser_storage::NewVisit;

use cookie_jar::{CookieJar, StoredCookie};
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
    history_stack: VecDeque<CurrentPage>,
    history: Option<Arc<dyn HistoryStore + Send + Sync>>,
    history_settings: HistorySettings,
    suggestion_index: SuggestionIndex,
    cookie_jar: CookieJar,
    default_cookie_policy: CookiePolicyPair,
    site_policies: Option<Arc<dyn SitePolicyStore + Send + Sync>>,
    site_exceptions: HashMap<String, CookiePolicy>,
    cookie_records: Vec<CookieRecord>,
    search_engine: SearchEngine,
    config_store: Option<Arc<dyn ConfigStore + Send + Sync>>,
    request_headers: RequestHeaders,
}

impl fmt::Debug for NavigationController {
    /// Hand-written because the injected stores are trait objects that carry no `Debug`
    /// bound, and because the cookie jar must be redacted. Each store is shown as a
    /// presence flag, never its contents; the jar prints only counts through its own
    /// `Debug`, so a controller stays printable in adapter diagnostics without leaking
    /// history or a cookie value.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NavigationController")
            .field("current_page", &self.current_page)
            .field("history_stack", &self.history_stack)
            .field("has_history_store", &self.history.is_some())
            .field("history_settings", &self.history_settings)
            .field("cookie_jar", &self.cookie_jar)
            .field("default_cookie_policy", &self.default_cookie_policy)
            .field("has_site_policy_store", &self.site_policies.is_some())
            .field("site_exceptions", &self.site_exceptions)
            .field("cookie_records", &self.cookie_records)
            .field("search_engine", &self.search_engine)
            .field("has_config_store", &self.config_store.is_some())
            .field("request_headers", &self.request_headers)
            .finish()
    }
}

/// The most pages the back history retains. Older pages are dropped when the stack
/// overflows so a long browsing session cannot grow memory without bound.
const MAX_HISTORY_DEPTH: usize = 50;

/// The running per-page cookie tally accumulated while a navigation is processed.
///
/// Kept separate from the session record list because the counts are per page (they reset
/// each navigation) while the records span the whole session.
#[derive(Default, Clone, Copy)]
struct CookieTally {
    accepted: usize,
    rejected: usize,
}

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
            history_stack: VecDeque::new(),
            history,
            history_settings,
            suggestion_index: SuggestionIndex::from_entries(initial_suggestions),
            cookie_jar: CookieJar::default(),
            default_cookie_policy: CookiePolicyPair::default(),
            site_policies: None,
            site_exceptions: HashMap::new(),
            cookie_records: Vec::new(),
            search_engine: SearchEngine::default(),
            config_store: None,
            request_headers: RequestHeaders::default(),
        }
    }

    /// Wires cookie enforcement into an existing controller: the default scope policy, an
    /// optional per-site policy store, and the exceptions already loaded from that store.
    ///
    /// A consuming builder so it composes with [`new`](Self::new) and
    /// [`with_history`](Self::with_history) without reshuffling their arguments. A caller
    /// that never calls this, such as the MCP server, keeps the reject-by-default pair, an
    /// empty jar, and no store, which is exactly the privacy-safe default that path needs.
    pub fn with_cookies(
        mut self,
        default: CookiePolicyPair,
        store: Option<Arc<dyn SitePolicyStore + Send + Sync>>,
        initial_exceptions: Vec<(String, CookiePolicy)>,
    ) -> Self {
        self.default_cookie_policy = default;
        self.site_policies = store;
        self.site_exceptions = initial_exceptions.into_iter().collect();
        self
    }

    /// Seeds the search engine `/search` sends queries to.
    ///
    /// A consuming builder mirroring [`with_cookies`](Self::with_cookies) so startup can
    /// override the DuckDuckGo lite default without reshuffling the other constructors. A
    /// caller that never calls this keeps [`SearchEngine::default`].
    pub fn with_search_engine(mut self, search_engine: SearchEngine) -> Self {
        self.search_engine = search_engine;
        self
    }

    /// Seeds the outbound request identity (`User-Agent`, `Accept-Language`) every fetch
    /// carries.
    ///
    /// A consuming builder mirroring [`with_search_engine`](Self::with_search_engine) so
    /// startup can override the degraded [`RequestHeaders::default`] with OS/locale
    /// detail without reshuffling the other constructors.
    pub fn with_request_headers(mut self, headers: RequestHeaders) -> Self {
        self.request_headers = headers;
        self
    }

    /// The search engine `/search` sends queries to, for the caller to read live.
    pub fn search_engine(&self) -> &SearchEngine {
        &self.search_engine
    }

    /// The global default cookie policy for both scopes, for the caller to read live.
    ///
    /// Returns a copy of the pair the resolver consults when no per-site exception applies,
    /// so an adapter can show the current first- and third-party defaults without reaching
    /// into controller internals.
    pub fn cookie_policy(&self) -> CookiePolicyPair {
        self.default_cookie_policy
    }

    /// Wires a configuration store so setting changes both apply live and persist.
    ///
    /// A consuming builder mirroring [`with_cookies`](Self::with_cookies) and
    /// [`with_search_engine`](Self::with_search_engine). When no store is wired the typed
    /// setters and [`persist_setting`](Self::persist_setting) still apply live and return
    /// `Ok`, so the MCP path and tests without persistence keep working.
    pub fn with_config_store(mut self, store: Arc<dyn ConfigStore + Send + Sync>) -> Self {
        self.config_store = Some(store);
        self
    }

    /// Persists a single configuration key and value through the wired store.
    ///
    /// The write path for terminal-only toggles whose live state lives in the terminal
    /// adapter: the adapter applies the change and calls this to record it. A no-op when no
    /// store is wired, so a toggle set in memory does not require persistence to take effect
    /// for the run. A store failure maps to [`CoreError::Storage`] by `?`, so no raw storage
    /// error crosses the core boundary. Only the caller's whitelisted keys reach the store;
    /// this never receives a cookie value, token, or password.
    pub fn persist_setting(&self, key: &str, value: &str) -> Result<(), CoreError> {
        let Some(store) = self.config_store.clone() else {
            return Ok(());
        };
        store.set_config_value(key, value)?;
        Ok(())
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
    ///
    /// The redirect loop is driven here, hop by hop, so the jar can carry accepted cookies
    /// across the hops of one navigation: each hop sends the `Cookie` header built from the
    /// jar and each hop's `Set-Cookie` lines are classified and decided before the next
    /// request. The top-level host is fixed to the initially requested host, so a cookie set
    /// on a redirect target is still classified against the site the user navigated to.
    pub async fn load_with_progress(
        &mut self,
        url: BrowserUrl,
        progress: tokio::sync::watch::Sender<usize>,
        source: NavigationSource,
    ) -> Result<(), CoreError> {
        let top_level_host = url.host_str().unwrap_or_default().to_string();
        let mut current_url = url;
        let mut hop_count: usize = 0;
        let mut tally = CookieTally::default();
        let fetched = loop {
            let cookie_header = self.cookie_jar.cookie_header_for(&current_url);
            let outcome = fetch_once(
                &current_url,
                cookie_header.as_deref(),
                &self.request_headers,
                progress.clone(),
            )
            .await?;
            match outcome {
                HopOutcome::Final(document) => {
                    self.process_set_cookies(
                        &current_url,
                        &top_level_host,
                        document.set_cookie_lines(),
                        &mut tally,
                    );
                    break document;
                }
                HopOutcome::Redirect {
                    location,
                    set_cookie_lines,
                    ..
                } => {
                    self.process_set_cookies(
                        &current_url,
                        &top_level_host,
                        &set_cookie_lines,
                        &mut tally,
                    );
                    hop_count += 1;
                    if hop_count > MAX_REDIRECT_COUNT {
                        return Err(CoreError::from(NetworkError::TooManyRedirects));
                    }
                    current_url = resolve_redirect(&current_url, &location)?;
                }
            }
        };
        self.finish_load(fetched, source, tally).await
    }

    /// Turn the final fetched document into the current page and record the visit.
    ///
    /// Splits the tail of the redirect loop out so the loop stays shallow. The byte count
    /// prefers the wire `Content-Length` and falls back to the collected body length. The
    /// previous page is pushed to the back stack before it is replaced, bounded by
    /// [`MAX_HISTORY_DEPTH`], so a long session cannot grow memory without limit.
    async fn finish_load(
        &mut self,
        fetched: FetchedDocument,
        source: NavigationSource,
        tally: CookieTally,
    ) -> Result<(), CoreError> {
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
                self.history_stack.pop_front();
            }
            self.history_stack.push_back(previous);
        }
        self.current_page = Some(CurrentPage::new(
            fetched.final_url().clone(),
            document,
            title,
            byte_count,
            tally.accepted,
            tally.rejected,
        ));
        self.record_current_visit(source).await;
        Ok(())
    }

    /// Classify and decide every `Set-Cookie` line offered on one hop.
    fn process_set_cookies(
        &mut self,
        hop_url: &BrowserUrl,
        top_level_host: &str,
        set_cookie_lines: &[String],
        tally: &mut CookieTally,
    ) {
        for set_cookie_line in set_cookie_lines {
            self.process_one_set_cookie(hop_url, top_level_host, set_cookie_line, tally);
        }
    }

    /// Classify and decide a single `Set-Cookie` line, storing or recording the outcome.
    ///
    /// A line that does not parse is recorded as malformed and dropped. A cookie whose host
    /// is a public suffix is recorded and dropped. Otherwise the effective policy is
    /// resolved (a per-site exception overrides the scope default) and the decision either
    /// stores the cookie in the jar or records the rejection with its reason.
    fn process_one_set_cookie(
        &mut self,
        hop_url: &BrowserUrl,
        top_level_host: &str,
        set_cookie_line: &str,
        tally: &mut CookieTally,
    ) {
        let Ok(cookie) = browser_privacy::parse(set_cookie_line) else {
            self.record_rejection(CookieRecord::malformed(hop_url.origin()), tally);
            return;
        };
        let hop_host = hop_url.host_str().unwrap_or_default();
        let cookie_host = cookie.domain().unwrap_or(hop_host);
        let Some(scope) = classify(cookie_host, top_level_host) else {
            let record = CookieRecord::rejected(
                hop_url.origin(),
                &cookie,
                false,
                RejectionReason::PublicSuffix,
            );
            self.record_rejection(record, tally);
            return;
        };
        let policy = self.resolve_policy(cookie_host, scope);
        let context = CookieContext {
            scope,
            request_is_secure: hop_url.scheme() == "https",
        };
        self.apply_decision(
            hop_url,
            &cookie,
            scope,
            decide(policy, &cookie, &context),
            tally,
        );
    }

    /// Store an accepted cookie or record a rejection, given the resolved decision.
    ///
    /// A third-party cookie rejected by the scope policy is recorded with the `ThirdParty`
    /// reason rather than the bare `Policy` reason, so the inspection view names why it was
    /// refused.
    fn apply_decision(
        &mut self,
        hop_url: &BrowserUrl,
        cookie: &ParsedCookie,
        scope: CookieScope,
        decision: CookieDecision,
        tally: &mut CookieTally,
    ) {
        let first_party = matches!(scope, CookieScope::FirstParty);
        match decision {
            CookieDecision::Accept { .. } => {
                // No wall-clock expiry instant is tracked this milestone: the jar is
                // in-memory and dropped at process exit, and the parsed cookie exposes only
                // whether an expiry was present, not when. Session and persistent cookies
                // alike therefore live for the run, so no instant is stored.
                let stored = StoredCookie::new(
                    cookie.name().to_string(),
                    cookie.value().clone(),
                    cookie.path().unwrap_or("/").to_string(),
                    cookie.secure(),
                    None,
                );
                self.cookie_jar.store(hop_url, stored);
                self.cookie_records.push(CookieRecord::new_accepted(
                    hop_url.origin(),
                    cookie,
                    first_party,
                ));
                tally.accepted += 1;
            }
            CookieDecision::Reject(reason) => {
                let recorded = reason_for_scope(reason, scope);
                let record =
                    CookieRecord::rejected(hop_url.origin(), cookie, first_party, recorded);
                self.record_rejection(record, tally);
            }
        }
    }

    /// Push a rejection record and bump the per-page rejected count.
    fn record_rejection(&mut self, record: CookieRecord, tally: &mut CookieTally) {
        self.cookie_records.push(record);
        tally.rejected += 1;
    }

    /// The effective policy for a cookie: a per-site exception if one is set for its
    /// registrable domain, otherwise the scope default.
    fn resolve_policy(&self, cookie_host: &str, scope: CookieScope) -> CookiePolicy {
        let exception = registrable_domain(cookie_host)
            .and_then(|domain| self.site_exceptions.get(&domain).copied());
        if let Some(policy) = exception {
            return policy;
        }
        match scope {
            CookieScope::FirstParty => self.default_cookie_policy.first_party,
            CookieScope::ThirdParty => self.default_cookie_policy.third_party,
        }
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
        match self.history_stack.pop_back() {
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

    /// The current page's `(accepted, rejected)` cookie counts, or `(0, 0)` with no page.
    ///
    /// Per-page counts, not session totals: they reflect the cookies this navigation
    /// offered so an adapter can show a per-page indicator.
    pub fn cookie_counts(&self) -> (usize, usize) {
        match &self.current_page {
            Some(page) => (page.accepted_cookie_count(), page.rejected_cookie_count()),
            None => (0, 0),
        }
    }

    /// The session's cookie inspection records, in the order the decisions were made.
    ///
    /// The list spans the whole run, not just the current page, so the inspection view can
    /// show every cookie the session has accepted or rejected. No record holds a value.
    pub fn cookie_records(&self) -> &[CookieRecord] {
        &self.cookie_records
    }

    /// Empties the session cookie jar and the inspection records.
    ///
    /// Drops every accepted cookie held in memory and clears the record list, so a user can
    /// discard the session's cookie state without restarting. Persisted per-site exceptions
    /// are untouched: this clears cookies, not the policy.
    pub fn clear_cookies(&mut self) {
        self.cookie_jar.clear();
        self.cookie_records.clear();
    }

    /// Sets a per-site cookie policy exception for `host` and persists it when a store is
    /// wired.
    ///
    /// The exception is keyed by the site's registrable domain, so it covers every
    /// subdomain. A host with no registrable domain is ignored rather than stored under a
    /// meaningless key. Setting the policy to [`CookiePolicy::Reject`] also drops that
    /// site's cookies already held in the jar, so rejecting a site takes effect at once.
    pub fn set_site_cookie_policy(
        &mut self,
        host: &str,
        policy: CookiePolicy,
    ) -> Result<(), CoreError> {
        let Some(domain) = registrable_domain(host) else {
            return Ok(());
        };
        self.site_exceptions.insert(domain.clone(), policy);
        if matches!(policy, CookiePolicy::Reject) {
            self.cookie_jar.clear_domain(&domain);
        }
        self.write_through_site_policy(&domain, policy)
    }

    /// Persists a per-site policy exception to the store, if one is wired.
    ///
    /// A no-op when no store is injected (the MCP path), so an exception set in memory does
    /// not require persistence to take effect for the run. The policy is written as its
    /// lowercase word; a store failure maps to [`CoreError::Storage`] by `?`.
    fn write_through_site_policy(
        &self,
        domain: &str,
        policy: CookiePolicy,
    ) -> Result<(), CoreError> {
        let Some(store) = self.site_policies.clone() else {
            return Ok(());
        };
        store.set_site_policy(domain, policy_word(policy), now_unix_seconds())?;
        Ok(())
    }

    /// Sets the global default cookie policy for one scope and applies it to the running
    /// session at once.
    ///
    /// The default governs every site with no per-site exception. Tightening the first-party
    /// default to [`CookiePolicy::Reject`] also drops the cookies the jar could still send:
    /// the jar only ever sends cookies first-party, so a held cookie for a domain no
    /// exception permits is now forbidden and is removed immediately, not merely on the next
    /// decision. Relaxing a scope, or changing the third-party default, leaves held cookies
    /// in place. The built-in default pair stays `Reject`/`Reject`; this changes the running
    /// value, not that default.
    ///
    /// The change is applied live first, then persisted through the wired
    /// [`ConfigStore`], mirroring [`set_site_cookie_policy`](Self::set_site_cookie_policy):
    /// a persistence failure surfaces as [`CoreError`] without leaving the live value unset.
    /// With no store wired, the change applies live and returns `Ok`.
    pub fn set_global_cookie_policy(
        &mut self,
        scope: CookieScope,
        policy: CookiePolicy,
    ) -> Result<(), CoreError> {
        self.assign_scope_policy(scope, policy);
        if first_party_tightened_to_reject(scope, policy) {
            self.drop_cookies_without_permitting_exception();
        }
        self.persist_setting(config_key_for_scope(scope), policy_word(policy))
    }

    /// Replaces the search engine `/search` uses, applying it live and persisting it.
    ///
    /// The engine is validated through [`SearchEngine::new`], so a `file://` or otherwise
    /// malformed base URL returns [`CoreError`] and nothing is applied or persisted. On
    /// success the held engine is replaced first, then `search.base_url` and
    /// `search.query_parameter` are written through the wired [`ConfigStore`]. With no store
    /// wired, the engine is replaced and `Ok` is returned.
    pub fn set_search_engine(
        &mut self,
        base_url: String,
        query_parameter: String,
    ) -> Result<(), CoreError> {
        self.search_engine = SearchEngine::new(base_url, query_parameter)?;
        self.persist_setting("search.base_url", self.search_engine.base_url())?;
        self.persist_setting(
            "search.query_parameter",
            self.search_engine.query_parameter(),
        )
    }

    /// Writes `policy` into the matching scope field of the default policy pair.
    fn assign_scope_policy(&mut self, scope: CookieScope, policy: CookiePolicy) {
        match scope {
            CookieScope::FirstParty => self.default_cookie_policy.first_party = policy,
            CookieScope::ThirdParty => self.default_cookie_policy.third_party = policy,
        }
    }

    /// Drops every jar cookie whose domain relies on the global default, keeping only those a
    /// per-site exception still permits.
    ///
    /// Called when the first-party default tightens to reject. A domain with its own
    /// permitting exception is governed by that exception, not the global default, so its
    /// cookies survive; every other held cookie is now forbidden and is removed.
    fn drop_cookies_without_permitting_exception(&mut self) {
        let site_exceptions = &self.site_exceptions;
        self.cookie_jar
            .retain_domains(|domain| domain_has_permitting_exception(site_exceptions, domain));
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

/// Whether a global cookie change tightens the first-party default to reject.
///
/// Only this case drops cookies already held: the jar sends cookies first-party only, so a
/// first-party reject forbids every held cookie no exception still permits. Relaxing, or any
/// change to the third-party default, leaves the jar untouched.
fn first_party_tightened_to_reject(scope: CookieScope, policy: CookiePolicy) -> bool {
    matches!(scope, CookieScope::FirstParty) && matches!(policy, CookiePolicy::Reject)
}

/// Whether a domain keeps its cookies when the global first-party default tightens to reject.
///
/// A per-site exception that is not reject governs that domain in place of the global
/// default, so its cookies survive the tightening. A domain with no exception follows the
/// default and loses them.
fn domain_has_permitting_exception(
    site_exceptions: &HashMap<String, CookiePolicy>,
    domain: &str,
) -> bool {
    match site_exceptions.get(domain) {
        Some(policy) => !matches!(policy, CookiePolicy::Reject),
        None => false,
    }
}

/// Refines a rejection reason for how it is recorded.
///
/// A third-party cookie the scope policy rejects comes back as the bare `Policy` reason;
/// it is recorded as `ThirdParty` instead so the inspection view names the real cause. All
/// other reasons pass through unchanged.
fn reason_for_scope(reason: RejectionReason, scope: CookieScope) -> RejectionReason {
    let policy_rejected_third_party =
        matches!(reason, RejectionReason::Policy) && matches!(scope, CookieScope::ThirdParty);
    if policy_rejected_third_party {
        return RejectionReason::ThirdParty;
    }
    reason
}

/// The lowercase policy word persisted for a [`CookiePolicy`].
///
/// The inverse of [`parse_policy`](crate::parse_policy): the store keeps the policy as
/// opaque text, so this maps the domain enum to the word the store round-trips.
fn policy_word(policy: CookiePolicy) -> &'static str {
    match policy {
        CookiePolicy::Allow => "allow",
        CookiePolicy::Session => "session",
        CookiePolicy::Ask => "ask",
        CookiePolicy::Reject => "reject",
    }
}

/// The `ConfigStore` key the global default policy for `scope` is persisted under.
///
/// The two scopes map to the two fixed config keys the panel and startup read back, so a
/// global cookie change round-trips to the same field it came from.
fn config_key_for_scope(scope: CookieScope) -> &'static str {
    match scope {
        CookieScope::FirstParty => "cookies.first_party",
        CookieScope::ThirdParty => "cookies.third_party",
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
