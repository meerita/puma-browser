// @file crates/browser-core/tests/config_write_through.rs
// @description Behavior tests for the ConfigStore write-through in the navigation controller.
// @layer core
// @created meerita <meerita@icloud.com>

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use browser_core::{ConfigStore, CookiePolicy, CookieScope, NavigationController, StorageError};

/// An in-memory `ConfigStore` that records every write, so a test can assert exactly which
/// keys and values the controller persisted.
#[derive(Default)]
struct FakeConfigStore {
    values: Mutex<HashMap<String, String>>,
}

impl FakeConfigStore {
    fn stored(&self, key: &str) -> Option<String> {
        self.values
            .lock()
            .expect("the fake store lock must not be poisoned")
            .get(key)
            .cloned()
    }

    fn write_count(&self) -> usize {
        self.values
            .lock()
            .expect("the fake store lock must not be poisoned")
            .len()
    }
}

impl ConfigStore for FakeConfigStore {
    fn config_value(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self
            .values
            .lock()
            .expect("the fake store lock must not be poisoned")
            .get(key)
            .cloned())
    }

    fn set_config_value(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.values
            .lock()
            .expect("the fake store lock must not be poisoned")
            .insert(key.to_string(), value.to_string());
        Ok(())
    }
}

#[test]
fn set_global_cookie_policy_persists_the_policy_word() {
    let store = Arc::new(FakeConfigStore::default());
    let mut controller = NavigationController::new().with_config_store(store.clone());

    controller
        .set_global_cookie_policy(CookieScope::FirstParty, CookiePolicy::Allow)
        .expect("applying and persisting a first-party allow must succeed");

    assert_eq!(
        store.stored("cookies.first_party").as_deref(),
        Some("allow"),
        "the first-party scope persists under its own key as the policy word"
    );
}

#[test]
fn set_global_third_party_policy_persists_under_the_third_party_key() {
    let store = Arc::new(FakeConfigStore::default());
    let mut controller = NavigationController::new().with_config_store(store.clone());

    controller
        .set_global_cookie_policy(CookieScope::ThirdParty, CookiePolicy::Session)
        .expect("applying and persisting a third-party session policy must succeed");

    assert_eq!(
        store.stored("cookies.third_party").as_deref(),
        Some("session"),
        "the third-party scope persists under its own key"
    );
    assert_eq!(
        store.stored("cookies.first_party"),
        None,
        "changing the third-party scope does not write the first-party key"
    );
}

#[test]
fn set_search_engine_with_a_valid_url_persists_both_keys_and_updates_the_engine() {
    let store = Arc::new(FakeConfigStore::default());
    let mut controller = NavigationController::new().with_config_store(store.clone());

    controller
        .set_search_engine(
            "https://search.example/results".to_string(),
            "query".to_string(),
        )
        .expect("a valid https base URL must apply and persist");

    assert_eq!(
        store.stored("search.base_url").as_deref(),
        Some("https://search.example/results"),
        "the base URL persists verbatim"
    );
    assert_eq!(
        store.stored("search.query_parameter").as_deref(),
        Some("query"),
        "the query parameter persists verbatim"
    );
    assert_eq!(
        controller.search_engine().base_url(),
        "https://search.example/results",
        "the held engine is replaced with the configured one"
    );
    assert_eq!(controller.search_engine().query_parameter(), "query");
}

#[test]
fn set_search_engine_with_an_invalid_url_returns_an_error_and_writes_nothing() {
    let store = Arc::new(FakeConfigStore::default());
    let mut controller = NavigationController::new().with_config_store(store.clone());

    let result = controller.set_search_engine("file:///etc/passwd".to_string(), "q".to_string());

    assert!(
        result.is_err(),
        "a non-http(s) base URL is rejected before anything is applied or persisted"
    );
    assert_eq!(
        store.write_count(),
        0,
        "a rejected engine writes no config keys"
    );
    assert_eq!(
        controller.search_engine().base_url(),
        "https://lite.duckduckgo.com/lite/",
        "the held engine is unchanged after a rejected update"
    );
}

#[test]
fn persist_setting_writes_a_terminal_toggle_key_verbatim() {
    let store = Arc::new(FakeConfigStore::default());
    let controller = NavigationController::new().with_config_store(store.clone());

    controller
        .persist_setting("ui.copy_on_select", "false")
        .expect("persisting a terminal toggle must succeed");

    assert_eq!(
        store.stored("ui.copy_on_select").as_deref(),
        Some("false"),
        "the key and value are written verbatim, uninterpreted by the store"
    );
}

#[test]
fn every_setter_applies_live_and_returns_ok_without_a_store() {
    let mut controller = NavigationController::new();

    controller
        .set_global_cookie_policy(CookieScope::FirstParty, CookiePolicy::Allow)
        .expect("a cookie policy change without a store applies live and returns Ok");
    controller
        .set_search_engine("https://search.example/".to_string(), "q".to_string())
        .expect("a search engine change without a store applies live and returns Ok");
    controller
        .persist_setting("ui.force_osc52", "true")
        .expect("persisting without a store is a no-op that returns Ok");

    assert_eq!(
        controller.search_engine().base_url(),
        "https://search.example/",
        "the engine still changes live even with no store to persist to"
    );
}
