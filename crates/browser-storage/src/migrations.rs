// @file crates/browser-storage/src/migrations.rs
// @description Hand-rolled PRAGMA user_version migration runner and the schema migrations.
// @layer storage
// @created meerita <meerita@icloud.com>

use rusqlite::Connection;

use crate::error::StorageError;

/// A single forward schema migration: the SQL to apply and the `user_version` it
/// produces once applied.
struct Migration {
    target_version: i64,
    sql: &'static str,
}

/// The ordered migration list. Each entry raises `user_version` by one; new schema
/// changes are appended here and never edited in place, so an existing database always
/// reaches the current schema by replaying the entries it has not yet seen.
const MIGRATIONS: &[Migration] = &[
    Migration {
        target_version: 1,
        sql: "
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
        CREATE INDEX idx_pages_host  ON pages(host);
        CREATE INDEX idx_visits_time ON visits(visited_at DESC);
        CREATE INDEX idx_visits_page ON visits(page_id);
    ",
    },
    Migration {
        target_version: 2,
        sql: "
        CREATE TABLE site_cookie_policies (
          id         INTEGER PRIMARY KEY,
          domain     TEXT    NOT NULL UNIQUE,
          policy     TEXT    NOT NULL,
          created_at INTEGER NOT NULL
        );
    ",
    },
    Migration {
        target_version: 3,
        sql: "
        CREATE TABLE config (
          key        TEXT    NOT NULL PRIMARY KEY,
          value      TEXT    NOT NULL,
          updated_at INTEGER NOT NULL
        );
    ",
    },
];

/// Applies every migration whose target version is greater than the database's current
/// `user_version`, each inside its own transaction, then advances `user_version`.
///
/// Running against an already-current database applies nothing and succeeds, so startup
/// can call this unconditionally.
pub(crate) fn run_migrations(connection: &Connection) -> Result<(), StorageError> {
    let current_version = read_user_version(connection)?;
    for migration in MIGRATIONS {
        if migration.target_version <= current_version {
            continue;
        }
        apply_migration(connection, migration)?;
    }
    Ok(())
}

/// Reads the database schema version stored in `PRAGMA user_version`.
fn read_user_version(connection: &Connection) -> Result<i64, StorageError> {
    connection
        .query_row("PRAGMA user_version;", [], |row| row.get(0))
        .map_err(|_| StorageError::MigrationFailed)
}

/// Applies one migration's SQL and sets `user_version` atomically.
///
/// `user_version` cannot be set through a bound parameter, so the already-validated
/// integer target version is formatted directly into the pragma statement. The value
/// originates only from the `const MIGRATIONS` table, never from user input.
fn apply_migration(connection: &Connection, migration: &Migration) -> Result<(), StorageError> {
    connection
        .execute_batch("BEGIN;")
        .map_err(|_| StorageError::MigrationFailed)?;
    let applied = apply_migration_body(connection, migration);
    if applied.is_err() {
        let _ = connection.execute_batch("ROLLBACK;");
        return applied;
    }
    connection
        .execute_batch("COMMIT;")
        .map_err(|_| StorageError::MigrationFailed)
}

/// Executes the migration SQL and advances `user_version` within the open transaction.
fn apply_migration_body(
    connection: &Connection,
    migration: &Migration,
) -> Result<(), StorageError> {
    connection
        .execute_batch(migration.sql)
        .map_err(|_| StorageError::MigrationFailed)?;
    let set_version = format!("PRAGMA user_version = {};", migration.target_version);
    connection
        .execute_batch(&set_version)
        .map_err(|_| StorageError::MigrationFailed)
}
