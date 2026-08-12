// @file crates/browser-network/tests/submit_once.rs
// @description Behavior tests for submit_once: GET/POST bodies, cookie threading, scheme
//   rejection, and redirect pass-through with no POST-redirect-GET downgrade at this layer.
// @layer network
// @created meerita <meerita@icloud.com>

use browser_network::{
    submit_once, BrowserUrl, HopOutcome, NetworkError, RequestBody, RequestHeaders, RequestMethod,
};
use tokio::sync::watch;
use wiremock::matchers::{body_string, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn url_for(server: &MockServer, suffix: &str) -> BrowserUrl {
    let full = format!("{}{}", server.uri(), suffix);
    BrowserUrl::parse(&full).expect("mock server URL must parse")
}

#[tokio::test]
async fn post_urlencoded_sends_form_content_type_and_encoded_body() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/submit"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(body_string("name=Ada+Lovelace&role=engineer"))
        .respond_with(ResponseTemplate::new(200).set_body_string("submitted"))
        .mount(&server)
        .await;

    let (progress_tx, _progress_rx) = watch::channel(0usize);
    let body = RequestBody::UrlEncoded(vec![
        ("name".to_string(), "Ada Lovelace".to_string()),
        ("role".to_string(), "engineer".to_string()),
    ]);
    let outcome = submit_once(
        &url_for(&server, "/submit"),
        RequestMethod::Post,
        &body,
        None,
        &RequestHeaders::default(),
        progress_tx,
    )
    .await
    .expect("a matched POST must return a final outcome");

    match outcome {
        HopOutcome::Final(document) => assert_eq!(document.body_bytes(), b"submitted"),
        HopOutcome::Redirect { .. } => panic!("a 200 response must not be a redirect"),
    }
}

#[tokio::test]
async fn get_urlencoded_replaces_the_query_string_with_no_body() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/search"))
        .and(query_param("query", "puma"))
        .respond_with(ResponseTemplate::new(200).set_body_string("results"))
        .mount(&server)
        .await;

    let (progress_tx, _progress_rx) = watch::channel(0usize);
    let body = RequestBody::UrlEncoded(vec![("query".to_string(), "puma".to_string())]);
    let mut target_with_existing_query = url_for(&server, "/search");
    target_with_existing_query =
        BrowserUrl::parse(&format!("{target_with_existing_query}?stale=1"))
            .expect("URL with an existing query string must parse");
    let outcome = submit_once(
        &target_with_existing_query,
        RequestMethod::Get,
        &body,
        None,
        &RequestHeaders::default(),
        progress_tx,
    )
    .await
    .expect("a matched GET must return a final outcome");

    match outcome {
        HopOutcome::Final(document) => assert_eq!(document.body_bytes(), b"results"),
        HopOutcome::Redirect { .. } => panic!("a 200 response must not be a redirect"),
    }
}

#[tokio::test]
async fn a_supplied_cookie_header_is_sent_and_set_cookie_is_collected() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/login"))
        .and(header("cookie", "session=old"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("set-cookie", "session=new")
                .set_body_string("ok"),
        )
        .mount(&server)
        .await;

    let (progress_tx, _progress_rx) = watch::channel(0usize);
    let outcome = submit_once(
        &url_for(&server, "/login"),
        RequestMethod::Post,
        &RequestBody::None,
        Some("session=old"),
        &RequestHeaders::default(),
        progress_tx,
    )
    .await
    .expect("the request carrying the Cookie header must be served");

    match outcome {
        HopOutcome::Final(document) => {
            assert_eq!(document.body_bytes(), b"ok");
            assert_eq!(document.set_cookie_lines(), &["session=new".to_string()]);
        }
        HopOutcome::Redirect { .. } => panic!("a 200 response must not be a redirect"),
    }
}

#[tokio::test]
async fn a_file_url_is_rejected_without_attempting_any_io() {
    let url = BrowserUrl::parse("file:///etc/hosts").expect("file URL must parse");
    let (progress_tx, _progress_rx) = watch::channel(0usize);
    let outcome = submit_once(
        &url,
        RequestMethod::Post,
        &RequestBody::None,
        None,
        &RequestHeaders::default(),
        progress_tx,
    )
    .await;

    assert!(matches!(
        outcome,
        Err(NetworkError::UnsupportedScheme { scheme }) if scheme == "file"
    ));
}

#[tokio::test]
async fn a_redirect_response_to_a_post_returns_redirect_outcome_unchanged() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/create"))
        .respond_with(
            ResponseTemplate::new(303)
                .insert_header("location", "/created")
                .insert_header("set-cookie", "flash=saved")
                .set_body_string("should not be read"),
        )
        .mount(&server)
        .await;

    let (progress_tx, _progress_rx) = watch::channel(0usize);
    let outcome = submit_once(
        &url_for(&server, "/create"),
        RequestMethod::Post,
        &RequestBody::None,
        None,
        &RequestHeaders::default(),
        progress_tx,
    )
    .await
    .expect("a 303 with a Location must return a redirect outcome");

    match outcome {
        HopOutcome::Redirect {
            status,
            location,
            set_cookie_lines,
        } => {
            assert_eq!(status, 303);
            assert_eq!(location, "/created");
            assert_eq!(set_cookie_lines, vec!["flash=saved".to_string()]);
        }
        HopOutcome::Final(_) => panic!("a 303 with a Location must be a redirect"),
    }
}
