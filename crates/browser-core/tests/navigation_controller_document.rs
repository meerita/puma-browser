// @file crates/browser-core/tests/navigation_controller_document.rs
// @description Tests for NavigationController::current_document().
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{BrowserUrl, NavigationController, NavigationSource};

#[test]
fn current_document_returns_none_before_loading() {
    let controller = NavigationController::new();
    assert!(controller.current_document().is_none());
}

#[tokio::test]
async fn current_document_returns_some_after_successful_load() {
    let mock_server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .and(wiremock::matchers::path("/"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string("<html><body><p>Hello</p></body></html>"),
        )
        .mount(&mock_server)
        .await;

    let url = BrowserUrl::parse(&mock_server.uri()).expect("wiremock URI must be valid");
    let mut controller = NavigationController::new();
    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("load must succeed");

    let document = controller.current_document();
    assert!(document.is_some());
    assert!(!document.unwrap().children().is_empty());
}
