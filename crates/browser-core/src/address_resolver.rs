// @file crates/browser-core/src/address_resolver.rs
// @description Resolves a typed CLI argument to a BrowserUrl: file:// URLs, local paths, web addresses.
// @layer core
// @created meerita <meerita@icloud.com>

use std::path::{Path, PathBuf};

use browser_network::BrowserUrl;

use crate::error::CoreError;

/// An explicitly typed `file://` URL is taken as written.
const FILE_URL_PREFIX: &str = "file://";

/// A leading `~/` expands against the user's home directory.
const HOME_PREFIX: &str = "~/";

/// Turn a user-typed argument into a validated [`BrowserUrl`].
///
/// Resolution follows a fixed precedence:
///
/// 1. input starting with `file://` is parsed as a URL as written;
/// 2. input starting with `~/` expands against the home directory, then resolves as a
///    local path;
/// 3. input starting with `/`, `./`, or `../` resolves as a local path;
/// 4. a bare token naming a file that exists under `working_directory` resolves as a
///    local path;
/// 5. anything else is a web address, keeping the schemeless `https` default.
///
/// A local path must exist and be a file; a missing path yields
/// [`CoreError::LocalFileNotFound`] and a directory yields
/// [`CoreError::LocalPathIsDirectory`]. The only filesystem access is a single metadata
/// probe per candidate; the resolver never reads file contents and never touches the
/// network.
pub fn resolve_address(input: &str, working_directory: &Path) -> Result<BrowserUrl, CoreError> {
    let trimmed = input.trim();

    if trimmed.starts_with(FILE_URL_PREFIX) {
        return Ok(BrowserUrl::parse(trimmed)?);
    }

    if let Some(expanded) = expand_home(trimmed) {
        return resolve_local_path(&expanded, working_directory);
    }

    if has_path_marker(trimmed) {
        return resolve_local_path(Path::new(trimmed), working_directory);
    }

    if bare_token_names_existing_file(trimmed, working_directory) {
        return resolve_local_path(Path::new(trimmed), working_directory);
    }

    Ok(BrowserUrl::parse(trimmed)?)
}

/// Whether `input` begins with a marker that unambiguously names a local path.
fn has_path_marker(input: &str) -> bool {
    input.starts_with('/') || input.starts_with("./") || input.starts_with("../")
}

/// Expand a leading `~/` against the home directory, returning the joined path.
///
/// Returns `None` when `input` is not home-relative or when no home directory is
/// available, in which case the caller falls through to the remaining precedence rules.
fn expand_home(input: &str) -> Option<PathBuf> {
    let remainder = input.strip_prefix(HOME_PREFIX)?;
    let home = dirs::home_dir()?;
    Some(home.join(remainder))
}

/// Whether a bare token names a file that already exists under the working directory.
///
/// A directory match is deliberately not treated as a local file: a bare token that is
/// not an existing file falls through to the web-address rule.
fn bare_token_names_existing_file(input: &str, working_directory: &Path) -> bool {
    working_directory.join(input).is_file()
}

/// Absolutize `path` against `working_directory`, then require it to be an existing file.
fn resolve_local_path(path: &Path, working_directory: &Path) -> Result<BrowserUrl, CoreError> {
    let absolute = absolutize(path, working_directory);
    let Ok(metadata) = std::fs::metadata(&absolute) else {
        return Err(CoreError::LocalFileNotFound);
    };
    if metadata.is_dir() {
        return Err(CoreError::LocalPathIsDirectory);
    }
    Ok(BrowserUrl::from_file_path(&absolute)?)
}

/// Join a relative path onto the working directory; leave an absolute path untouched.
fn absolutize(path: &Path, working_directory: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }
    working_directory.join(path)
}
