// @file crates/browser-core/tests/history.rs
// @description Behavior tests for the back-history stack on NavigationController.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{BrowserUrl, NavigationController};
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

fn url_for(server: &MockServer, suffix: &str) -> BrowserUrl {
    let full = format!("{}{}", server.uri(), suffix);
    BrowserUrl::parse(&full).expect("mock server URL must parse")
}

/// Mounts a catch-all GET handler that answers every path with a small HTML document, so
/// distinct paths load as distinct pages without a mock per path.
async fn mount_any_page(server: &MockServer) {
    Mock::given(method("GET"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("<html><body><p>page</p></body></html>", "text/html"),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn go_back_after_single_load_restores_previous_page() {
    let server = MockServer::start().await;
    mount_any_page(&server).await;
    let mut controller = NavigationController::new();
    controller
        .load(url_for(&server, "/a"))
        .await
        .expect("loading page A must succeed");
    controller
        .load(url_for(&server, "/b"))
        .await
        .expect("loading page B must succeed");

    let restored = controller.go_back();

    assert!(restored, "go_back must restore the previous page");
    assert_eq!(
        controller
            .current_url()
            .expect("a page must be current after go_back")
            .as_str(),
        url_for(&server, "/a").as_str()
    );
}

#[test]
fn go_back_when_stack_is_empty_returns_false() {
    let mut controller = NavigationController::new();

    let restored = controller.go_back();

    assert!(!restored, "go_back on an empty stack must return false");
    assert!(controller.current_url().is_none());
}

#[tokio::test]
async fn can_go_back_reflects_stack_depth() {
    let server = MockServer::start().await;
    mount_any_page(&server).await;
    let mut controller = NavigationController::new();
    assert!(
        !controller.can_go_back(),
        "a fresh controller has no history"
    );

    controller
        .load(url_for(&server, "/first"))
        .await
        .expect("loading the first page must succeed");
    assert!(
        !controller.can_go_back(),
        "one loaded page leaves nothing to go back to"
    );

    controller
        .load(url_for(&server, "/second"))
        .await
        .expect("loading the second page must succeed");
    assert!(
        controller.can_go_back(),
        "a second load makes the first page available"
    );
}

#[tokio::test]
async fn history_stack_is_capped_at_50() {
    let server = MockServer::start().await;
    mount_any_page(&server).await;
    let mut controller = NavigationController::new();
    for page_number in 1..=51 {
        controller
            .load(url_for(&server, &format!("/page/{page_number}")))
            .await
            .expect("each page must load");
    }

    assert!(controller.can_go_back());
    let mut back_steps = 0;
    while controller.go_back() {
        back_steps += 1;
    }
    assert_eq!(
        back_steps, 50,
        "the history stack must retain at most 50 pages"
    );
}
