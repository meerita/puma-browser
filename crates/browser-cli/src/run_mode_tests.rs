// @file crates/browser-cli/src/run_mode_tests.rs
// @description Verifies argument resolution and environment-driven terminal settings parsing.
// @layer cli
// @created meerita <meerita@icloud.com>

use std::path::Path;

use tempfile::tempdir;

use browser_core::{CookiePolicy, HistoryMode};
use browser_storage::{load_config, BrowserConfig};

use super::{
    copy_on_select_enabled, force_osc52_enabled, resolve_cookie_policy, resolve_history_mode,
    resolve_mode, search_enabled, unwrap_tracking_enabled, ResolvedMode,
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
    let pair = resolve_cookie_policy(&config);
    assert_eq!(pair.first_party, CookiePolicy::Session);
    assert_eq!(pair.third_party, CookiePolicy::Allow);
}

#[test]
fn an_unrecognized_scope_word_resolves_to_reject_for_that_scope() {
    let config = config_with_cookies("nonsense", "allow");
    let pair = resolve_cookie_policy(&config);
    assert_eq!(pair.first_party, CookiePolicy::Reject);
    assert_eq!(pair.third_party, CookiePolicy::Allow);
}

#[test]
fn an_absent_cookies_section_resolves_to_reject_in_both_scopes() {
    let pair = resolve_cookie_policy(&BrowserConfig::default());
    assert_eq!(pair.first_party, CookiePolicy::Reject);
    assert_eq!(pair.third_party, CookiePolicy::Reject);
}
