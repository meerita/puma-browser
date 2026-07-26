// @file crates/browser-layout/tests/anchor_spans.rs
// @description Behavior tests for CellBuffer anchor-span extraction produced during layout.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_html::{Document, InlineEmphasis, InlineRun, SemanticNode};
use browser_layout::{render_document, CellBuffer, WidthConfig};

fn document_of(nodes: Vec<SemanticNode>) -> Document {
    Document::new(nodes, None, 0)
}

fn anchored_run(text: &str, names: &[&str]) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: None,
        anchors: names.iter().map(|name| name.to_string()).collect(),
    }
}

fn paragraph_of(runs: Vec<InlineRun>) -> SemanticNode {
    SemanticNode::Paragraph {
        runs,
        inline_style: None,
    }
}

fn render(nodes: Vec<SemanticNode>) -> CellBuffer {
    render_document(&document_of(nodes), 40, &WidthConfig::default())
        .expect("document must lay out")
}

#[test]
fn single_anchor_produces_one_span_naming_it() {
    let buffer = render(vec![paragraph_of(vec![anchored_run("Body", &["intro"])])]);

    assert_eq!(buffer.anchors().len(), 1);
    assert_eq!(buffer.anchors()[0].name, "intro");
}

#[test]
fn two_names_on_one_run_produce_two_spans_on_the_same_row() {
    let buffer = render(vec![paragraph_of(vec![anchored_run(
        "Body",
        &["old", "new"],
    )])]);

    assert_eq!(buffer.anchors().len(), 2);
    let names: Vec<&str> = buffer
        .anchors()
        .iter()
        .map(|span| span.name.as_str())
        .collect();
    assert_eq!(names, vec!["old", "new"]);
    assert_eq!(buffer.anchors()[0].row, buffer.anchors()[1].row);
}

#[test]
fn a_run_with_no_anchor_produces_no_span() {
    let buffer = render(vec![paragraph_of(vec![InlineRun::plain(
        "plain body".to_string(),
    )])]);

    assert!(buffer.anchors().is_empty());
}

#[test]
fn spans_come_out_in_ascending_row_order() {
    let buffer = render(vec![
        paragraph_of(vec![anchored_run("First", &["a"])]),
        paragraph_of(vec![anchored_run("Second", &["b"])]),
    ]);

    assert_eq!(buffer.anchors().len(), 2);
    let first = &buffer.anchors()[0];
    let second = &buffer.anchors()[1];
    assert_eq!(first.name, "a");
    assert_eq!(second.name, "b");
    assert!(
        first.row < second.row,
        "an anchor earlier in the document must sit on an earlier row"
    );
}

#[test]
fn a_wrapped_anchored_run_records_the_row_of_its_first_line() {
    // Wider than the 40-column layout, so the run wraps onto more than one row.
    let long_text = "word ".repeat(20);
    let buffer = render(vec![
        paragraph_of(vec![anchored_run("Top", &["top-marker"])]),
        paragraph_of(vec![anchored_run(long_text.trim(), &["wrapped"])]),
    ]);

    let wrapped = buffer
        .anchors()
        .iter()
        .find(|span| span.name == "wrapped")
        .expect("the wrapped run's anchor must be recorded");
    let top = buffer
        .anchors()
        .iter()
        .find(|span| span.name == "top-marker")
        .expect("the top anchor must be recorded");
    // The wrapped anchor sits on the first row of its run, immediately after the first
    // paragraph, not on a later wrapped row.
    assert!(wrapped.row > top.row);
    assert_eq!(
        buffer
            .anchors()
            .iter()
            .filter(|span| span.name == "wrapped")
            .count(),
        1,
        "a run carrying one anchor produces exactly one span, however many rows it wraps to"
    );
}
