// @file crates/browser-core/tests/navigation_controller.rs
// @description Behavior tests for load and render: page storage, error mapping, blank page.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{BrowserUrl, CoreError, NavigationController};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Largest response body the network layer accepts, in bytes.
const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

fn url_for(server: &MockServer, suffix: &str) -> BrowserUrl {
    let full = format!("{}{}", server.uri(), suffix);
    BrowserUrl::parse(&full).expect("mock server URL must parse")
}

#[tokio::test]
async fn load_stores_the_fetched_page_and_renders_it() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(
            "<html><head><title>Hello</title></head><body><p>Some text here</p></body></html>",
            "text/html",
        ))
        .mount(&server)
        .await;
    let mut controller = NavigationController::new();

    controller
        .load(url_for(&server, "/"))
        .await
        .expect("loading a valid HTML page must succeed");

    assert!(controller.has_page());
    assert_eq!(
        controller
            .current_title()
            .expect("the page declared a title")
            .as_str(),
        "Hello"
    );
    let buffer = controller.render(80).expect("a loaded page must render");
    assert!(buffer.height() > 0, "a page with content must produce rows");
    assert_eq!(buffer.width(), 80);
}

#[tokio::test]
async fn load_decodes_the_body_using_the_content_type_charset() {
    let server = MockServer::start().await;
    // 0xE9 is `é` in windows-1252; the header charset must drive decoding at the parser.
    let mut body = b"<html><head><title>Caf".to_vec();
    body.push(0xE9);
    body.extend_from_slice(b"</title></head><body><p>x</p></body></html>");
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200).set_body_raw(body, "text/html; charset=windows-1252"),
        )
        .mount(&server)
        .await;
    let mut controller = NavigationController::new();

    controller
        .load(url_for(&server, "/"))
        .await
        .expect("loading a windows-1252 page must succeed");

    assert_eq!(
        controller
            .current_title()
            .expect("the page declared a title")
            .as_str(),
        "Café"
    );
}

#[tokio::test]
async fn load_of_an_oversized_body_surfaces_a_network_error() {
    let server = MockServer::start().await;
    let oversized = vec![b'a'; MAX_RESPONSE_BYTES + 1];
    Mock::given(method("GET"))
        .and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(oversized))
        .mount(&server)
        .await;
    let mut controller = NavigationController::new();

    let outcome = controller.load(url_for(&server, "/big")).await;

    assert!(matches!(outcome, Err(CoreError::Network(_))));
    assert!(!controller.has_page());
}

#[test]
fn render_without_a_loaded_page_returns_a_blank_buffer() {
    let controller = NavigationController::new();

    let buffer = controller
        .render(80)
        .expect("rendering a blank page must not error");

    assert!(!controller.has_page());
    assert_eq!(buffer.width(), 80);
    assert_eq!(buffer.height(), 0);
    assert_eq!(controller.script_count(), 0);
    assert!(controller.current_url().is_none());
}

#[tokio::test]
async fn render_at_zero_width_on_a_loaded_page_surfaces_a_layout_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("<html><body><p>text</p></body></html>", "text/html"),
        )
        .mount(&server)
        .await;
    let mut controller = NavigationController::new();
    controller
        .load(url_for(&server, "/"))
        .await
        .expect("loading a valid HTML page must succeed");

    let outcome = controller.render(0);

    assert!(matches!(outcome, Err(CoreError::Layout(_))));
}
