// @file crates/browser-core/tests/render_field_overlay.rs
// @description Behavior tests for the live field-value overlay render() builds from FormFieldValues.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{BrowserUrl, CellBuffer, NavigationController, NavigationSource, NodeId};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

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

async fn load_page(controller: &mut NavigationController, server: &MockServer, page_path: &str) {
    controller
        .load(url_for(server, page_path), NavigationSource::AddressBar)
        .await
        .expect("loading the test page must succeed");
}

fn buffer_text(buffer: &CellBuffer) -> String {
    (0..buffer.height())
        .map(|row| {
            (0..buffer.width())
                .filter_map(|column| buffer.cell_at(column, row))
                .map(|cell| cell.grapheme())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn setting_a_text_fields_value_renders_the_new_text() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="text" name="name" value="ada" />
            </form>
        </body></html>"#,
    )
    .await;
    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    let before = buffer_text(&controller.render(80).expect("page must render"));
    assert!(before.contains("ada"));

    controller
        .set_field_text(NodeId::new(1), "grace".to_string())
        .expect("setting a known field must succeed");

    let after = buffer_text(&controller.render(80).expect("page must render"));
    assert!(after.contains("grace"));
    assert!(!after.contains("ada"));
}

#[tokio::test]
async fn a_sensitive_fields_live_length_renders_as_a_mask_never_the_typed_value() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="password" name="pw" />
            </form>
        </body></html>"#,
    )
    .await;
    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    controller
        .set_sensitive_field_text(NodeId::new(1), "hunter2".to_string())
        .expect("setting a known sensitive field must succeed");

    let rendered = buffer_text(&controller.render(80).expect("page must render"));
    assert!(
        !rendered.contains("hunter2"),
        "the typed value must never reach the rendered text"
    );
    assert!(
        rendered.contains("•••••••"),
        "the mask length must match the typed length; rendered was: {rendered}"
    );
}

#[tokio::test]
async fn toggling_a_checkbox_renders_the_checked_marker() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="checkbox" name="subscribe" />
            </form>
        </body></html>"#,
    )
    .await;
    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    let before = buffer_text(&controller.render(80).expect("page must render"));
    assert!(before.contains("[ ]"));

    controller
        .toggle_checkbox(NodeId::new(1))
        .expect("toggling a known checkbox must succeed");

    let after = buffer_text(&controller.render(80).expect("page must render"));
    assert!(after.contains("[x]"));
}

#[tokio::test]
async fn resetting_a_form_restores_its_parsed_defaults() {
    let server = MockServer::start().await;
    mount_html(
        &server,
        "/page",
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="text" name="name" value="ada" />
                <input type="checkbox" name="subscribe" checked />
            </form>
        </body></html>"#,
    )
    .await;
    let mut controller = NavigationController::new();
    load_page(&mut controller, &server, "/page").await;

    let text_id = NodeId::new(1);
    let checkbox_id = NodeId::new(2);
    controller
        .set_field_text(text_id, "grace".to_string())
        .expect("setting a known field must succeed");
    controller
        .toggle_checkbox(checkbox_id)
        .expect("toggling a known checkbox must succeed");

    controller
        .reset_form(text_id)
        .expect("resetting a known form must succeed");

    let field_values = controller.field_values().expect("a page must be current");
    assert_eq!(field_values.text(text_id), Some("ada"));
    assert!(field_values.is_checked(checkbox_id));
}
