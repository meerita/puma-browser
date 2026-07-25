// @file crates/browser-layout/tests/link_spans.rs
// @description Behavior tests for CellBuffer link-span extraction produced during layout.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_html::{Document, InlineEmphasis, InlineRun, SemanticNode};
use browser_layout::{render_document, CellBuffer, WidthConfig};

fn document_of(nodes: Vec<SemanticNode>) -> Document {
    Document::new(nodes, None, 0)
}

fn link_run(text: &str, url: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: Some(url.to_string()),
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

/// The column of the first cell whose grapheme equals `needle` on `row`, if any.
fn column_of(buffer: &CellBuffer, row: u16, needle: &str) -> Option<u16> {
    (0..buffer.width()).find(|column| {
        buffer
            .cell_at(*column, row)
            .map(|cell| cell.grapheme() == needle)
            .unwrap_or(false)
    })
}

#[test]
fn single_link_produces_one_span() {
    let buffer = render(vec![paragraph_of(vec![link_run("X", "https://a.test/")])]);

    assert_eq!(buffer.links().len(), 1);
    let span = &buffer.links()[0];
    assert_eq!(span.url, "https://a.test/");
    assert!(span.col_start <= span.col_end);
}

#[test]
fn two_links_produce_two_spans() {
    let buffer = render(vec![paragraph_of(vec![
        link_run("AB", "https://a.test/"),
        InlineRun::plain(" ".to_string()),
        link_run("CD", "https://b.test/"),
    ])]);

    assert_eq!(buffer.links().len(), 2);
    let urls: Vec<&str> = buffer
        .links()
        .iter()
        .map(|span| span.url.as_str())
        .collect();
    assert_eq!(urls, vec!["https://a.test/", "https://b.test/"]);
}

#[test]
fn text_only_paragraph_produces_no_spans() {
    let buffer = render(vec![paragraph_of(vec![InlineRun::plain(
        "plain text with no links".to_string(),
    )])]);

    assert!(buffer.links().is_empty());
}

#[test]
fn link_at_end_of_run_closes_span() {
    let buffer = render(vec![paragraph_of(vec![
        link_run("AB", "https://a.test/"),
        InlineRun::plain(" C".to_string()),
    ])]);

    assert_eq!(buffer.links().len(), 1);
    let span = &buffer.links()[0];
    let c_column = column_of(&buffer, span.row, "C").expect("the trailing C must be rendered");
    assert!(
        span.col_end < c_column,
        "the link span must close before the non-link character that follows it"
    );
}

#[test]
fn link_url_not_exposed_on_cell() {
    let buffer = render(vec![paragraph_of(vec![link_run("X", "https://a.test/")])]);
    let cell = buffer.cell_at(0, 0).expect("the first cell must exist");

    // The public Cell surface is grapheme/foreground/background/emphasis/underline only.
    // There is no public accessor for the link URL, so remote-sourced URLs never leak
    // through the cell API; the link is observable only as a LinkSpan on the buffer.
    let _ = cell.grapheme();
    let _ = cell.foreground();
    let _ = cell.background();
    let _ = cell.emphasis();
    let _ = cell.underline();
    assert_eq!(cell.grapheme(), "X");
}
