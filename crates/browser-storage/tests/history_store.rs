// @file crates/browser-storage/tests/history_store.rs
// @description Verifies the SQLite history store: recording, reads, search, removal, and pruning.
// @layer storage
// @created meerita <meerita@icloud.com>

use browser_storage::{HistoryStore, NewVisit, SqliteStorage};

/// Opens a prepared in-memory database for a test, panicking with a clear message if the
/// substrate itself fails to open.
fn open() -> SqliteStorage {
    SqliteStorage::open_in_memory().expect("in-memory database must open and migrate")
}

/// Builds a followed-link visit to `url` on `host` at `visited_at`, with no title.
fn visit(url: &str, host: &str, visited_at: i64) -> NewVisit {
    NewVisit::new(url.to_string(), host.to_string(), None, false, visited_at)
}

/// Builds a followed-link visit carrying a page `title`.
fn visit_with_title(url: &str, host: &str, title: &str, visited_at: i64) -> NewVisit {
    NewVisit::new(
        url.to_string(),
        host.to_string(),
        Some(title.to_string()),
        false,
        visited_at,
    )
}

/// Builds a typed visit to `url` on `host` at `visited_at`, with no title.
fn typed_visit(url: &str, host: &str, visited_at: i64) -> NewVisit {
    NewVisit::new(url.to_string(), host.to_string(), None, true, visited_at)
}

#[test]
fn recording_a_new_url_creates_one_page_and_one_visit() {
    let storage = open();
    let suggestion = storage
        .record_visit(visit("https://example.com/", "example.com", 100))
        .expect("recording a visit must succeed");
    assert_eq!(suggestion.visit_count(), 1);
    assert_eq!(suggestion.typed_count(), 0);
    assert_eq!(suggestion.last_visit_at(), 100);
    assert_eq!(suggestion.url(), "https://example.com/");
    assert_eq!(
        storage
            .load_suggestions()
            .expect("loading suggestions must succeed")
            .len(),
        1
    );
    assert_eq!(
        storage
            .recent_entries(10)
            .expect("recent entries must read")
            .len(),
        1
    );
}

#[test]
fn recording_the_same_url_again_increments_the_visit_count() {
    let storage = open();
    storage
        .record_visit(visit("https://example.com/", "example.com", 100))
        .expect("first visit must record");
    let suggestion = storage
        .record_visit(visit("https://example.com/", "example.com", 200))
        .expect("second visit must record");
    assert_eq!(suggestion.visit_count(), 2);
    assert_eq!(suggestion.last_visit_at(), 200);
    assert_eq!(
        storage
            .load_suggestions()
            .expect("loading suggestions must succeed")
            .len(),
        1,
        "the same URL must not create a second page"
    );
}

#[test]
fn a_typed_visit_sets_and_increments_the_typed_count() {
    let storage = open();
    let first = storage
        .record_visit(typed_visit("https://example.com/", "example.com", 100))
        .expect("first typed visit must record");
    assert_eq!(first.typed_count(), 1);
    let second = storage
        .record_visit(typed_visit("https://example.com/", "example.com", 200))
        .expect("second typed visit must record");
    assert_eq!(second.typed_count(), 2);
    let followed = storage
        .record_visit(visit("https://example.com/", "example.com", 300))
        .expect("followed visit must record");
    assert_eq!(
        followed.typed_count(),
        2,
        "a followed-link visit must not change the typed count"
    );
}

#[test]
fn recent_entries_returns_most_recent_first_and_respects_the_limit() {
    let storage = open();
    storage
        .record_visit(visit("https://a.com/", "a.com", 100))
        .expect("visit must record");
    storage
        .record_visit(visit("https://b.com/", "b.com", 200))
        .expect("visit must record");
    storage
        .record_visit(visit("https://c.com/", "c.com", 300))
        .expect("visit must record");
    let entries = storage.recent_entries(2).expect("recent entries must read");
    assert_eq!(entries.len(), 2, "the limit must bound the result");
    assert_eq!(entries[0].url(), "https://c.com/");
    assert_eq!(entries[1].url(), "https://b.com/");
}

#[test]
fn search_matches_on_the_url() {
    let storage = open();
    storage
        .record_visit(visit("https://github.com/rust", "github.com", 100))
        .expect("visit must record");
    storage
        .record_visit(visit("https://example.com/", "example.com", 200))
        .expect("visit must record");
    let matches = storage
        .search_entries("github", 10)
        .expect("search must read");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].url(), "https://github.com/rust");
}

#[test]
fn search_matches_on_the_title() {
    let storage = open();
    storage
        .record_visit(visit_with_title(
            "https://example.com/",
            "example.com",
            "The Rust Programming Language",
            100,
        ))
        .expect("visit must record");
    let matches = storage
        .search_entries("Programming", 10)
        .expect("search must read");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].title(), Some("The Rust Programming Language"));
}

#[test]
fn search_ignores_non_matching_entries() {
    let storage = open();
    storage
        .record_visit(visit("https://example.com/", "example.com", 100))
        .expect("visit must record");
    let matches = storage
        .search_entries("nonexistent", 10)
        .expect("search must read");
    assert!(matches.is_empty());
}

#[test]
fn search_treats_wildcard_input_literally() {
    let storage = open();
    storage
        .record_visit(visit("https://example.com/", "example.com", 100))
        .expect("visit must record");
    let percent = storage.search_entries("%", 10).expect("search must read");
    assert!(
        percent.is_empty(),
        "an unescaped % would match every row; escaped it matches only a literal %"
    );
    let underscore = storage.search_entries("_", 10).expect("search must read");
    assert!(
        underscore.is_empty(),
        "an unescaped _ would match any single character; escaped it matches only a literal _"
    );
}

#[test]
fn load_suggestions_returns_every_page() {
    let storage = open();
    storage
        .record_visit(visit("https://a.com/", "a.com", 100))
        .expect("visit must record");
    storage
        .record_visit(visit("https://b.com/", "b.com", 200))
        .expect("visit must record");
    let suggestions = storage
        .load_suggestions()
        .expect("loading suggestions must succeed");
    assert_eq!(suggestions.len(), 2);
}

#[test]
fn remove_entry_deletes_the_visit_and_its_orphaned_page() {
    let storage = open();
    storage
        .record_visit(visit("https://example.com/", "example.com", 100))
        .expect("visit must record");
    let entries = storage
        .recent_entries(10)
        .expect("recent entries must read");
    let only_visit_id = entries[0].id();
    storage
        .remove_entry(only_visit_id)
        .expect("removal must succeed");
    assert!(
        storage
            .recent_entries(10)
            .expect("recent entries must read")
            .is_empty(),
        "the visit must be gone"
    );
    assert!(
        storage
            .load_suggestions()
            .expect("loading suggestions must succeed")
            .is_empty(),
        "the page must be removed once it has no remaining visit"
    );
}

#[test]
fn remove_entry_keeps_a_page_that_still_has_visits() {
    let storage = open();
    storage
        .record_visit(visit("https://example.com/", "example.com", 100))
        .expect("visit must record");
    storage
        .record_visit(visit("https://example.com/", "example.com", 200))
        .expect("visit must record");
    let entries = storage
        .recent_entries(10)
        .expect("recent entries must read");
    storage
        .remove_entry(entries[0].id())
        .expect("removal must succeed");
    assert_eq!(
        storage
            .recent_entries(10)
            .expect("recent entries must read")
            .len(),
        1,
        "one visit must remain"
    );
    assert_eq!(
        storage
            .load_suggestions()
            .expect("loading suggestions must succeed")
            .len(),
        1,
        "the page must survive while a visit remains"
    );
}

#[test]
fn remove_entry_with_an_unknown_id_is_a_no_op() {
    let storage = open();
    storage
        .record_visit(visit("https://example.com/", "example.com", 100))
        .expect("visit must record");
    storage.remove_entry(9999).expect("removal must succeed");
    assert_eq!(
        storage
            .recent_entries(10)
            .expect("recent entries must read")
            .len(),
        1
    );
}

#[test]
fn clear_site_removes_only_that_host_and_cascades() {
    let storage = open();
    storage
        .record_visit(visit("https://keep.com/", "keep.com", 100))
        .expect("visit must record");
    storage
        .record_visit(visit("https://drop.com/a", "drop.com", 200))
        .expect("visit must record");
    storage
        .record_visit(visit("https://drop.com/b", "drop.com", 300))
        .expect("visit must record");
    storage.clear_site("drop.com").expect("clear must succeed");
    let suggestions = storage
        .load_suggestions()
        .expect("loading suggestions must succeed");
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].host(), "keep.com");
    assert_eq!(
        storage
            .recent_entries(10)
            .expect("recent entries must read")
            .len(),
        1,
        "the cleared host's visits must cascade away"
    );
}

#[test]
fn clear_all_empties_both_tables() {
    let storage = open();
    storage
        .record_visit(visit("https://a.com/", "a.com", 100))
        .expect("visit must record");
    storage
        .record_visit(visit("https://b.com/", "b.com", 200))
        .expect("visit must record");
    storage.clear_all().expect("clear must succeed");
    assert!(storage
        .load_suggestions()
        .expect("loading suggestions must succeed")
        .is_empty());
    assert!(storage
        .recent_entries(10)
        .expect("recent entries must read")
        .is_empty());
}

#[test]
fn prune_older_than_deletes_old_visits_and_orphaned_pages_but_keeps_recent_ones() {
    let storage = open();
    storage
        .record_visit(visit("https://old.com/", "old.com", 100))
        .expect("visit must record");
    storage
        .record_visit(visit("https://new.com/", "new.com", 1000))
        .expect("visit must record");
    storage.prune_older_than(500).expect("prune must succeed");
    let suggestions = storage
        .load_suggestions()
        .expect("loading suggestions must succeed");
    assert_eq!(suggestions.len(), 1, "the old orphaned page must be pruned");
    assert_eq!(suggestions[0].host(), "new.com");
    let entries = storage
        .recent_entries(10)
        .expect("recent entries must read");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].url(), "https://new.com/");
}
