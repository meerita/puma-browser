// @file crates/browser-network/tests/fetch.rs
// @description Behavior tests for fetch: success, redirects, redirect cap, size cap, lossy decode.
// @layer network
// @created meerita <meerita@icloud.com>

use browser_network::{fetch, BrowserUrl, NetworkError};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

fn url_for(server: &MockServer, suffix: &str) -> BrowserUrl {
    let full = format!("{}{}", server.uri(), suffix);
    BrowserUrl::parse(&full).expect("mock server URL must parse")
}

#[tokio::test]
async fn successful_response_returns_body_and_content_type() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw("<html><body>hello</body></html>", "text/html"),
        )
        .mount(&server)
        .await;

    let document = fetch(&url_for(&server, "/"))
        .await
        .expect("successful fetch must return a document");

    assert_eq!(document.body(), "<html><body>hello</body></html>");
    assert!(document.content_type().starts_with("text/html"));
}

#[tokio::test]
async fn redirect_within_limit_resolves_to_final_url_and_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/end"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/end"))
        .respond_with(ResponseTemplate::new(200).set_body_string("final page"))
        .mount(&server)
        .await;

    let document = fetch(&url_for(&server, "/start"))
        .await
        .expect("redirect within limit must resolve");

    assert_eq!(document.body(), "final page");
    assert!(document.final_url().as_str().ends_with("/end"));
}

#[tokio::test]
async fn redirect_beyond_limit_returns_too_many_redirects_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/loop"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/loop"))
        .mount(&server)
        .await;

    let outcome = fetch(&url_for(&server, "/loop")).await;

    assert!(matches!(outcome, Err(NetworkError::TooManyRedirects)));
}

#[tokio::test]
async fn body_larger_than_limit_returns_response_too_large_error() {
    let server = MockServer::start().await;
    let oversized = vec![b'a'; MAX_RESPONSE_BYTES + 1];
    Mock::given(method("GET"))
        .and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(oversized))
        .mount(&server)
        .await;

    let outcome = fetch(&url_for(&server, "/big")).await;

    assert!(matches!(outcome, Err(NetworkError::ResponseTooLarge)));
}

#[tokio::test]
async fn non_utf8_body_decodes_lossily_without_error() {
    let server = MockServer::start().await;
    let invalid_utf8 = vec![0xff, 0xfe, b'H', b'i'];
    Mock::given(method("GET"))
        .and(path("/binary"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(invalid_utf8))
        .mount(&server)
        .await;

    let document = fetch(&url_for(&server, "/binary"))
        .await
        .expect("non-UTF-8 body must decode lossily without error");

    assert!(document.body().contains('\u{FFFD}'));
    assert!(document.body().ends_with("Hi"));
}
