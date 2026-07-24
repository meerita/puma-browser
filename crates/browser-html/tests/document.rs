// @file crates/browser-html/tests/document.rs
// @description Behavior tests for the Document type constructor and accessors.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{Document, DocumentTitle, InlineRun, SemanticNode};

#[test]
fn document_exposes_its_children_and_title() {
    let children = vec![SemanticNode::Paragraph {
        runs: vec![InlineRun::plain("Body".to_string())],
        inline_style: None,
    }];
    let title = Some(DocumentTitle::new("Example Domain"));
    let document = Document::new(children, title, 0);

    assert_eq!(document.children().len(), 1);
    assert_eq!(
        document.title().map(DocumentTitle::as_str),
        Some("Example Domain")
    );
}

#[test]
fn document_without_title_reports_none() {
    let document = Document::new(Vec::new(), None, 0);
    assert!(document.title().is_none());
    assert!(document.children().is_empty());
}

#[test]
fn document_reports_its_suppressed_script_count() {
    let document = Document::new(Vec::new(), None, 3);
    assert_eq!(document.script_count(), 3);
}
