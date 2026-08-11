// @file crates/browser-layout/tests/render_table.rs
// @description Behavior tests for table layout: aligned column mode, record-view fallback, widths.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::Emphasis;
use browser_html::{Document, InlineRun, SemanticNode};
use browser_layout::{render_document, CellBuffer, WidthConfig};

fn document_of(nodes: Vec<SemanticNode>) -> Document {
    Document::new(nodes, None, 0)
}

fn cell(header: bool, text: &str) -> SemanticNode {
    SemanticNode::TableCell {
        header,
        children: vec![SemanticNode::Paragraph {
            runs: vec![InlineRun::plain(text.to_string())],
            inline_style: None,
        }],
        inline_style: None,
    }
}

fn linked_cell(text: &str, href: &str) -> SemanticNode {
    SemanticNode::TableCell {
        header: false,
        children: vec![SemanticNode::Paragraph {
            runs: vec![InlineRun {
                text: text.to_string(),
                emphasis: browser_html::InlineEmphasis::none(),
                link: Some(href.to_string()),
                citation: None,
                anchors: Vec::new(),
            }],
            inline_style: None,
        }],
        inline_style: None,
    }
}

fn row(cells: Vec<SemanticNode>) -> SemanticNode {
    SemanticNode::TableRow { children: cells }
}

fn table(rows: Vec<SemanticNode>) -> SemanticNode {
    SemanticNode::Table { children: rows }
}

fn people_table() -> SemanticNode {
    table(vec![
        row(vec![
            cell(true, "Name"),
            cell(true, "Role"),
            cell(true, "Location"),
        ]),
        row(vec![
            cell(false, "Alice"),
            cell(false, "Engineer"),
            cell(false, "Madrid"),
        ]),
        row(vec![
            cell(false, "Bob"),
            cell(false, "Designer"),
            cell(false, "London"),
        ]),
    ])
}

fn row_text(buffer: &CellBuffer, row: u16) -> String {
    (0..buffer.width())
        .filter_map(|column| buffer.cell_at(column, row))
        .map(|cell| cell.grapheme())
        .collect()
}

fn find_row(buffer: &CellBuffer, needle: &str) -> Option<u16> {
    (0..buffer.height()).find(|row| row_text(buffer, *row).contains(needle))
}

#[test]
fn wide_view_aligns_each_column_across_header_and_data_rows() {
    let buffer = render_document(
        &document_of(vec![people_table()]),
        40,
        &WidthConfig::default(),
    )
    .expect("a table that fits must lay out");

    let header = row_text(&buffer, 0);
    let alice = row_text(&buffer, 1);
    assert_eq!(
        header.find("Role"),
        alice.find("Engineer"),
        "a column starts at the same offset on every row"
    );
    assert_eq!(
        header.find("Location"),
        alice.find("Madrid"),
        "the third column is aligned too"
    );
}

#[test]
fn header_cells_render_bold_and_data_cells_do_not() {
    let buffer = render_document(
        &document_of(vec![people_table()]),
        40,
        &WidthConfig::default(),
    )
    .expect("a table that fits must lay out");

    let header_cell = buffer.cell_at(0, 0).expect("the header row exists");
    let data_cell = buffer.cell_at(0, 1).expect("the first data row exists");
    assert_eq!(header_cell.emphasis(), Emphasis::Bold);
    assert_eq!(data_cell.emphasis(), Emphasis::None);
}

#[test]
fn narrow_view_falls_back_to_record_lines() {
    let buffer = render_document(
        &document_of(vec![people_table()]),
        18,
        &WidthConfig::default(),
    )
    .expect("a narrow table must lay out");

    let heading = find_row(&buffer, "Alice").expect("the first record heading appears");
    assert_eq!(
        row_text(&buffer, heading).trim_end(),
        "Alice",
        "the first cell is the record heading on its own line"
    );
    let role = find_row(&buffer, "Role:").expect("a labelled field line appears");
    assert!(
        row_text(&buffer, role).contains("Role: Engineer"),
        "remaining cells render as Label: value"
    );
}

#[test]
fn column_widths_derive_from_the_widest_cell_in_the_column() {
    let grid = table(vec![
        row(vec![cell(false, "a"), cell(false, "x")]),
        row(vec![cell(false, "long"), cell(false, "y")]),
    ]);

    let buffer = render_document(&document_of(vec![grid]), 40, &WidthConfig::default())
        .expect("table must lay out");

    // Column zero is as wide as "long" (4) plus a two-column gap, so the second column
    // starts at offset 6 on both rows regardless of the shorter cell above.
    assert_eq!(buffer.cell_at(6, 0).map(|cell| cell.grapheme()), Some("x"));
    assert_eq!(buffer.cell_at(6, 1).map(|cell| cell.grapheme()), Some("y"));
}

#[test]
fn wide_grapheme_cells_measure_at_two_columns() {
    let grid = table(vec![
        row(vec![cell(false, "字"), cell(false, "b")]),
        row(vec![cell(false, "a"), cell(false, "c")]),
    ]);

    let buffer = render_document(&document_of(vec![grid]), 40, &WidthConfig::default())
        .expect("table must lay out");

    // The wide grapheme occupies two columns, so column zero is two wide and the second
    // column starts at offset 4 (two for the grapheme, two for the gap).
    assert_eq!(buffer.cell_at(0, 0).map(|cell| cell.grapheme()), Some("字"));
    assert_eq!(buffer.cell_at(4, 0).map(|cell| cell.grapheme()), Some("b"));
    assert_eq!(buffer.cell_at(4, 1).map(|cell| cell.grapheme()), Some("c"));
}

#[test]
fn linked_cell_content_is_underlined() {
    let grid = table(vec![row(vec![
        linked_cell("docs", "/x"),
        cell(false, "tail"),
    ])]);

    let buffer = render_document(&document_of(vec![grid]), 40, &WidthConfig::default())
        .expect("table must lay out");

    let first = buffer.cell_at(0, 0).expect("the linked cell renders");
    assert!(
        first.underline(),
        "a linked cell keeps its underline styling"
    );
}
