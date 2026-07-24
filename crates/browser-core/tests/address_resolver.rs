// @file crates/browser-core/tests/address_resolver.rs
// @description Integration tests for resolve_address: every D2 precedence branch and each error variant.
// @layer core
// @created meerita <meerita@icloud.com>

use std::fs;
use std::path::Path;

use browser_core::{resolve_address, CoreError};
use tempfile::tempdir;

fn write_file(directory: &Path, name: &str) -> std::path::PathBuf {
    let path = directory.join(name);
    fs::write(&path, b"<html><body>local</body></html>").expect("temp file must be writable");
    path
}

#[test]
fn explicit_file_url_resolves_to_file_scheme() {
    let working_directory = tempdir().expect("temp working directory must be created");
    let file_path = write_file(working_directory.path(), "page.html");
    let input = format!("file://{}", file_path.display());

    let resolved = resolve_address(&input, working_directory.path())
        .expect("an explicit file:// URL must resolve");

    assert_eq!(resolved.scheme(), "file");
}

#[test]
fn absolute_path_to_existing_file_resolves_to_file_scheme() {
    let working_directory = tempdir().expect("temp working directory must be created");
    let file_path = write_file(working_directory.path(), "page.html");

    let resolved = resolve_address(&file_path.display().to_string(), working_directory.path())
        .expect("an absolute path to a file must resolve");

    assert_eq!(resolved.scheme(), "file");
    assert_eq!(resolved.path_buf().as_deref(), Some(file_path.as_path()));
}

#[test]
fn relative_dot_path_to_existing_file_resolves_to_file_scheme() {
    let working_directory = tempdir().expect("temp working directory must be created");
    write_file(working_directory.path(), "page.html");

    let resolved = resolve_address("./page.html", working_directory.path())
        .expect("an existing ./ path must resolve");

    assert_eq!(resolved.scheme(), "file");
}

#[test]
fn relative_dotdot_path_to_existing_file_resolves_to_file_scheme() {
    let parent = tempdir().expect("temp parent directory must be created");
    write_file(parent.path(), "page.html");
    let working_directory = parent.path().join("nested");
    fs::create_dir(&working_directory).expect("nested working directory must be created");

    let resolved = resolve_address("../page.html", &working_directory)
        .expect("an existing ../ path must resolve");

    assert_eq!(resolved.scheme(), "file");
}

#[test]
fn relative_path_to_missing_file_returns_local_file_not_found() {
    let working_directory = tempdir().expect("temp working directory must be created");

    let error = resolve_address("./missing.html", working_directory.path())
        .expect_err("a missing local path must fail");

    assert!(matches!(error, CoreError::LocalFileNotFound));
}

#[test]
fn absolute_path_to_directory_returns_local_path_is_directory() {
    let working_directory = tempdir().expect("temp working directory must be created");
    let directory_path = working_directory.path().join("subdir");
    fs::create_dir(&directory_path).expect("subdirectory must be created");

    let error = resolve_address(
        &directory_path.display().to_string(),
        working_directory.path(),
    )
    .expect_err("a path pointing at a directory must fail");

    assert!(matches!(error, CoreError::LocalPathIsDirectory));
}

#[test]
fn bare_token_matching_existing_file_resolves_to_file_scheme() {
    let working_directory = tempdir().expect("temp working directory must be created");
    write_file(working_directory.path(), "index.html");

    let resolved = resolve_address("index.html", working_directory.path())
        .expect("a bare token naming an existing file must resolve");

    assert_eq!(resolved.scheme(), "file");
}

#[test]
fn bare_token_without_matching_file_resolves_to_web_address() {
    let working_directory = tempdir().expect("temp working directory must be created");

    let resolved = resolve_address("example.com", working_directory.path())
        .expect("a bare token with no local file must resolve as a web address");

    assert_eq!(resolved.scheme(), "https");
    assert_eq!(resolved.host_str(), Some("example.com"));
}

#[test]
fn bare_token_matching_a_directory_resolves_to_web_address() {
    let working_directory = tempdir().expect("temp working directory must be created");
    fs::create_dir(working_directory.path().join("docs")).expect("directory must be created");

    let resolved = resolve_address("docs", working_directory.path())
        .expect("a bare token matching a directory falls through to a web address");

    assert_eq!(resolved.scheme(), "https");
}

#[test]
fn home_prefixed_path_expands_against_the_home_directory() {
    let Some(home) = dirs::home_dir() else {
        // No home directory on this runner; the expansion branch cannot be exercised.
        return;
    };
    let temp_file = tempfile::Builder::new()
        .prefix("puma-address-resolver-")
        .suffix(".html")
        .tempfile_in(&home)
        .expect("temp file in home must be created");
    let file_name = temp_file
        .path()
        .file_name()
        .expect("temp file must have a name")
        .to_string_lossy()
        .into_owned();
    let working_directory = tempdir().expect("temp working directory must be created");

    let resolved = resolve_address(&format!("~/{file_name}"), working_directory.path())
        .expect("a ~/ path to an existing home file must resolve");

    assert_eq!(resolved.scheme(), "file");
    assert_eq!(
        resolved.path_buf().as_deref(),
        Some(home.join(&file_name).as_path())
    );
}
