// @file crates/browser-core/src/ids.rs
// @description Newtype identifiers for history entries and bookmarks.
// @layer core
// @created meerita <meerita@icloud.com>

/// Identifies a single entry in navigation history.
///
/// A newtype over `u64` so a history identifier can never be confused with a bookmark
/// identifier or another numeric value at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HistoryEntryId(u64);

impl HistoryEntryId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}

/// Identifies a single bookmark.
///
/// A distinct newtype from [`HistoryEntryId`] so the two identifier spaces cannot be
/// mixed at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BookmarkId(u64);

impl BookmarkId {
    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }
}
