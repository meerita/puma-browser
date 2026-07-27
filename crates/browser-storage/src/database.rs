// @file crates/browser-storage/src/database.rs
// @description SQLite connection lifecycle: opens a WAL, foreign-key-enforcing, migrated database.
// @layer storage
// @created meerita <meerita@icloud.com>

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::StorageError;
use crate::migrations::run_migrations;

/// The storage backend: a single SQLite connection shared behind a mutex.
///
/// A single-process TUI has one writer, so one connection guarded by a `Mutex` is
/// enough; a pool would add coordination for a concurrency this design never has. The
/// connection is wrapped in an `Arc` so the async layer above can share it across tasks.
#[derive(Clone)]
pub struct SqliteStorage {
    pub(crate) connection: Arc<Mutex<Connection>>,
}

impl SqliteStorage {
    /// Opens the database file at `path`, preparing it for use: WAL journalling, foreign
    /// keys enforced, and all pending migrations applied.
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let connection = Connection::open(path).map_err(|_| StorageError::OpenFailed)?;
        Self::prepare(connection)
    }

    /// Opens an in-memory database prepared the same way as [`open`]. The database is
    /// discarded when the process exits, so history recorded here never reaches disk.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let connection = Connection::open_in_memory().map_err(|_| StorageError::OpenFailed)?;
        Self::prepare(connection)
    }

    /// Returns the schema version the connection has been migrated to, read from
    /// `PRAGMA user_version`. Callers and diagnostics use it to confirm the database
    /// reached the expected schema.
    pub fn schema_version(&self) -> Result<i64, StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        connection
            .query_row("PRAGMA user_version;", [], |row| row.get(0))
            .map_err(|_| StorageError::QueryFailed)
    }

    /// Applies the connection pragmas and migrations shared by every open path, then
    /// wraps the ready connection.
    fn prepare(connection: Connection) -> Result<Self, StorageError> {
        apply_connection_pragmas(&connection)?;
        run_migrations(&connection)?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }
}

/// Enables write-ahead logging for concurrent reads during a write, and turns on
/// foreign-key enforcement, which SQLite leaves off per connection by default.
fn apply_connection_pragmas(connection: &Connection) -> Result<(), StorageError> {
    connection
        .pragma_update(None, "journal_mode", "WAL")
        .map_err(|_| StorageError::OpenFailed)?;
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .map_err(|_| StorageError::OpenFailed)
}

#[cfg(test)]
#[path = "database_tests.rs"]
mod tests;
