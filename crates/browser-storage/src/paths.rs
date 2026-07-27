// @file crates/browser-storage/src/paths.rs
// @description Resolves the platform data directory and the on-disk history database path.
// @layer storage
// @created meerita <meerita@icloud.com>

use std::path::PathBuf;

use directories::ProjectDirs;

use crate::error::StorageError;

/// File name of the history database inside the platform data directory.
const DATABASE_FILE_NAME: &str = "history.sqlite3";

/// Returns the default on-disk path for the history database, creating the platform
/// data directory if it does not yet exist.
///
/// The `ProjectDirs` lookup and every filesystem call stay confined to this module so
/// the rest of the crate never touches platform path logic directly. Any failure to
/// locate or create the directory maps to `OpenFailed`, because from a caller's point
/// of view the database could not be opened.
pub fn default_database_path() -> Result<PathBuf, StorageError> {
    let project_dirs = ProjectDirs::from("", "", "puma").ok_or(StorageError::OpenFailed)?;
    let data_dir = project_dirs.data_dir();
    std::fs::create_dir_all(data_dir).map_err(|_| StorageError::OpenFailed)?;
    Ok(data_dir.join(DATABASE_FILE_NAME))
}
