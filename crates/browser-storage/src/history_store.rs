// @file crates/browser-storage/src/history_store.rs
// @description SQLite implementation of the history store: visit recording, reads, and pruning.
// @layer storage
// @created meerita <meerita@icloud.com>

use rusqlite::{params, OptionalExtension, Row};

use crate::database::SqliteStorage;
use crate::error::StorageError;
use crate::history_records::{HistoryEntry, NewVisit, SuggestionEntry};
use crate::stores::HistoryStore;

/// Reads a `SuggestionEntry` from a `pages` row selected as
/// `(url, host, visit_count, typed_count, last_visit_at)`.
fn suggestion_from_row(row: &Row<'_>) -> rusqlite::Result<SuggestionEntry> {
    Ok(SuggestionEntry::new(
        row.get(0)?,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
        row.get(4)?,
    ))
}

/// Reads a `HistoryEntry` from a joined row selected as
/// `(visits.id, pages.url, pages.title, visits.visited_at)`.
fn history_entry_from_row(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    let visit_id: i64 = row.get(0)?;
    Ok(HistoryEntry::new(
        visit_id as u64,
        row.get(1)?,
        row.get(2)?,
        row.get(3)?,
    ))
}

/// Escapes the `LIKE` metacharacters in a user query so they match literally under an
/// `ESCAPE '\'` clause. The backslash is escaped first so escapes introduced for `%` and
/// `_` are not themselves re-escaped.
fn escape_like_query(query: &str) -> String {
    query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

impl HistoryStore for SqliteStorage {
    fn record_visit(&self, visit: NewVisit) -> Result<SuggestionEntry, StorageError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        let transaction = connection
            .transaction()
            .map_err(|_| StorageError::QueryFailed)?;
        let typed_increment: i64 = if visit.was_typed() { 1 } else { 0 };
        transaction
            .execute(
                "INSERT INTO pages \
                   (url, host, title, visit_count, typed_count, first_visit_at, last_visit_at) \
                 VALUES (?1, ?2, ?3, 1, ?4, ?5, ?5) \
                 ON CONFLICT(url) DO UPDATE SET \
                   visit_count = visit_count + 1, \
                   typed_count = typed_count + excluded.typed_count, \
                   last_visit_at = excluded.last_visit_at, \
                   title = COALESCE(excluded.title, pages.title)",
                params![
                    visit.url(),
                    visit.host(),
                    visit.title(),
                    typed_increment,
                    visit.visited_at()
                ],
            )
            .map_err(|_| StorageError::QueryFailed)?;
        let page_id: i64 = transaction
            .query_row(
                "SELECT id FROM pages WHERE url = ?1",
                params![visit.url()],
                |row| row.get(0),
            )
            .map_err(|_| StorageError::QueryFailed)?;
        transaction
            .execute(
                "INSERT INTO visits (page_id, visited_at, was_typed) VALUES (?1, ?2, ?3)",
                params![page_id, visit.visited_at(), typed_increment],
            )
            .map_err(|_| StorageError::QueryFailed)?;
        let suggestion = transaction
            .query_row(
                "SELECT url, host, visit_count, typed_count, last_visit_at \
                 FROM pages WHERE id = ?1",
                params![page_id],
                suggestion_from_row,
            )
            .map_err(|_| StorageError::QueryFailed)?;
        transaction
            .commit()
            .map_err(|_| StorageError::QueryFailed)?;
        Ok(suggestion)
    }

    fn recent_entries(&self, limit: usize) -> Result<Vec<HistoryEntry>, StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        let mut statement = connection
            .prepare(
                "SELECT visits.id, pages.url, pages.title, visits.visited_at \
                 FROM visits JOIN pages ON pages.id = visits.page_id \
                 ORDER BY visits.visited_at DESC, visits.id DESC \
                 LIMIT ?1",
            )
            .map_err(|_| StorageError::QueryFailed)?;
        let entries = statement
            .query_map(params![limit as i64], history_entry_from_row)
            .map_err(|_| StorageError::QueryFailed)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| StorageError::QueryFailed)?;
        Ok(entries)
    }

    fn search_entries(&self, query: &str, limit: usize) -> Result<Vec<HistoryEntry>, StorageError> {
        let pattern = format!("%{}%", escape_like_query(query));
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        let mut statement = connection
            .prepare(
                "SELECT visits.id, pages.url, pages.title, visits.visited_at \
                 FROM visits JOIN pages ON pages.id = visits.page_id \
                 WHERE pages.url LIKE ?1 ESCAPE '\\' OR pages.title LIKE ?1 ESCAPE '\\' \
                 ORDER BY visits.visited_at DESC, visits.id DESC \
                 LIMIT ?2",
            )
            .map_err(|_| StorageError::QueryFailed)?;
        let entries = statement
            .query_map(params![pattern, limit as i64], history_entry_from_row)
            .map_err(|_| StorageError::QueryFailed)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| StorageError::QueryFailed)?;
        Ok(entries)
    }

    fn load_suggestions(&self) -> Result<Vec<SuggestionEntry>, StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        let mut statement = connection
            .prepare("SELECT url, host, visit_count, typed_count, last_visit_at FROM pages")
            .map_err(|_| StorageError::QueryFailed)?;
        let suggestions = statement
            .query_map([], suggestion_from_row)
            .map_err(|_| StorageError::QueryFailed)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(|_| StorageError::QueryFailed)?;
        Ok(suggestions)
    }

    fn remove_entry(&self, id: u64) -> Result<(), StorageError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        let transaction = connection
            .transaction()
            .map_err(|_| StorageError::QueryFailed)?;
        let page_id: Option<i64> = transaction
            .query_row(
                "SELECT page_id FROM visits WHERE id = ?1",
                params![id as i64],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| StorageError::QueryFailed)?;
        let Some(page_id) = page_id else {
            return Ok(());
        };
        transaction
            .execute("DELETE FROM visits WHERE id = ?1", params![id as i64])
            .map_err(|_| StorageError::QueryFailed)?;
        transaction
            .execute(
                "DELETE FROM pages WHERE id = ?1 \
                 AND NOT EXISTS (SELECT 1 FROM visits WHERE visits.page_id = ?1)",
                params![page_id],
            )
            .map_err(|_| StorageError::QueryFailed)?;
        transaction
            .commit()
            .map_err(|_| StorageError::QueryFailed)?;
        Ok(())
    }

    fn clear_all(&self) -> Result<(), StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        connection
            .execute_batch("DELETE FROM visits; DELETE FROM pages;")
            .map_err(|_| StorageError::QueryFailed)
    }

    fn clear_site(&self, host: &str) -> Result<(), StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        connection
            .execute("DELETE FROM pages WHERE host = ?1", params![host])
            .map(|_| ())
            .map_err(|_| StorageError::QueryFailed)
    }

    fn prune_older_than(&self, cutoff: i64) -> Result<(), StorageError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        let transaction = connection
            .transaction()
            .map_err(|_| StorageError::QueryFailed)?;
        transaction
            .execute("DELETE FROM visits WHERE visited_at < ?1", params![cutoff])
            .map_err(|_| StorageError::QueryFailed)?;
        transaction
            .execute(
                "DELETE FROM pages \
                 WHERE NOT EXISTS (SELECT 1 FROM visits WHERE visits.page_id = pages.id)",
                [],
            )
            .map_err(|_| StorageError::QueryFailed)?;
        transaction
            .commit()
            .map_err(|_| StorageError::QueryFailed)?;
        Ok(())
    }
}
