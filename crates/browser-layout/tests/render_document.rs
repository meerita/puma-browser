// @file crates/browser-layout/tests/render_document.rs
// @description Behavior tests for render_document: wrapping, markers, verbatim code, widths.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_html::{Document, InlineRun, SemanticNode};
use browser_layout::{render_document, CellBuffer, LayoutError};
use unicode_width::UnicodeWidthStr;

fn document_of(nodes: Vec<SemanticNode>) -> Document {
    Document::new(nodes, None, 0)
}

fn paragraph(text: &str) -> SemanticNode {
    SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(text.to_string())],
    }
}

fn list_item(text: &str) -> SemanticNode {
    SemanticNode::ListItem {
        children: vec![paragraph(text)],
    }
}

fn row_text(buffer: &CellBuffer, row: u16) -> String {
    (0..buffer.width())
        .filter_map(|column| buffer.cell_at(column, row))
        .map(|cell| cell.grapheme())
        .collect()
}

#[test]
fn zero_width_returns_zero_width_error() {
    let document = document_of(vec![paragraph("hello")]);

    let outcome = render_document(&document, 0);

    assert!(matches!(outcome, Err(LayoutError::ZeroWidth)));
}

#[test]
fn long_paragraph_wraps_so_no_row_exceeds_the_width() {
    let width = 10u16;
    let words = vec!["word"; 40].join(" ");
    let document = document_of(vec![paragraph(&words)]);

    let buffer = render_document(&document, width).expect("paragraph must lay out");

    assert!(
        buffer.height() > 1,
        "a long paragraph must wrap onto many rows"
    );
    for row in 0..buffer.height() {
        let text = row_text(&buffer, row);
        let columns = UnicodeWidthStr::width(text.trim_end());
        assert!(columns <= usize::from(width), "row {row} exceeds the width");
    }
}

#[test]
fn heading_and_two_list_items_produce_expected_rows_and_bullets() {
    let document = document_of(vec![
        SemanticNode::Heading {
            level: 1,
            runs: vec![InlineRun::plain(String::from("Title"))],
        },
        SemanticNode::List {
            ordered: false,
            children: vec![list_item("one"), list_item("two")],
        },
    ]);

    let buffer = render_document(&document, 40).expect("document must lay out");

    // One blank row before and after the heading, then one row per list item.
    assert_eq!(buffer.height(), 5);
    assert_eq!(row_text(&buffer, 1).trim_end(), "Title");
    assert_eq!(buffer.cell_at(0, 3).expect("bullet cell").grapheme(), "•");
    assert_eq!(buffer.cell_at(0, 4).expect("bullet cell").grapheme(), "•");
    assert_eq!(row_text(&buffer, 3).trim_end(), "• one");
}

#[test]
fn code_block_is_rendered_verbatim_and_clipped_not_wrapped() {
    let document = document_of(vec![SemanticNode::CodeBlock {
        text: String::from("abcdefghij\nkl"),
    }]);

    let buffer = render_document(&document, 5).expect("code block must lay out");

    // Two source lines stay two rows; the long line is clipped to the width, not wrapped.
    assert_eq!(buffer.height(), 2);
    assert_eq!(row_text(&buffer, 0), "abcde");
    assert_eq!(row_text(&buffer, 1).trim_end(), "kl");
}

#[test]
fn combining_mark_grapheme_occupies_a_single_cell() {
    let document = document_of(vec![paragraph("e\u{0301}x")]);

    let buffer = render_document(&document, 10).expect("paragraph must lay out");

    assert_eq!(
        buffer.cell_at(0, 0).expect("cluster cell").grapheme(),
        "e\u{0301}"
    );
    assert_eq!(buffer.cell_at(1, 0).expect("next cell").grapheme(), "x");
}

#[test]
fn double_width_grapheme_advances_two_columns() {
    let document = document_of(vec![paragraph("x界y")]);

    let buffer = render_document(&document, 10).expect("paragraph must lay out");

    assert_eq!(buffer.cell_at(0, 0).expect("first cell").grapheme(), "x");
    assert_eq!(buffer.cell_at(1, 0).expect("wide cell").grapheme(), "界");
    // The column the wide grapheme spans into stays blank; the next grapheme is at 3.
    assert_eq!(buffer.cell_at(2, 0).expect("spanned cell").grapheme(), " ");
    assert_eq!(
        buffer.cell_at(3, 0).expect("following cell").grapheme(),
        "y"
    );
}

#[test]
fn representative_document_renders_to_the_expected_rows() {
    let document = document_of(vec![
        SemanticNode::Heading {
            level: 1,
            runs: vec![InlineRun::plain(String::from("Title"))],
        },
        paragraph("Body text"),
        SemanticNode::List {
            ordered: false,
            children: vec![list_item("one"), list_item("two")],
        },
        SemanticNode::Quote {
            children: vec![paragraph("Quoted")],
        },
        SemanticNode::Separator,
    ]);

    let buffer = render_document(&document, 20).expect("document must lay out");

    let rows: Vec<String> = (0..buffer.height())
        .map(|row| row_text(&buffer, row).trim_end().to_string())
        .collect();

    assert_eq!(
        rows,
        vec![
            String::new(),         // heading spacing before
            String::from("Title"), // heading
            String::new(),         // heading spacing after
            String::from("Body text"),
            String::from("• one"), // list, no surrounding spacing
            String::from("• two"),
            String::new(),                        // quote spacing before
            String::from("  Quoted"),             // quote indented two columns
            String::new(),                        // quote spacing after
            String::from("────────────────────"), // separator fills the width
        ]
    );
}

#[test]
fn document_taller_than_the_addressable_range_returns_dimension_overflow() {
    let nodes = vec![SemanticNode::Separator; usize::from(u16::MAX) + 1];
    let document = document_of(nodes);

    let outcome = render_document(&document, 4);

    assert!(matches!(outcome, Err(LayoutError::DimensionOverflow)));
}
