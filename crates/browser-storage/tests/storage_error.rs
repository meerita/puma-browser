// @file crates/browser-storage/tests/storage_error.rs
// @description Verifies StorageError Display strings expose no SQL or driver internals.
// @layer storage
// @created meerita <meerita@icloud.com>

use browser_storage::StorageError;

/// The Display string of every variant, so a new variant added without a test fails
/// to compile here (exhaustive match) rather than silently escaping the safety check.
fn display_of(error: &StorageError) -> String {
    match error {
        StorageError::OpenFailed
        | StorageError::MigrationFailed
        | StorageError::QueryFailed
        | StorageError::NotFound => error.to_string(),
    }
}

fn all_variants() -> [StorageError; 4] {
    [
        StorageError::OpenFailed,
        StorageError::MigrationFailed,
        StorageError::QueryFailed,
        StorageError::NotFound,
    ]
}

#[test]
fn every_variant_has_a_non_empty_message() {
    for variant in all_variants() {
        assert!(
            !display_of(&variant).is_empty(),
            "variant must render a user-facing message"
        );
    }
}

#[test]
fn no_variant_leaks_sql_or_driver_internals() {
    let forbidden_fragments = [
        "SELECT",
        "INSERT",
        "UPDATE",
        "DELETE",
        "sql",
        "SQL",
        "rusqlite",
        "sqlite",
        "SQLITE",
        "error code",
        "no such table",
        ";",
    ];
    for variant in all_variants() {
        let message = display_of(&variant);
        for fragment in forbidden_fragments {
            assert!(
                !message.contains(fragment),
                "message {message:?} must not contain {fragment:?}"
            );
        }
    }
}

#[test]
fn open_failed_reports_an_open_failure() {
    assert_eq!(
        StorageError::OpenFailed.to_string(),
        "failed to open the storage database"
    );
}

#[test]
fn migration_failed_reports_a_migration_failure() {
    assert_eq!(
        StorageError::MigrationFailed.to_string(),
        "database migration failed"
    );
}

#[test]
fn query_failed_reports_a_query_failure() {
    assert_eq!(
        StorageError::QueryFailed.to_string(),
        "storage query failed"
    );
}

#[test]
fn not_found_reports_a_missing_record() {
    assert_eq!(
        StorageError::NotFound.to_string(),
        "requested record was not found"
    );
}
