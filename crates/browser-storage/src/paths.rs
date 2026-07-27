// @file crates/browser-storage/src/paths.rs
// @description Resolves the platform data directory and the on-disk history database path.
// @layer storage
// @created meerita <meerita@icloud.com>

use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::error::StorageError;

/// File name of the history database inside the platform data directory.
const DATABASE_FILE_NAME: &str = "history.sqlite3";

/// Returns the on-disk path for the history database, creating the containing directory
/// if it does not yet exist.
///
/// A `data_dir` override uses that directory; otherwise the platform data directory is
/// resolved. The database file name stays single-sourced here so no caller repeats it.
/// Any failure to locate or create the directory maps to `OpenFailed`, because from a
/// caller's point of view the database could not be opened.
pub fn history_database_path(data_dir: Option<&Path>) -> Result<PathBuf, StorageError> {
    match data_dir {
        Some(directory) => database_path_in(directory),
        None => default_database_path(),
    }
}

/// Returns the default on-disk path for the history database, creating the platform
/// data directory if it does not yet exist.
///
/// The `ProjectDirs` lookup and every filesystem call stay confined to this module so
/// the rest of the crate never touches platform path logic directly. Any failure to
/// locate or create the directory maps to `OpenFailed`, because from a caller's point
/// of view the database could not be opened.
pub fn default_database_path() -> Result<PathBuf, StorageError> {
    let project_dirs = ProjectDirs::from("", "", "puma").ok_or(StorageError::OpenFailed)?;
    database_path_in(project_dirs.data_dir())
}

/// Returns the database path inside `directory`, creating the directory if needed.
fn database_path_in(directory: &Path) -> Result<PathBuf, StorageError> {
    std::fs::create_dir_all(directory).map_err(|_| StorageError::OpenFailed)?;
    Ok(directory.join(DATABASE_FILE_NAME))
}
