// @file crates/browser-cli/src/run_mode_tests.rs
// @description Verifies argument resolution and environment-driven terminal settings parsing.
// @layer cli
// @created meerita <meerita@icloud.com>

use std::path::Path;
use std::sync::Arc;

use tempfile::tempdir;

use browser_core::{ConfigStore, CookiePolicy, CookieScope, HistoryMode, NavigationController};
use browser_storage::{load_config, BrowserConfig, SqliteStorage};

use super::{
    copy_on_select_enabled, force_osc52_enabled, resolve_cookie_policy, resolve_history_mode,
    resolve_mode, resolve_search_engine, resolve_setting, resolve_toggle, search_enabled,
    unwrap_tracking_enabled, ResolvedMode, COOKIES_FIRST_PARTY_KEY, COPY_ON_SELECT_KEY,
};

/// Loads a config whose `[cookies]` section carries the given scope words.
///
/// The config fields are private, so a real TOML file resolved through `load_config` is the
/// only way to build one with specific cookie values in a test.
fn config_with_cookies(first_party: &str, third_party: &str) -> BrowserConfig {
    let config_directory = tempdir().expect("a temporary config directory must be created");
    let config_path = config_directory.path().join("config.toml");
    std::fs::write(
        &config_path,
        format!("[cookies]\nfirst_party = \"{first_party}\"\nthird_party = \"{third_party}\"\n"),
    )
    .expect("the temporary config file must be written");
    load_config(&config_path).expect("a well-formed config must load")
}

fn resolved_in(working_directory: &Path, arguments: &[&str]) -> ResolvedMode {
    resolve_mode(
        arguments.iter().map(|argument| argument.to_string()),
        working_directory,
    )
}

fn resolved_for(arguments: &[&str]) -> ResolvedMode {
    let working_directory = tempdir().expect("a temporary working directory must be created");
    resolved_in(working_directory.path(), arguments)
}

#[test]
fn no_arguments_opens_the_terminal_on_a_blank_page() {
    assert!(matches!(resolved_for(&[]), ResolvedMode::TerminalBlank));
}

#[test]
fn mcp_keyword_selects_mcp_mode() {
    assert!(matches!(resolved_for(&["mcp"]), ResolvedMode::Mcp));
}

#[test]
fn terminal_keyword_without_an_address_opens_a_blank_page() {
    assert!(matches!(
        resolved_for(&["terminal"]),
        ResolvedMode::TerminalBlank
    ));
}

#[test]
fn valid_url_argument_becomes_the_terminal_load_target() {
    let resolved = resolved_for(&["https://example.com"]);
    let ResolvedMode::TerminalUrl(url) = resolved else {
        panic!("a valid URL must resolve to a terminal load target");
    };
    assert_eq!(url.host_str(), Some("example.com"));
}

#[test]
fn a_startup_url_keeps_its_fragment_for_the_opening_viewport() {
    let resolved = resolved_for(&["https://example.com/page#section"]);
    let ResolvedMode::TerminalUrl(url) = resolved else {
        panic!("a valid URL with a fragment must resolve to a terminal load target");
    };
    assert_eq!(url.fragment(), Some("section"));
}

#[test]
fn bare_host_argument_is_assumed_to_be_https() {
    let resolved = resolved_for(&["example.com"]);
    let ResolvedMode::TerminalUrl(url) = resolved else {
        panic!("a bare host must resolve to a terminal load target");
    };
    assert_eq!(url.scheme(), "https");
}

#[test]
fn bare_token_with_no_matching_file_is_assumed_to_be_https() {
    let working_directory = tempdir().expect("a temporary working directory must be created");
    let resolved = resolved_in(working_directory.path(), &["not-a-real-file.html"]);
    let ResolvedMode::TerminalUrl(url) = resolved else {
        panic!("a bare token with no matching file must resolve to a web address");
    };
    assert_eq!(url.scheme(), "https");
}

#[test]
fn bare_token_naming_an_existing_file_becomes_a_file_load_target() {
    let working_directory = tempdir().expect("a temporary working directory must be created");
    std::fs::write(working_directory.path().join("page.html"), "<p>hello</p>")
        .expect("the temporary file must be written");
    let resolved = resolved_in(working_directory.path(), &["page.html"]);
    let ResolvedMode::TerminalUrl(url) = resolved else {
        panic!("an existing local file must resolve to a terminal load target");
    };
    assert_eq!(url.scheme(), "file");
}

#[test]
fn absolute_path_to_a_missing_file_resolves_to_a_usage_error_echoing_the_argument() {
    let working_directory = tempdir().expect("a temporary working directory must be created");
    let missing = working_directory.path().join("does-not-exist.html");
    let argument = missing.to_string_lossy().to_string();
    let resolved = resolved_in(working_directory.path(), &[argument.as_str()]);
    let ResolvedMode::UsageError(message) = resolved else {
        panic!("a missing absolute path must resolve to a usage error");
    };
    assert!(
        message.contains(&argument),
        "the usage error must echo the typed argument: {message:?}"
    );
}

#[test]
fn unsupported_scheme_argument_resolves_to_a_usage_error() {
    assert!(matches!(
        resolved_for(&["ftp://example.com"]),
        ResolvedMode::UsageError(_)
    ));
}

#[test]
fn malformed_url_argument_resolves_to_a_usage_error() {
    assert!(matches!(
        resolved_for(&["http://"]),
        ResolvedMode::UsageError(_)
    ));
}

#[test]
fn mcp_keyword_wins_over_a_later_url_argument() {
    assert!(matches!(
        resolved_for(&["mcp", "https://example.com"]),
        ResolvedMode::Mcp
    ));
}

#[test]
fn copy_on_select_is_enabled_when_the_variable_is_unset() {
    assert!(copy_on_select_enabled(None));
}

#[test]
fn copy_on_select_is_disabled_by_zero_or_false() {
    assert!(!copy_on_select_enabled(Some("0")));
    assert!(!copy_on_select_enabled(Some("false")));
}

#[test]
fn copy_on_select_stays_enabled_for_any_other_value() {
    assert!(copy_on_select_enabled(Some("1")));
    assert!(copy_on_select_enabled(Some("true")));
    assert!(copy_on_select_enabled(Some("")));
}

#[test]
fn force_osc52_is_off_when_the_variable_is_unset() {
    assert!(!force_osc52_enabled(None));
}

#[test]
fn force_osc52_is_on_only_for_one_or_true() {
    assert!(force_osc52_enabled(Some("1")));
    assert!(force_osc52_enabled(Some("true")));
    assert!(!force_osc52_enabled(Some("0")));
    assert!(!force_osc52_enabled(Some("yes")));
}

#[test]
fn search_is_enabled_when_the_variable_is_unset() {
    assert!(search_enabled(None));
}

#[test]
fn search_is_disabled_by_zero_or_false() {
    assert!(!search_enabled(Some("0")));
    assert!(!search_enabled(Some("false")));
}

#[test]
fn search_stays_enabled_for_any_other_value() {
    assert!(search_enabled(Some("1")));
    assert!(search_enabled(Some("true")));
    assert!(search_enabled(Some("")));
}

#[test]
fn unwrap_tracking_is_enabled_when_the_variable_is_unset() {
    assert!(unwrap_tracking_enabled(None));
}

#[test]
fn unwrap_tracking_is_disabled_by_zero_or_false() {
    assert!(!unwrap_tracking_enabled(Some("0")));
    assert!(!unwrap_tracking_enabled(Some("false")));
}

#[test]
fn unwrap_tracking_stays_enabled_for_any_other_value() {
    assert!(unwrap_tracking_enabled(Some("1")));
    assert!(unwrap_tracking_enabled(Some("true")));
    assert!(unwrap_tracking_enabled(Some("")));
}

#[test]
fn the_file_mode_applies_when_the_env_override_is_unset() {
    assert_eq!(
        resolve_history_mode("disabled", None),
        HistoryMode::Disabled
    );
}

#[test]
fn the_env_override_wins_over_the_file_mode() {
    assert_eq!(
        resolve_history_mode("persistent", Some("disabled")),
        HistoryMode::Disabled
    );
}

#[test]
fn an_unrecognized_mode_resolves_to_persistent() {
    assert_eq!(
        resolve_history_mode("nonsense", None),
        HistoryMode::Persistent
    );
    assert_eq!(
        resolve_history_mode("disabled", Some("nonsense")),
        HistoryMode::Persistent
    );
}

#[test]
fn configured_scope_words_resolve_to_the_matching_policy_pair() {
    let config = config_with_cookies("session", "allow");
    let pair = resolve_cookie_policy(&config, None);
    assert_eq!(pair.first_party, CookiePolicy::Session);
    assert_eq!(pair.third_party, CookiePolicy::Allow);
}

#[test]
fn an_unrecognized_scope_word_resolves_to_reject_for_that_scope() {
    let config = config_with_cookies("nonsense", "allow");
    let pair = resolve_cookie_policy(&config, None);
    assert_eq!(pair.first_party, CookiePolicy::Reject);
    assert_eq!(pair.third_party, CookiePolicy::Allow);
}

#[test]
fn an_absent_cookies_section_resolves_to_reject_in_both_scopes() {
    let pair = resolve_cookie_policy(&BrowserConfig::default(), None);
    assert_eq!(pair.first_party, CookiePolicy::Reject);
    assert_eq!(pair.third_party, CookiePolicy::Reject);
}

#[test]
fn a_stored_cookie_policy_overrides_the_toml_value() {
    let config = config_with_cookies("session", "allow");
    let storage = SqliteStorage::open_in_memory().expect("in-memory SQLite must open");
    storage
        .set_config_value(COOKIES_FIRST_PARTY_KEY, "reject")
        .expect("the config value must persist");
    let pair = resolve_cookie_policy(&config, Some(&storage));
    assert_eq!(pair.first_party, CookiePolicy::Reject);
    assert_eq!(pair.third_party, CookiePolicy::Allow);
}

#[test]
fn the_config_store_value_overrides_the_toml_value() {
    assert_eq!(
        resolve_setting("default", Some("toml"), Some("store"), None),
        "store"
    );
}

#[test]
fn the_env_value_wins_over_the_config_store_value() {
    assert_eq!(
        resolve_setting("default", Some("toml"), Some("store"), Some("env")),
        "env"
    );
}

#[test]
fn an_absent_value_in_every_layer_yields_the_built_in_default() {
    assert_eq!(
        resolve_setting("default", None, None, None::<&str>),
        "default"
    );
}

#[test]
fn a_stored_toggle_value_applies_when_no_env_variable_is_set() {
    let resolved = resolve_toggle(Some("false"), None, copy_on_select_enabled);
    assert!(!resolved.value);
    assert!(!resolved.env_overridden);
}

#[test]
fn an_env_toggle_value_wins_over_the_stored_value_and_is_reported_as_overridden() {
    let resolved = resolve_toggle(Some("false"), Some("true"), copy_on_select_enabled);
    assert!(resolved.value);
    assert!(resolved.env_overridden);
}

#[test]
fn a_toggle_with_no_stored_or_env_value_takes_the_built_in_default() {
    let resolved = resolve_toggle(None, None, copy_on_select_enabled);
    assert!(resolved.value);
    assert!(!resolved.env_overridden);
}

#[test]
fn a_stored_toggle_read_through_the_config_store_key_resolves() {
    let storage = SqliteStorage::open_in_memory().expect("in-memory SQLite must open");
    storage
        .set_config_value(COPY_ON_SELECT_KEY, "false")
        .expect("the config value must persist");
    let stored = storage
        .config_value(COPY_ON_SELECT_KEY)
        .expect("the config value must read back");
    let resolved = resolve_toggle(stored.as_deref(), None, copy_on_select_enabled);
    assert!(!resolved.value);
}

/// A shared in-memory store the controller writes through and the resolver reads back, standing
/// in for the SQLite file the panel and the next startup both open.
fn shared_store() -> Arc<SqliteStorage> {
    Arc::new(SqliteStorage::open_in_memory().expect("in-memory SQLite must open and migrate"))
}

#[test]
fn a_checkbox_persisted_through_the_controller_is_read_back_by_the_resolver() {
    let storage = shared_store();
    let controller = NavigationController::new().with_config_store(storage.clone());

    controller
        .persist_setting(COPY_ON_SELECT_KEY, "false")
        .expect("the checkbox value must persist through the store");

    let stored = storage
        .config_value(COPY_ON_SELECT_KEY)
        .expect("the value must read back from the same store");
    let resolved = resolve_toggle(stored.as_deref(), None, copy_on_select_enabled);
    assert!(!resolved.value);
}

#[test]
fn a_cookie_policy_set_through_the_controller_is_read_back_above_the_toml_value() {
    let storage = shared_store();
    let mut controller = NavigationController::new().with_config_store(storage.clone());

    controller
        .set_global_cookie_policy(CookieScope::FirstParty, CookiePolicy::Allow)
        .expect("the global cookie policy must apply and persist");

    // The panel-written store value must win over the TOML first-party value, while the
    // third-party scope, absent from the store, keeps the TOML value.
    let config = config_with_cookies("session", "session");
    let pair = resolve_cookie_policy(&config, Some(storage.as_ref()));
    assert_eq!(pair.first_party, CookiePolicy::Allow);
    assert_eq!(pair.third_party, CookiePolicy::Session);
}

#[test]
fn a_search_engine_set_through_the_controller_is_read_back_by_the_resolver() {
    let storage = shared_store();
    let mut controller = NavigationController::new().with_config_store(storage.clone());

    controller
        .set_search_engine("https://roundtrip.test/".to_string(), "qq".to_string())
        .expect("a valid search engine must apply and persist");

    let engine = resolve_search_engine(Some(storage.as_ref()));
    assert_eq!(engine.base_url(), "https://roundtrip.test/");
    assert_eq!(engine.query_parameter(), "qq");
}

#[test]
fn a_panel_written_toggle_loses_to_an_env_override() {
    let storage = shared_store();
    let controller = NavigationController::new().with_config_store(storage.clone());

    controller
        .persist_setting(COPY_ON_SELECT_KEY, "false")
        .expect("the checkbox value must persist through the store");

    // The store value written by the panel is overridden by a PUMA_* environment variable and
    // reported as environment-fixed, confirming the store sits below env in the precedence chain.
    let stored = storage
        .config_value(COPY_ON_SELECT_KEY)
        .expect("the value must read back from the same store");
    let resolved = resolve_toggle(stored.as_deref(), Some("true"), copy_on_select_enabled);
    assert!(resolved.value);
    assert!(resolved.env_overridden);
}
