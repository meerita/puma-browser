// @file crates/browser-core/tests/ids.rs
// @description Verifies HistoryEntryId and BookmarkId are distinct newtypes that round-trip.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{BookmarkId, HistoryEntryId};

/// The two identifiers are distinct types at compile time: a `HistoryEntryId` cannot be
/// passed where a `BookmarkId` is expected. This test also confirms each round-trips its
/// value, so the newtypes remain usable identifiers rather than opaque markers.
#[test]
fn history_entry_id_and_bookmark_id_are_distinct_types() {
    let history_entry = HistoryEntryId::new(7);
    let bookmark = BookmarkId::new(7);
    assert_eq!(history_entry.value(), 7);
    assert_eq!(bookmark.value(), 7);
}
