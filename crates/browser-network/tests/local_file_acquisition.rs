// @file crates/browser-network/tests/local_file_acquisition.rs
// @description Behavior tests for file:// acquisition: URL construction, path round-trip, reads, errors.
// @layer network
// @created meerita <meerita@icloud.com>

use std::path::Path;

use browser_network::{fetch, BrowserUrl, NetworkError};
use tempfile::tempdir;

const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

fn file_url(path: &Path) -> BrowserUrl {
    BrowserUrl::from_file_path(path).expect("absolute path must yield a file URL")
}

#[test]
fn from_file_path_with_absolute_path_yields_file_scheme() {
    let directory = tempdir().expect("temp directory must be created");
    let path = directory.path().join("example.html");
    let url = BrowserUrl::from_file_path(&path).expect("absolute path must construct a file URL");
    assert_eq!(url.scheme(), "file");
}

#[test]
fn from_file_path_with_relative_path_returns_invalid_url() {
    let outcome = BrowserUrl::from_file_path(Path::new("relative/example.html"));
    assert!(matches!(outcome, Err(NetworkError::InvalidUrl)));
}

#[test]
fn path_buf_round_trips_the_absolute_path() {
    let directory = tempdir().expect("temp directory must be created");
    let original = directory.path().join("round-trip.html");
    let url =
        BrowserUrl::from_file_path(&original).expect("absolute path must construct a file URL");
    assert_eq!(url.path_buf().as_deref(), Some(original.as_path()));
}

#[tokio::test]
async fn html_file_is_read_as_text_html_with_no_charset() {
    let directory = tempdir().expect("temp directory must be created");
    let path = directory.path().join("page.html");
    let markup = b"<html><body>local</body></html>";
    tokio::fs::write(&path, markup)
        .await
        .expect("temp file must be written");

    let document = fetch(&file_url(&path))
        .await
        .expect("an existing HTML file must be read");

    assert_eq!(document.body_bytes(), markup);
    assert_eq!(document.content_type(), "text/html");
    assert_eq!(document.charset(), None);
}

#[tokio::test]
async fn htm_and_xhtml_extensions_are_treated_as_html() {
    let directory = tempdir().expect("temp directory must be created");
    for name in ["page.htm", "page.xhtml", "PAGE.HTML"] {
        let path = directory.path().join(name);
        tokio::fs::write(&path, b"<html></html>")
            .await
            .expect("temp file must be written");
        let document = fetch(&file_url(&path))
            .await
            .expect("an existing HTML file must be read");
        assert_eq!(document.content_type(), "text/html");
    }
}

#[tokio::test]
async fn non_html_extension_is_read_as_text_plain() {
    let directory = tempdir().expect("temp directory must be created");
    let path = directory.path().join("notes.txt");
    tokio::fs::write(&path, b"just text")
        .await
        .expect("temp file must be written");

    let document = fetch(&file_url(&path))
        .await
        .expect("an existing text file must be read");

    assert_eq!(document.content_type(), "text/plain");
}

#[tokio::test]
async fn a_file_with_no_extension_is_read_as_text_plain() {
    let directory = tempdir().expect("temp directory must be created");
    let path = directory.path().join("README");
    tokio::fs::write(&path, b"contents")
        .await
        .expect("temp file must be written");

    let document = fetch(&file_url(&path))
        .await
        .expect("an existing extensionless file must be read");

    assert_eq!(document.content_type(), "text/plain");
}

#[tokio::test]
async fn a_missing_file_returns_file_not_found() {
    let directory = tempdir().expect("temp directory must be created");
    let path = directory.path().join("does-not-exist.html");

    let outcome = fetch(&file_url(&path)).await;

    assert!(matches!(outcome, Err(NetworkError::FileNotFound)));
}

#[tokio::test]
async fn a_directory_returns_path_is_directory() {
    let directory = tempdir().expect("temp directory must be created");

    let outcome = fetch(&file_url(directory.path())).await;

    assert!(matches!(outcome, Err(NetworkError::PathIsDirectory)));
}

#[tokio::test]
async fn a_file_larger_than_the_limit_returns_file_too_large() {
    let directory = tempdir().expect("temp directory must be created");
    let path = directory.path().join("oversized.html");
    let oversized = vec![b'a'; MAX_RESPONSE_BYTES + 1];
    tokio::fs::write(&path, oversized)
        .await
        .expect("oversized temp file must be written");

    let outcome = fetch(&file_url(&path)).await;

    assert!(matches!(outcome, Err(NetworkError::FileTooLarge)));
}
