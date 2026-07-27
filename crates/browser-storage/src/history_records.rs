// @file crates/browser-storage/src/history_records.rs
// @description Data-transfer types the history store consumes and returns across its boundary.
// @layer storage
// @created meerita <meerita@icloud.com>

/// A visit to record, assembled by the caller before it reaches the store.
///
/// The `host` is extracted upstream and `visited_at` is supplied as Unix epoch seconds,
/// because the store never reads the clock and never parses URLs; both responsibilities
/// belong to the caller. The `title` is `None` when title storage is disabled.
#[derive(Debug, Clone)]
pub struct NewVisit {
    url: String,
    host: String,
    title: Option<String>,
    was_typed: bool,
    visited_at: i64,
}

impl NewVisit {
    pub fn new(
        url: String,
        host: String,
        title: Option<String>,
        was_typed: bool,
        visited_at: i64,
    ) -> Self {
        Self {
            url,
            host,
            title,
            was_typed,
            visited_at,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn was_typed(&self) -> bool {
        self.was_typed
    }

    pub fn visited_at(&self) -> i64 {
        self.visited_at
    }
}

/// One visit as read back for the history list.
///
/// `id` is the raw visit-row identifier; the domain identifier newtype lives in the core
/// crate, so storage passes the plain value across its boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    id: u64,
    url: String,
    title: Option<String>,
    visited_at: i64,
}

impl HistoryEntry {
    pub fn new(id: u64, url: String, title: Option<String>, visited_at: i64) -> Self {
        Self {
            id,
            url,
            title,
            visited_at,
        }
    }

    pub fn id(&self) -> u64 {
        self.id
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn visited_at(&self) -> i64 {
        self.visited_at
    }
}

/// The per-URL aggregate the in-memory suggestion index ranks.
///
/// It carries the raw counters and timestamp the ranking needs, never a precomputed
/// score: frecency depends on the current clock, so it is computed at rank time in the
/// core crate rather than materialized here where it would go stale between visits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuggestionEntry {
    url: String,
    host: String,
    visit_count: u32,
    typed_count: u32,
    last_visit_at: i64,
}

impl SuggestionEntry {
    pub fn new(
        url: String,
        host: String,
        visit_count: u32,
        typed_count: u32,
        last_visit_at: i64,
    ) -> Self {
        Self {
            url,
            host,
            visit_count,
            typed_count,
            last_visit_at,
        }
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn visit_count(&self) -> u32 {
        self.visit_count
    }

    pub fn typed_count(&self) -> u32 {
        self.typed_count
    }

    pub fn last_visit_at(&self) -> i64 {
        self.last_visit_at
    }
}
