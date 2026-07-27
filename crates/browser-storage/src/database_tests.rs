// @file crates/browser-storage/src/database_tests.rs
// @description Verifies the SQLite substrate: migrations, pragmas, schema, and cascade behavior.
// @layer storage
// @created meerita <meerita@icloud.com>

use super::SqliteStorage;
use crate::migrations::run_migrations;

/// Opens a prepared in-memory database for a test, panicking with a clear message if
/// the substrate itself fails to open.
fn open_prepared() -> SqliteStorage {
    SqliteStorage::open_in_memory().expect("in-memory database must open and migrate")
}

/// Reads the column names of `table_name` in definition order.
fn column_names(storage: &SqliteStorage, table_name: &str) -> Vec<String> {
    let connection = storage
        .connection
        .lock()
        .expect("connection lock must not poison");
    let query = format!("PRAGMA table_info({table_name});");
    let mut statement = connection.prepare(&query).expect("table_info must prepare");
    let names = statement
        .query_map([], |row| row.get::<_, String>(1))
        .expect("table_info must query")
        .collect::<Result<Vec<_>, _>>()
        .expect("column names must read");
    names
}

#[test]
fn opening_in_memory_migrates_to_version_one() {
    let storage = open_prepared();
    assert_eq!(
        storage.schema_version().expect("schema version must read"),
        1
    );
}

#[test]
fn re_running_migrations_on_a_current_database_is_a_no_op() {
    let storage = open_prepared();
    let connection = storage
        .connection
        .lock()
        .expect("connection lock must not poison");
    run_migrations(&connection).expect("re-running migrations must succeed");
    let version: i64 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version pragma must read");
    assert_eq!(version, 1);
}

#[test]
fn pages_table_has_the_expected_columns() {
    let storage = open_prepared();
    assert_eq!(
        column_names(&storage, "pages"),
        [
            "id",
            "url",
            "host",
            "title",
            "visit_count",
            "typed_count",
            "first_visit_at",
            "last_visit_at",
        ]
    );
}

#[test]
fn visits_table_has_the_expected_columns() {
    let storage = open_prepared();
    assert_eq!(
        column_names(&storage, "visits"),
        ["id", "page_id", "visited_at", "was_typed"]
    );
}

#[test]
fn foreign_key_enforcement_is_enabled() {
    let storage = open_prepared();
    let connection = storage
        .connection
        .lock()
        .expect("connection lock must not poison");
    let enforced: i64 = connection
        .query_row("PRAGMA foreign_keys;", [], |row| row.get(0))
        .expect("foreign_keys pragma must read");
    assert_eq!(enforced, 1);
}

#[test]
fn inserting_a_visit_for_a_missing_page_is_rejected() {
    let storage = open_prepared();
    let connection = storage
        .connection
        .lock()
        .expect("connection lock must not poison");
    let dangling_insert = connection.execute(
        "INSERT INTO visits (page_id, visited_at, was_typed) VALUES (999, 0, 0);",
        [],
    );
    assert!(
        dangling_insert.is_err(),
        "a visit referencing a missing page must violate the foreign key"
    );
}

#[test]
fn deleting_a_page_cascades_to_its_visits() {
    let storage = open_prepared();
    let connection = storage
        .connection
        .lock()
        .expect("connection lock must not poison");
    connection
        .execute(
            "INSERT INTO pages (id, url, host, first_visit_at, last_visit_at) \
             VALUES (1, 'https://example.com/', 'example.com', 0, 0);",
            [],
        )
        .expect("page insert must succeed");
    connection
        .execute(
            "INSERT INTO visits (page_id, visited_at, was_typed) VALUES (1, 0, 0);",
            [],
        )
        .expect("visit insert must succeed");
    connection
        .execute("DELETE FROM pages WHERE id = 1;", [])
        .expect("page delete must succeed");
    let remaining_visits: i64 = connection
        .query_row("SELECT COUNT(*) FROM visits;", [], |row| row.get(0))
        .expect("visit count must read");
    assert_eq!(remaining_visits, 0);
}
