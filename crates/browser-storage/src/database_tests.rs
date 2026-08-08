// @file crates/browser-storage/src/database_tests.rs
// @description Verifies the SQLite substrate: migrations, pragmas, schema, and cascade behavior.
// @layer storage
// @created meerita <meerita@icloud.com>

use rusqlite::Connection;

use super::SqliteStorage;
use crate::migrations::run_migrations;

/// The migration-1 schema, applied directly to a raw connection so a test can simulate a
/// version-1 database that predates the cookie-policy table and then upgrade it.
const V1_SCHEMA: &str = "
    CREATE TABLE pages (
      id             INTEGER PRIMARY KEY,
      url            TEXT    NOT NULL UNIQUE,
      host           TEXT    NOT NULL,
      title          TEXT,
      visit_count    INTEGER NOT NULL DEFAULT 0,
      typed_count    INTEGER NOT NULL DEFAULT 0,
      first_visit_at INTEGER NOT NULL,
      last_visit_at  INTEGER NOT NULL
    );
    CREATE TABLE visits (
      id         INTEGER PRIMARY KEY,
      page_id    INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
      visited_at INTEGER NOT NULL,
      was_typed  INTEGER NOT NULL DEFAULT 0
    );
    PRAGMA user_version = 1;
";

/// The migration-2 schema, applied directly to a raw connection so a test can simulate a
/// version-2 database that predates the config table and then upgrade it.
const V2_SCHEMA: &str = "
    CREATE TABLE pages (
      id             INTEGER PRIMARY KEY,
      url            TEXT    NOT NULL UNIQUE,
      host           TEXT    NOT NULL,
      title          TEXT,
      visit_count    INTEGER NOT NULL DEFAULT 0,
      typed_count    INTEGER NOT NULL DEFAULT 0,
      first_visit_at INTEGER NOT NULL,
      last_visit_at  INTEGER NOT NULL
    );
    CREATE TABLE visits (
      id         INTEGER PRIMARY KEY,
      page_id    INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
      visited_at INTEGER NOT NULL,
      was_typed  INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE site_cookie_policies (
      id         INTEGER PRIMARY KEY,
      domain     TEXT    NOT NULL UNIQUE,
      policy     TEXT    NOT NULL,
      created_at INTEGER NOT NULL
    );
    PRAGMA user_version = 2;
";

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
fn opening_in_memory_migrates_to_the_current_version() {
    let storage = open_prepared();
    assert_eq!(
        storage.schema_version().expect("schema version must read"),
        3
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
    assert_eq!(version, 3);
}

#[test]
fn site_cookie_policies_table_has_the_expected_columns() {
    let storage = open_prepared();
    assert_eq!(
        column_names(&storage, "site_cookie_policies"),
        ["id", "domain", "policy", "created_at"]
    );
}

#[test]
fn config_table_has_the_expected_columns() {
    let storage = open_prepared();
    assert_eq!(
        column_names(&storage, "config"),
        ["key", "value", "updated_at"]
    );
}

#[test]
fn upgrading_a_version_two_database_reaches_version_three_and_keeps_site_policy_data() {
    let connection = Connection::open_in_memory().expect("in-memory connection must open");
    connection
        .execute_batch(V2_SCHEMA)
        .expect("version-2 schema must apply");
    connection
        .execute(
            "INSERT INTO site_cookie_policies (domain, policy, created_at) \
             VALUES ('example.com', 'session', 0);",
            [],
        )
        .expect("site policy insert must succeed");

    run_migrations(&connection).expect("upgrade migrations must succeed");

    let version: i64 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version pragma must read");
    assert_eq!(version, 3, "the database must reach version 3");
    let config_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'table' AND name = 'config';",
            [],
            |row| row.get(0),
        )
        .expect("sqlite_master must read");
    assert_eq!(config_table_count, 1, "the new config table must exist");
    let policy_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM site_cookie_policies;", [], |row| {
            row.get(0)
        })
        .expect("site policy count must read");
    assert_eq!(
        policy_count, 1,
        "existing site-policy data must survive the upgrade"
    );
}

#[test]
fn upgrading_a_version_one_database_reaches_the_current_version_and_keeps_history_data() {
    let connection = Connection::open_in_memory().expect("in-memory connection must open");
    connection
        .execute_batch(V1_SCHEMA)
        .expect("version-1 schema must apply");
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

    run_migrations(&connection).expect("upgrade migrations must succeed");

    let version: i64 = connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .expect("user_version pragma must read");
    assert_eq!(version, 3, "the database must reach the current version");
    let cookie_table_count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master \
             WHERE type = 'table' AND name = 'site_cookie_policies';",
            [],
            |row| row.get(0),
        )
        .expect("sqlite_master must read");
    assert_eq!(cookie_table_count, 1, "the new table must exist");
    let pages_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM pages;", [], |row| row.get(0))
        .expect("page count must read");
    let visits_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM visits;", [], |row| row.get(0))
        .expect("visit count must read");
    assert_eq!(
        pages_count, 1,
        "existing page data must survive the upgrade"
    );
    assert_eq!(
        visits_count, 1,
        "existing visit data must survive the upgrade"
    );
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
