// @file crates/browser-core/tests/form_submission.rs
// @description Behavior tests for form field state and submission via submit_with_progress.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{BrowserUrl, CoreError, NavigationController, NavigationSource, NodeId};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn url_for(server: &MockServer, suffix: &str) -> BrowserUrl {
    let full = format!("{}{}", server.uri(), suffix);
    BrowserUrl::parse(&full).expect("mock server URL must parse")
}

async fn mount_html(server: &MockServer, page_path: &str, body: &str) {
    Mock::given(method("GET"))
        .and(path(page_path))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.to_string(), "text/html"))
        .mount(server)
        .await;
}

async fn mount_plain_page(server: &MockServer, page_path: &str) {
    mount_html(server, page_path, "<html><body><p>ok</p></body></html>").await;
}

async fn requests_to(server: &MockServer, suffix: &str) -> Vec<Request> {
    server
        .received_requests()
        .await
        .expect("request recording must be enabled on the mock server")
        .into_iter()
        .filter(|request| request.url.path() == suffix)
        .collect()
}

fn body_text(request: &Request) -> String {
    String::from_utf8_lossy(&request.body).into_owned()
}

async fn load_page(controller: &mut NavigationController, server: &MockServer, page_path: &str) {
    controller
        .load(url_for(server, page_path), NavigationSource::AddressBar)
        .await
        .expect("loading the test page must succeed");
}

#[tokio::test]
async fn a_get_form_submission_sends_encoded_fields_as_the_query_string() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/search" method="get">
                <input type="text" name="q" value="hello" />
                <input type="submit" name="go" value="Search" />
            </form>
        </body></html>"#,
    )
    .await;
    mount_plain_page(&server, "/search").await;

    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    controller
        .submit(NodeId::new(2))
        .await
        .expect("a GET form submission must succeed");

    let requests = requests_to(&server, "/search").await;
    assert_eq!(requests.len(), 1, "the search page must be requested once");
    let query = requests[0].url.query().unwrap_or_default();
    assert!(query.contains("q=hello"), "query was: {query}");
    assert!(query.contains("go=Search"), "query was: {query}");
    assert_eq!(
        controller
            .current_url()
            .expect("a page must be current")
            .host_str(),
        url_for(&server, "/search").host_str()
    );
}

#[tokio::test]
async fn a_post_form_submission_sends_urlencoded_fields_as_the_body() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="text" name="name" value="ada" />
                <button type="submit" name="go" value="1">Go</button>
            </form>
        </body></html>"#,
    )
    .await;
    mount_plain_page(&server, "/submit").await;

    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    controller
        .submit(NodeId::new(2))
        .await
        .expect("a POST form submission must succeed");

    let requests = requests_to(&server, "/submit").await;
    assert_eq!(
        requests.len(),
        1,
        "the submit endpoint must be requested once"
    );
    assert_eq!(requests[0].method.as_str(), "POST");
    let body = body_text(&requests[0]);
    assert!(body.contains("name=ada"), "body was: {body}");
    assert!(body.contains("go=1"), "body was: {body}");
    assert_eq!(
        requests[0]
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("application/x-www-form-urlencoded")
    );
}

#[tokio::test]
async fn an_unchecked_checkbox_is_absent_and_a_valueless_checked_checkbox_submits_on() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="checkbox" name="subscribe" />
                <input type="checkbox" name="agree" checked />
                <button type="submit" name="go" value="1">Go</button>
            </form>
        </body></html>"#,
    )
    .await;
    mount_plain_page(&server, "/submit").await;

    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    controller
        .submit(NodeId::new(3))
        .await
        .expect("submission must succeed");

    let requests = requests_to(&server, "/submit").await;
    let body = body_text(&requests[0]);
    assert!(
        !body.contains("subscribe"),
        "an unchecked checkbox must not be submitted; body was: {body}"
    );
    assert!(body.contains("agree=on"), "body was: {body}");
}

#[tokio::test]
async fn selecting_one_radio_unchecks_its_siblings_sharing_the_same_name() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="radio" name="color" value="red" checked />
                <input type="radio" name="color" value="blue" />
            </form>
        </body></html>"#,
    )
    .await;

    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    let red_id = NodeId::new(1);
    let blue_id = NodeId::new(2);
    assert!(controller
        .field_values()
        .expect("a page must be current")
        .is_checked(red_id));

    controller
        .select_radio(blue_id)
        .expect("selecting a known radio must succeed");

    let field_values = controller
        .field_values()
        .expect("a page must still be current");
    assert!(
        !field_values.is_checked(red_id),
        "selecting a sibling radio must uncheck the previously checked one"
    );
    assert!(field_values.is_checked(blue_id));
}

#[tokio::test]
async fn a_hidden_inputs_value_round_trips_into_the_submitted_pairs_unmodified() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="hidden" name="csrf" value="tok123" />
                <button type="submit" name="go" value="1">Go</button>
            </form>
        </body></html>"#,
    )
    .await;
    mount_plain_page(&server, "/submit").await;

    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    controller
        .submit(NodeId::new(2))
        .await
        .expect("submission must succeed");

    let requests = requests_to(&server, "/submit").await;
    let body = body_text(&requests[0]);
    assert!(
        body.contains("csrf=tok123"),
        "a hidden field must round-trip unmodified without user interaction; body was: {body}"
    );
}

#[tokio::test]
async fn a_password_fields_value_reaches_the_request_but_never_a_debug_print() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="password" name="pw" />
                <button type="submit" name="go" value="1">Go</button>
            </form>
        </body></html>"#,
    )
    .await;
    mount_plain_page(&server, "/submit").await;

    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    controller
        .set_sensitive_field_text(NodeId::new(1), "hunter2".to_string())
        .expect("setting a known sensitive field must succeed");

    let debug_output = format!("{controller:?}");
    assert!(
        !debug_output.contains("hunter2"),
        "a Debug print must never leak a sensitive field's value"
    );

    controller
        .submit(NodeId::new(2))
        .await
        .expect("submission must succeed");

    let requests = requests_to(&server, "/submit").await;
    let body = body_text(&requests[0]);
    assert!(body.contains("pw=hunter2"), "body was: {body}");

    let debug_output_after_submission = format!("{controller:?}");
    assert!(!debug_output_after_submission.contains("hunter2"));
}

#[tokio::test]
async fn a_post_form_submission_redirected_with_302_follows_up_with_a_bodyless_get() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/create" method="post">
                <input type="text" name="title" value="new" />
                <button type="submit" name="go" value="1">Go</button>
            </form>
        </body></html>"#,
    )
    .await;
    Mock::given(method("POST"))
        .and(path("/create"))
        .respond_with(ResponseTemplate::new(302).append_header("location", "/created"))
        .mount(&server)
        .await;
    mount_plain_page(&server, "/created").await;

    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    controller
        .submit(NodeId::new(2))
        .await
        .expect("the redirect chain must resolve to the created page");

    let followup_requests = requests_to(&server, "/created").await;
    assert_eq!(followup_requests.len(), 1);
    assert_eq!(followup_requests[0].method.as_str(), "GET");
    assert!(
        followup_requests[0].body.is_empty(),
        "a POST-redirect-GET downgrade must carry no body"
    );
}

#[tokio::test]
async fn submitting_with_an_unknown_node_id_returns_field_not_found() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="text" name="name" value="ada" />
                <button type="submit" name="go" value="1">Go</button>
            </form>
        </body></html>"#,
    )
    .await;

    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    let result = controller.submit(NodeId::new(9999)).await;
    assert!(matches!(result, Err(CoreError::FieldNotFound)));
}
