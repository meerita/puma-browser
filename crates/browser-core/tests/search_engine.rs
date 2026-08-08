// @file crates/browser-core/tests/search_engine.rs
// @description Behavior tests for SearchEngine result-URL construction.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{NavigationController, SearchEngine};

#[test]
fn default_engine_builds_a_duckduckgo_lite_result_url() {
    let engine = SearchEngine::default();
    let url = engine
        .result_url("rust")
        .expect("a simple query must build a result URL");
    assert_eq!(url.host_str(), Some("lite.duckduckgo.com"));
    assert!(
        url.as_str()
            .starts_with("https://lite.duckduckgo.com/lite/?q="),
        "got {}",
        url.as_str()
    );
}

#[test]
fn a_multi_word_query_is_percent_encoded_in_the_result_url() {
    let engine = SearchEngine::default();
    let url = engine
        .result_url("terminal browser")
        .expect("a multi-word query must build a result URL");
    assert!(
        url.as_str().contains("q=terminal+browser")
            || url.as_str().contains("q=terminal%20browser"),
        "the query must be percent-encoded: {}",
        url.as_str()
    );
}

#[test]
fn the_result_url_host_is_duckduckgo_lite() {
    let engine = SearchEngine::default();
    let url = engine
        .result_url("anything")
        .expect("a query must build a result URL");
    assert_eq!(url.host_str(), Some("lite.duckduckgo.com"));
}

#[test]
fn a_custom_https_engine_builds_a_result_url_with_the_configured_parameter() {
    let engine = SearchEngine::new(
        "https://search.example.com/results".to_string(),
        "query".to_string(),
    )
    .expect("a valid https base URL must build an engine");
    assert_eq!(engine.base_url(), "https://search.example.com/results");
    assert_eq!(engine.query_parameter(), "query");
    let url = engine
        .result_url("rust")
        .expect("a configured engine must build a result URL");
    assert_eq!(url.host_str(), Some("search.example.com"));
    assert!(
        url.as_str()
            .starts_with("https://search.example.com/results?query=rust"),
        "got {}",
        url.as_str()
    );
}

#[test]
fn a_file_scheme_base_url_is_rejected() {
    let engine = SearchEngine::new("file:///etc/passwd".to_string(), "q".to_string());
    assert!(
        engine.is_err(),
        "a non-http(s) base URL must be rejected at construction"
    );
}

#[test]
fn a_malformed_base_url_is_rejected() {
    let engine = SearchEngine::new("http://".to_string(), "q".to_string());
    assert!(
        engine.is_err(),
        "a malformed base URL must be rejected at construction"
    );
}

#[test]
fn the_controller_returns_the_seeded_search_engine() {
    let engine = SearchEngine::new(
        "https://search.example.com/results".to_string(),
        "query".to_string(),
    )
    .expect("a valid https base URL must build an engine");
    let controller = NavigationController::new().with_search_engine(engine);
    assert_eq!(
        controller.search_engine().base_url(),
        "https://search.example.com/results"
    );
    assert_eq!(controller.search_engine().query_parameter(), "query");
}

#[test]
fn the_controller_defaults_to_the_duckduckgo_lite_engine() {
    let controller = NavigationController::new();
    assert_eq!(
        controller.search_engine().base_url(),
        "https://lite.duckduckgo.com/lite/"
    );
}
