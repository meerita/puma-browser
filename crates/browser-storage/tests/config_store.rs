// @file crates/browser-storage/tests/config_store.rs
// @description Verifies the SQLite config store: round-trip, upsert replacement, and missing keys.
// @layer storage
// @created meerita <meerita@icloud.com>

use browser_storage::{ConfigStore, SqliteStorage};

/// Opens a prepared in-memory database for a test, panicking with a clear message if the
/// substrate itself fails to open.
fn open() -> SqliteStorage {
    SqliteStorage::open_in_memory().expect("in-memory database must open and migrate")
}

#[test]
fn setting_then_reading_a_key_returns_its_value() {
    let storage = open();
    storage
        .set_config_value("cookies.first_party", "session")
        .expect("setting a value must succeed");
    assert_eq!(
        storage
            .config_value("cookies.first_party")
            .expect("reading a value must succeed"),
        Some("session".to_string())
    );
}

#[test]
fn setting_the_same_key_again_returns_the_latest_value() {
    let storage = open();
    storage
        .set_config_value("search.query_parameter", "q")
        .expect("first set must succeed");
    storage
        .set_config_value("search.query_parameter", "query")
        .expect("second set must succeed");
    assert_eq!(
        storage
            .config_value("search.query_parameter")
            .expect("reading a value must succeed"),
        Some("query".to_string()),
        "the newer value must win"
    );
}

#[test]
fn reading_an_unset_key_returns_none() {
    let storage = open();
    assert_eq!(
        storage
            .config_value("network.unwrap_tracking")
            .expect("reading an unset value must succeed"),
        None
    );
}

#[test]
fn a_fresh_database_migrates_to_the_config_schema_version() {
    let storage = open();
    assert_eq!(
        storage.schema_version().expect("schema version must read"),
        3,
        "the config table migration must have run on a fresh database"
    );
    // A round-trip succeeds only if the `config` table exists after migrations.
    storage
        .set_config_value("ui.copy_on_select", "true")
        .expect("writing to the config table must succeed");
    assert_eq!(
        storage
            .config_value("ui.copy_on_select")
            .expect("reading the config table must succeed"),
        Some("true".to_string())
    );
}
