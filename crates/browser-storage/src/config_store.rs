// @file crates/browser-storage/src/config_store.rs
// @description SQLite implementation of the key-value configuration store.
// @layer storage
// @created meerita <meerita@icloud.com>

use rusqlite::{params, OptionalExtension};

use crate::database::SqliteStorage;
use crate::error::StorageError;
use crate::stores::ConfigStore;

impl ConfigStore for SqliteStorage {
    fn config_value(&self, key: &str) -> Result<Option<String>, StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        connection
            .query_row(
                "SELECT value FROM config WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|_| StorageError::QueryFailed)
    }

    fn set_config_value(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| StorageError::QueryFailed)?;
        // The trait signature carries no timestamp and the store never reads the clock,
        // so `updated_at` is written as a fixed 0; the column exists for a future caller
        // that supplies a real time without reshaping the table.
        connection
            .execute(
                "INSERT INTO config (key, value, updated_at) \
                 VALUES (?1, ?2, 0) \
                 ON CONFLICT(key) DO UPDATE SET \
                   value = excluded.value, \
                   updated_at = excluded.updated_at",
                params![key, value],
            )
            .map(|_| ())
            .map_err(|_| StorageError::QueryFailed)
    }
}
