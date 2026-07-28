// @file crates/browser-storage/tests/config_loading.rs
// @description Verifies TOML config loading overlays the file onto defaults and rejects malformed input.
// @layer storage
// @created meerita <meerita@icloud.com>

use std::path::{Path, PathBuf};

use browser_storage::{load_config, BrowserConfig, StorageError};
use tempfile::tempdir;

/// Writes `contents` to a `config.toml` inside a fresh temporary directory and returns
/// the path, keeping the directory alive for the duration of the returned guard.
fn write_config(contents: &str) -> (tempfile::TempDir, PathBuf) {
    let directory = tempdir().expect("a temporary directory must be created");
    let path = directory.path().join("config.toml");
    std::fs::write(&path, contents).expect("the config file must be written");
    (directory, path)
}

#[test]
fn a_full_history_section_parses_to_the_exact_values() {
    let (_directory, path) = write_config(
        "[history]\nmode = \"in-memory\"\nretention_days = 30\nstore_titles = false\n",
    );
    let config = load_config(&path).expect("a well-formed file must load");
    assert_eq!(config.history_mode(), "in-memory");
    assert_eq!(config.retention_days(), 30);
    assert!(!config.store_titles());
    assert_eq!(config.data_dir(), None);
}

#[test]
fn a_partial_file_fills_the_rest_from_defaults() {
    let (_directory, path) = write_config("[history]\nretention_days = 7\n");
    let config = load_config(&path).expect("a partial file must load");
    let defaults = BrowserConfig::default();
    assert_eq!(config.retention_days(), 7);
    assert_eq!(config.history_mode(), defaults.history_mode());
    assert_eq!(config.store_titles(), defaults.store_titles());
}

#[test]
fn a_missing_file_returns_the_defaults() {
    let directory = tempdir().expect("a temporary directory must be created");
    let path = directory.path().join("does-not-exist.toml");
    let config = load_config(&path).expect("a missing file must resolve to defaults");
    assert_eq!(config, BrowserConfig::default());
}

#[test]
fn an_unknown_key_is_accepted_and_ignored() {
    let (_directory, path) =
        write_config("unknown_top = 1\n[history]\nmode = \"disabled\"\nfuture_key = true\n");
    let config = load_config(&path).expect("an unknown key must not fail the load");
    assert_eq!(config.history_mode(), "disabled");
}

#[test]
fn a_malformed_file_returns_config_invalid() {
    let (_directory, path) = write_config("this is = not valid toml [[[\n");
    let error = load_config(&path).expect_err("malformed TOML must fail to load");
    assert!(matches!(error, StorageError::ConfigInvalid));
}

#[test]
fn a_malformed_file_error_leaks_neither_path_nor_contents() {
    let (_directory, path) = write_config("garbage ]]] value\n");
    let error = load_config(&path).expect_err("malformed TOML must fail to load");
    let message = error.to_string();
    assert!(!message.contains("garbage"));
    assert!(!message.contains(&path.to_string_lossy().to_string()));
}

#[test]
fn a_top_level_data_dir_parses_to_some_path() {
    let (_directory, path) = write_config("data_dir = \"/opt/puma/data\"\n");
    let config = load_config(&path).expect("a data_dir file must load");
    assert_eq!(config.data_dir(), Some(Path::new("/opt/puma/data")));
}

#[test]
fn cookie_policies_default_to_reject_when_the_section_is_absent() {
    let (_directory, path) = write_config("[history]\nmode = \"persistent\"\n");
    let config = load_config(&path).expect("a file without cookies must load");
    assert_eq!(config.cookie_first_party(), "reject");
    assert_eq!(config.cookie_third_party(), "reject");
}

#[test]
fn a_partial_cookies_section_fills_the_missing_scope_from_the_default() {
    let (_directory, path) = write_config("[cookies]\nfirst_party = \"session\"\n");
    let config = load_config(&path).expect("a partial cookies section must load");
    assert_eq!(config.cookie_first_party(), "session");
    assert_eq!(
        config.cookie_third_party(),
        "reject",
        "the unset scope must keep the reject default"
    );
}

#[test]
fn a_full_cookies_section_parses_both_scopes() {
    let (_directory, path) =
        write_config("[cookies]\nfirst_party = \"allow\"\nthird_party = \"session\"\n");
    let config = load_config(&path).expect("a full cookies section must load");
    assert_eq!(config.cookie_first_party(), "allow");
    assert_eq!(config.cookie_third_party(), "session");
}

#[test]
fn an_unknown_key_under_cookies_is_accepted_and_ignored() {
    let (_directory, path) =
        write_config("[cookies]\nfirst_party = \"session\"\nfuture_scope = \"maybe\"\n");
    let config = load_config(&path).expect("an unknown cookies key must not fail the load");
    assert_eq!(config.cookie_first_party(), "session");
}
