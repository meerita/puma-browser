// @file crates/browser-core/tests/navigation_target.rs
// @description Behavior tests for classify_navigation: same-page anchor vs page fetch.
// @layer core
// @created meerita <meerita@icloud.com>

use std::path::Path;

use browser_core::{classify_navigation, BrowserUrl, CoreError, NavigationTarget};

fn url(input: &str) -> BrowserUrl {
    BrowserUrl::parse(input).expect("test URL must parse")
}

fn classify(current: Option<&BrowserUrl>, target: &str) -> NavigationTarget {
    classify_navigation(current, target, Path::new("."))
        .expect("classification must succeed for this case")
}

fn same_page_fragment(target: NavigationTarget) -> Option<String> {
    match target {
        NavigationTarget::SamePageAnchor { fragment } => fragment,
        NavigationTarget::Fetch { .. } => panic!("expected a same-page anchor, got a fetch"),
    }
}

fn fetch_parts(target: NavigationTarget) -> (String, Option<String>) {
    match target {
        NavigationTarget::Fetch { url, fragment } => (url.as_str().to_string(), fragment),
        NavigationTarget::SamePageAnchor { .. } => {
            panic!("expected a fetch, got a same-page anchor")
        }
    }
}

#[test]
fn a_bare_fragment_on_a_loaded_page_is_a_same_page_anchor() {
    let current = url("https://example.test/page");
    let target = classify(Some(&current), "#section");

    assert_eq!(same_page_fragment(target), Some("section".to_string()));
}

#[test]
fn an_absolute_url_matching_the_current_base_is_a_same_page_anchor() {
    let current = url("https://example.test/page");
    let target = classify(Some(&current), "https://example.test/page#section");

    assert_eq!(same_page_fragment(target), Some("section".to_string()));
}

#[test]
fn a_url_with_a_different_path_is_a_fetch_carrying_the_fragment() {
    let current = url("https://example.test/page");
    let target = classify(Some(&current), "https://example.test/other#section");

    let (base, fragment) = fetch_parts(target);
    assert_eq!(base, "https://example.test/other");
    assert_eq!(fragment, Some("section".to_string()));
}

#[test]
fn a_url_with_a_different_path_and_no_fragment_is_a_fetch_with_no_fragment() {
    let current = url("https://example.test/page");
    let target = classify(Some(&current), "https://example.test/other");

    let (base, fragment) = fetch_parts(target);
    assert_eq!(base, "https://example.test/other");
    assert_eq!(fragment, None);
}

#[test]
fn an_empty_fragment_is_preserved_for_the_top_of_the_page() {
    let current = url("https://example.test/page");
    let target = classify(Some(&current), "#");

    assert_eq!(same_page_fragment(target), Some(String::new()));
}

#[test]
fn a_top_fragment_is_preserved_verbatim() {
    let current = url("https://example.test/page");
    let target = classify(Some(&current), "#top");

    assert_eq!(same_page_fragment(target), Some("top".to_string()));
}

#[test]
fn an_absolute_url_with_no_current_page_is_a_fetch() {
    let target = classify(None, "https://example.test/page");

    let (base, fragment) = fetch_parts(target);
    assert_eq!(base, "https://example.test/page");
    assert_eq!(fragment, None);
}

#[test]
fn a_bare_fragment_with_no_current_page_fails_to_navigate() {
    let error = classify_navigation(None, "#section", Path::new("."))
        .expect_err("a bare fragment cannot resolve without a current page");

    assert!(matches!(error, CoreError::NavigationFailed));
}

#[test]
fn an_unsupported_scheme_returns_a_network_error() {
    let current = url("https://example.test/page");
    let error = classify_navigation(Some(&current), "ftp://example.test/file", Path::new("."))
        .expect_err("an unsupported scheme must be rejected");

    assert!(matches!(error, CoreError::Network(_)));
}
