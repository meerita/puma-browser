// @file crates/browser-core/tests/search_engine.rs
// @description Behavior tests for SearchEngine result-URL construction.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::SearchEngine;

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
