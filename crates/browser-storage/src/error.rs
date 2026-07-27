// @file crates/browser-storage/src/error.rs
// @description Storage-layer error taxonomy; maps SQLite failures without leaking driver types.
// @layer storage
// @created meerita <meerita@icloud.com>

use thiserror::Error;

/// Errors produced by the storage layer.
///
/// Raw `rusqlite` errors and SQL text never appear in any variant. A failure that
/// carries detail stores a crate-local message string produced at the boundary, so
/// driver internals and SQL never cross outward to callers.
#[derive(Debug, Error)]
pub enum StorageError {
    #[error("failed to open the storage database")]
    OpenFailed,

    #[error("database migration failed")]
    MigrationFailed,

    #[error("storage query failed")]
    QueryFailed,

    #[error("requested record was not found")]
    NotFound,

    #[error("configuration file is invalid")]
    ConfigInvalid,
}
