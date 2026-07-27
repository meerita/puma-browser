// @file crates/browser-core/src/suggestion_index.rs
// @description In-memory index of visited pages, filtered by prefix and ranked by frecency.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_storage::SuggestionEntry;

use crate::frecency::frecency;

/// A stored entry alongside its lowercased match keys.
///
/// The keys are computed once when an entry enters the index so a query scans without
/// allocating: an address-bar keystroke can rank the whole index on every character, and
/// recomputing three lowercased strings per entry per keystroke would show up as latency.
struct IndexedEntry {
    entry: SuggestionEntry,
    host_key: String,
    url_key: String,
    host_path_key: String,
}

impl IndexedEntry {
    fn build(entry: SuggestionEntry) -> Self {
        let url_key = scheme_stripped(entry.url());
        let host_path_key = host_and_path(&url_key);
        Self {
            host_key: entry.host().to_ascii_lowercase(),
            url_key,
            host_path_key,
            entry,
        }
    }

    /// Whether `needle` (already lowercased) is a prefix of any of the three match keys.
    fn matches(&self, needle: &str) -> bool {
        self.host_key.starts_with(needle)
            || self.url_key.starts_with(needle)
            || self.host_path_key.starts_with(needle)
    }
}

/// The in-memory suggestion index the address bar queries.
///
/// It is loaded once at startup from the store's aggregates and updated in place on each
/// visit; it never persists. Ranking is computed at query time from the current clock, so
/// recency reflects the moment of the query rather than the moment of the last write.
#[derive(Default)]
pub struct SuggestionIndex {
    entries: Vec<IndexedEntry>,
}

impl SuggestionIndex {
    /// Builds an index from the aggregates loaded from the store.
    pub fn from_entries(entries: Vec<SuggestionEntry>) -> Self {
        Self {
            entries: entries.into_iter().map(IndexedEntry::build).collect(),
        }
    }

    /// Removes every indexed entry whose host equals `host`, compared case-insensitively.
    ///
    /// Called when a site is cleared from history so its URLs stop surfacing as
    /// suggestions immediately, without reloading the index from the store.
    pub fn remove_host(&mut self, host: &str) {
        let needle = host.to_ascii_lowercase();
        self.entries.retain(|indexed| indexed.host_key != needle);
    }

    /// Inserts `entry`, replacing any existing entry for the same URL rather than
    /// duplicating it, so a repeat visit updates the counters in place.
    pub fn upsert(&mut self, entry: SuggestionEntry) {
        let indexed = IndexedEntry::build(entry);
        let existing = self
            .entries
            .iter_mut()
            .find(|candidate| candidate.entry.url() == indexed.entry.url());
        match existing {
            Some(slot) => *slot = indexed,
            None => self.entries.push(indexed),
        }
    }

    /// Returns up to `limit` entries whose keys start with `input`, ranked by frecency at
    /// `now` (descending), ties broken by the most recent visit.
    ///
    /// An empty input yields nothing: suggestions appear once the user starts typing, not
    /// as an unprompted dump of the whole index.
    pub fn suggest(&self, input: &str, now: i64, limit: usize) -> Vec<SuggestionEntry> {
        let needle = input.trim().to_ascii_lowercase();
        if needle.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(i64, &IndexedEntry)> = self
            .entries
            .iter()
            .filter(|candidate| candidate.matches(&needle))
            .map(|candidate| (score_at(&candidate.entry, now), candidate))
            .collect();
        scored.sort_by(|left, right| {
            right.0.cmp(&left.0).then(
                right
                    .1
                    .entry
                    .last_visit_at()
                    .cmp(&left.1.entry.last_visit_at()),
            )
        });
        scored
            .into_iter()
            .take(limit)
            .map(|(_, indexed)| indexed.entry.clone())
            .collect()
    }
}

/// The frecency score of `entry` evaluated at `now`.
fn score_at(entry: &SuggestionEntry, now: i64) -> i64 {
    frecency(
        entry.visit_count(),
        entry.typed_count(),
        entry.last_visit_at(),
        now,
    )
}

/// Lowercases `url` and drops its scheme, so `https://github.com/x` becomes
/// `github.com/x`. A string with no `://` is returned lowercased unchanged.
fn scheme_stripped(url: &str) -> String {
    let lowered = url.to_ascii_lowercase();
    match lowered.split_once("://") {
        Some((_, rest)) => rest.to_string(),
        None => lowered,
    }
}

/// Drops the query and fragment from a scheme-stripped URL, leaving `host + path`.
fn host_and_path(scheme_stripped_url: &str) -> String {
    let end = scheme_stripped_url
        .find(['?', '#'])
        .unwrap_or(scheme_stripped_url.len());
    scheme_stripped_url[..end].to_string()
}
