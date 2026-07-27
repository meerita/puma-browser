// @file crates/browser-core/tests/local_file_load.rs
// @description End-to-end test: resolve a local HTML file, load it, and render a non-empty buffer.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{resolve_address, NavigationController, NavigationSource};
use tempfile::tempdir;

#[tokio::test]
async fn a_local_html_file_resolves_loads_and_renders_a_non_empty_buffer() {
    let working_directory = tempdir().expect("a temporary working directory must be created");
    std::fs::write(
        working_directory.path().join("page.html"),
        "<html><head><title>Local</title></head><body><p>Local file text</p></body></html>",
    )
    .expect("the temporary HTML file must be written");

    let url = resolve_address("page.html", working_directory.path())
        .expect("an existing local HTML file must resolve to a file URL");
    assert_eq!(url.scheme(), "file");

    let mut controller = NavigationController::new();
    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("loading a local HTML file must succeed");

    assert!(controller.has_page());
    assert_eq!(
        controller
            .current_title()
            .expect("the local page declared a title")
            .as_str(),
        "Local"
    );
    let buffer = controller
        .render(80)
        .expect("a loaded local page must render");
    assert!(
        buffer.height() > 0,
        "a local page with content must produce rows"
    );
    assert_eq!(buffer.width(), 80);
}
