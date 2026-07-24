// @file crates/browser-html/tests/base_url_resolution.rs
// @description Behavior tests for parse_html_with_base: relative references resolve against the document URL.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html_with_base, SemanticNode};

/// The `link` target of the first link run in the first paragraph of `source`.
fn first_link(source: &str, document_url: Option<&str>) -> Option<String> {
    let document = parse_html_with_base(source.as_bytes(), None, document_url)
        .expect("well-formed HTML must parse");
    document.children().iter().find_map(|node| match node {
        SemanticNode::Paragraph { runs, .. } => runs.iter().find_map(|run| run.link.clone()),
        _ => None,
    })
}

/// The `source` of the first image placeholder in `source`.
fn first_image_source(source: &str, document_url: Option<&str>) -> Option<String> {
    let document = parse_html_with_base(source.as_bytes(), None, document_url)
        .expect("well-formed HTML must parse");
    document.children().iter().find_map(|node| match node {
        SemanticNode::ImagePlaceholder { source, .. } => source.clone(),
        _ => None,
    })
}

#[test]
fn relative_link_resolves_against_the_document_url_when_no_base_element_is_present() {
    let link = first_link(
        r#"<p><a href="page.html">next</a></p>"#,
        Some("https://example.com/docs/index.html"),
    );

    assert_eq!(link, Some("https://example.com/docs/page.html".to_string()));
}

#[test]
fn relative_image_source_resolves_against_the_document_url() {
    let source = first_image_source(
        r#"<img alt="A diagram" src="pic.png">"#,
        Some("https://example.com/docs/index.html"),
    );

    assert_eq!(source, Some("https://example.com/docs/pic.png".to_string()));
}

#[test]
fn a_base_href_overrides_the_document_url() {
    let link = first_link(
        r#"<base href="https://cdn.example.com/assets/"><p><a href="page.html">next</a></p>"#,
        Some("https://example.com/docs/index.html"),
    );

    assert_eq!(
        link,
        Some("https://cdn.example.com/assets/page.html".to_string())
    );
}

#[test]
fn a_relative_base_href_resolves_against_the_document_url() {
    let link = first_link(
        r#"<base href="../assets/"><p><a href="page.html">next</a></p>"#,
        Some("https://example.com/docs/index.html"),
    );

    assert_eq!(
        link,
        Some("https://example.com/assets/page.html".to_string())
    );
}

#[test]
fn a_relative_reference_is_kept_as_authored_when_no_base_and_no_document_url_exist() {
    let link = first_link(r#"<p><a href="page.html">next</a></p>"#, None);

    assert_eq!(link, Some("page.html".to_string()));
}

#[test]
fn a_local_file_document_url_resolves_a_sibling_reference_to_a_file_url() {
    let link = first_link(
        r#"<p><a href="sibling.html">next</a></p>"#,
        Some("file:///home/user/site/index.html"),
    );

    assert_eq!(
        link,
        Some("file:///home/user/site/sibling.html".to_string())
    );
}
