// @file crates/browser-network/tests/fetch_once.rs
// @description Behavior tests for fetch_once: Set-Cookie capture, redirect outcome, outgoing Cookie header.
// @layer network
// @created meerita <meerita@icloud.com>

use browser_network::{fetch_once, BrowserUrl, HopOutcome};
use tokio::sync::watch;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn url_for(server: &MockServer, suffix: &str) -> BrowserUrl {
    let full = format!("{}{}", server.uri(), suffix);
    BrowserUrl::parse(&full).expect("mock server URL must parse")
}

#[tokio::test]
async fn a_final_response_captures_every_set_cookie_line_verbatim() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("set-cookie", "a=1")
                .append_header("set-cookie", "b=2")
                .set_body_string("page"),
        )
        .mount(&server)
        .await;

    let (progress_tx, _progress_rx) = watch::channel(0usize);
    let outcome = fetch_once(&url_for(&server, "/"), None, progress_tx)
        .await
        .expect("a 200 response must return a final outcome");

    match outcome {
        HopOutcome::Final(document) => {
            assert_eq!(
                document.set_cookie_lines(),
                &["a=1".to_string(), "b=2".to_string()]
            );
            assert_eq!(document.body_bytes(), b"page");
        }
        HopOutcome::Redirect { .. } => panic!("a 200 response must not be a redirect"),
    }
}

#[tokio::test]
async fn a_redirect_response_returns_status_location_and_set_cookie_without_a_body() {
    let server = MockServer::start().await;
    // A body is attached to prove the redirect path never reads it: the Redirect
    // variant carries no body, so the response is discarded after the headers.
    Mock::given(method("GET"))
        .and(path("/login"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", "/dashboard")
                .insert_header("set-cookie", "session=abc")
                .set_body_string("should not be read"),
        )
        .mount(&server)
        .await;

    let (progress_tx, _progress_rx) = watch::channel(0usize);
    let outcome = fetch_once(&url_for(&server, "/login"), None, progress_tx)
        .await
        .expect("a 302 with a Location must return a redirect outcome");

    match outcome {
        HopOutcome::Redirect {
            status,
            location,
            set_cookie_lines,
        } => {
            assert_eq!(status, 302);
            assert_eq!(location, "/dashboard");
            assert_eq!(set_cookie_lines, vec!["session=abc".to_string()]);
        }
        HopOutcome::Final(_) => panic!("a 302 with a Location must be a redirect"),
    }
}

#[tokio::test]
async fn a_supplied_cookie_header_is_sent_on_the_request() {
    let server = MockServer::start().await;
    // The mock matches only when the request carries the exact Cookie header, so a
    // matched response (body "ok") proves the header was sent verbatim.
    Mock::given(method("GET"))
        .and(path("/"))
        .and(header("cookie", "a=1"))
        .respond_with(ResponseTemplate::new(200).set_body_string("ok"))
        .mount(&server)
        .await;

    let (progress_tx, _progress_rx) = watch::channel(0usize);
    let outcome = fetch_once(&url_for(&server, "/"), Some("a=1"), progress_tx)
        .await
        .expect("the request carrying the Cookie header must be served");

    match outcome {
        HopOutcome::Final(document) => assert_eq!(document.body_bytes(), b"ok"),
        HopOutcome::Redirect { .. } => panic!("a 200 response must not be a redirect"),
    }
}
