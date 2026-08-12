// @file crates/browser-layout/tests/render_cascade.rs
// @description Verifies the layout consumes the cascade: hidden nodes vanish and inline color reaches cells.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::Color;
use browser_html::{Document, InlineRun, SemanticNode};
use browser_layout::{render_document, Cell, CellBuffer, WidthConfig};

const CONTENT_WIDTH: u16 = 20;

/// The first cell in the buffer whose grapheme matches, scanning every row and column.
fn cell_with_grapheme<'buffer>(
    buffer: &'buffer CellBuffer,
    grapheme: &str,
) -> Option<&'buffer Cell> {
    (0..buffer.height())
        .flat_map(|row| (0..buffer.width()).map(move |column| (column, row)))
        .filter_map(|(column, row)| buffer.cell_at(column, row))
        .find(|cell| cell.grapheme() == grapheme)
}

fn document_of(children: Vec<SemanticNode>) -> Document {
    Document::new(children, None, 0)
}

fn paragraph(text: &str, inline_style: Option<&str>) -> SemanticNode {
    SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(String::from(text))],
        inline_style: inline_style.map(String::from),
    }
}

#[test]
fn a_display_none_paragraph_produces_no_rows() {
    let document = document_of(vec![paragraph("hidden text", Some("display: none"))]);

    let buffer = render_document(&document, CONTENT_WIDTH, &WidthConfig::default(), None)
        .expect("layout must succeed");

    assert_eq!(buffer.height(), 0);
}

#[test]
fn a_hidden_paragraph_does_not_suppress_its_visible_sibling() {
    let document = document_of(vec![
        paragraph("hidden", Some("display: none")),
        paragraph("shown", None),
    ]);

    let buffer = render_document(&document, CONTENT_WIDTH, &WidthConfig::default(), None)
        .expect("layout must succeed");

    assert_eq!(buffer.height(), 2); // 1 content row + 1 blank from paragraph spacing_after
    assert_eq!(buffer.cell_at(0, 0).map(|cell| cell.grapheme()), Some("s"));
}

#[test]
fn an_inline_color_reaches_the_rendered_cells() {
    let document = document_of(vec![paragraph("hello", Some("color: red"))]);

    let buffer = render_document(&document, CONTENT_WIDTH, &WidthConfig::default(), None)
        .expect("layout must succeed");

    let first = buffer.cell_at(0, 0).expect("first cell must exist");
    assert_eq!(first.grapheme(), "h");
    assert_eq!(first.foreground(), Some(Color::Red));
}

#[test]
fn an_inherited_color_reaches_a_child_paragraph_cell() {
    let inner = paragraph("quoted", None);
    let document = document_of(vec![SemanticNode::Quote {
        children: vec![inner],
        inline_style: Some(String::from("color: green")),
    }]);

    let buffer = render_document(&document, CONTENT_WIDTH, &WidthConfig::default(), None)
        .expect("layout must succeed");

    let colored = cell_with_grapheme(&buffer, "q").expect("quoted text must be rendered");
    assert_eq!(colored.foreground(), Some(Color::Green));
}

#[test]
fn uppercase_text_transform_is_applied_to_rendered_text() {
    let document = document_of(vec![paragraph("hi", Some("text-transform: uppercase"))]);

    let buffer = render_document(&document, CONTENT_WIDTH, &WidthConfig::default(), None)
        .expect("layout must succeed");

    assert_eq!(buffer.cell_at(0, 0).map(|cell| cell.grapheme()), Some("H"));
    assert_eq!(buffer.cell_at(1, 0).map(|cell| cell.grapheme()), Some("I"));
}
