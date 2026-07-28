// @file crates/browser-storage/tests/site_policy_store.rs
// @description Verifies the SQLite site-policy store: upsert, read, remove, list, and clear.
// @layer storage
// @created meerita <meerita@icloud.com>

use browser_storage::{SitePolicyStore, SqliteStorage};

/// Opens a prepared in-memory database for a test, panicking with a clear message if the
/// substrate itself fails to open.
fn open() -> SqliteStorage {
    SqliteStorage::open_in_memory().expect("in-memory database must open and migrate")
}

#[test]
fn setting_then_reading_a_domain_returns_its_policy() {
    let storage = open();
    storage
        .set_site_policy("example.com", "session", 100)
        .expect("setting a policy must succeed");
    assert_eq!(
        storage
            .site_policy("example.com")
            .expect("reading a policy must succeed"),
        Some("session".to_string())
    );
}

#[test]
fn reading_an_unset_domain_returns_none() {
    let storage = open();
    assert_eq!(
        storage
            .site_policy("absent.com")
            .expect("reading a policy must succeed"),
        None
    );
}

#[test]
fn setting_the_same_domain_again_upserts_the_new_value() {
    let storage = open();
    storage
        .set_site_policy("example.com", "session", 100)
        .expect("first set must succeed");
    storage
        .set_site_policy("example.com", "reject", 200)
        .expect("second set must succeed");
    assert_eq!(
        storage
            .site_policy("example.com")
            .expect("reading a policy must succeed"),
        Some("reject".to_string()),
        "the newer value must win"
    );
    assert_eq!(
        storage
            .all_site_policies()
            .expect("listing must succeed")
            .len(),
        1,
        "an upsert must not create a duplicate row"
    );
}

#[test]
fn removing_a_domain_deletes_its_policy() {
    let storage = open();
    storage
        .set_site_policy("example.com", "session", 100)
        .expect("set must succeed");
    storage
        .remove_site_policy("example.com")
        .expect("remove must succeed");
    assert_eq!(
        storage
            .site_policy("example.com")
            .expect("reading a policy must succeed"),
        None
    );
}

#[test]
fn removing_an_absent_domain_is_a_no_op() {
    let storage = open();
    storage
        .remove_site_policy("absent.com")
        .expect("removing an absent domain must succeed");
}

#[test]
fn all_site_policies_returns_every_row() {
    let storage = open();
    storage
        .set_site_policy("first.com", "session", 100)
        .expect("set must succeed");
    storage
        .set_site_policy("second.com", "allow", 200)
        .expect("set must succeed");
    let mut policies = storage.all_site_policies().expect("listing must succeed");
    policies.sort();
    assert_eq!(
        policies,
        vec![
            ("first.com".to_string(), "session".to_string()),
            ("second.com".to_string(), "allow".to_string()),
        ]
    );
}

#[test]
fn clear_site_policies_empties_the_table() {
    let storage = open();
    storage
        .set_site_policy("first.com", "session", 100)
        .expect("set must succeed");
    storage
        .set_site_policy("second.com", "allow", 200)
        .expect("set must succeed");
    storage.clear_site_policies().expect("clear must succeed");
    assert!(
        storage
            .all_site_policies()
            .expect("listing must succeed")
            .is_empty(),
        "every row must be gone"
    );
}
