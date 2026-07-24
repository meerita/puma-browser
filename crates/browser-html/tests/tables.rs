// @file crates/browser-html/tests/tables.rs
// @description Behavior tests for table parsing: section flattening, header cells, and dimension caps.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, SemanticNode};

/// The row cap the parser enforces per table; a table beyond it is truncated and warns.
const ROW_CAP: usize = 1_000;

/// The column cap the parser enforces per row; a row beyond it is truncated and warns.
const COLUMN_CAP: usize = 64;

fn first_table(source: &str) -> SemanticNode {
    let document = parse_html(source).expect("well-formed HTML must parse");
    document
        .children()
        .iter()
        .find(|node| matches!(node, SemanticNode::Table { .. }))
        .cloned()
        .expect("a table must be produced")
}

fn document_has_warning(source: &str) -> bool {
    let document = parse_html(source).expect("well-formed HTML must parse");
    document
        .children()
        .iter()
        .any(|node| matches!(node, SemanticNode::Warning { .. }))
}

fn rows(table: &SemanticNode) -> Vec<SemanticNode> {
    let SemanticNode::Table { children } = table else {
        panic!("expected a table, found {table:?}");
    };
    children.clone()
}

fn cells(row: &SemanticNode) -> Vec<SemanticNode> {
    let SemanticNode::TableRow { children } = row else {
        panic!("expected a table row, found {row:?}");
    };
    children.clone()
}

fn cell_text(cell: &SemanticNode) -> String {
    let SemanticNode::TableCell { children, .. } = cell else {
        panic!("expected a table cell, found {cell:?}");
    };
    let mut text = String::new();
    for block in children {
        if let SemanticNode::Paragraph { runs, .. } = block {
            runs.iter().for_each(|run| text.push_str(&run.text));
        }
    }
    text
}

fn cell_is_header(cell: &SemanticNode) -> bool {
    matches!(cell, SemanticNode::TableCell { header: true, .. })
}

#[test]
fn thead_and_tbody_flatten_into_one_ordered_row_sequence() {
    let table = first_table(
        "<table>\
            <thead><tr><th>Name</th><th>Role</th></tr></thead>\
            <tbody>\
                <tr><td>Alice</td><td>Engineer</td></tr>\
                <tr><td>Bob</td><td>Designer</td></tr>\
            </tbody>\
        </table>",
    );

    let table_rows = rows(&table);
    assert_eq!(table_rows.len(), 3, "header row and two body rows in order");
    assert_eq!(cell_text(&cells(&table_rows[0])[0]), "Name");
    assert_eq!(cell_text(&cells(&table_rows[1])[0]), "Alice");
    assert_eq!(cell_text(&cells(&table_rows[2])[0]), "Bob");
    assert_eq!(cell_text(&cells(&table_rows[2])[1]), "Designer");
}

#[test]
fn th_cells_are_marked_as_headers_and_td_cells_are_not() {
    let table = first_table("<table><tr><th>Name</th><td>Alice</td></tr></table>");

    let cell_list = cells(&rows(&table)[0]);
    assert!(cell_is_header(&cell_list[0]), "a th cell is a header");
    assert!(!cell_is_header(&cell_list[1]), "a td cell is not a header");
}

#[test]
fn cells_preserve_inline_emphasis_and_links_as_runs() {
    let table = first_table(r#"<table><tr><td>see <a href="/x">docs</a> now</td></tr></table>"#);

    let cell = &cells(&rows(&table)[0])[0];
    let SemanticNode::TableCell { children, .. } = cell else {
        panic!("expected a table cell");
    };
    let SemanticNode::Paragraph { runs, .. } = &children[0] else {
        panic!("expected a paragraph inside the cell");
    };
    let linked = runs
        .iter()
        .find(|run| run.text == "docs")
        .expect("the anchor text must survive as its own run");
    assert_eq!(linked.link.as_deref(), Some("/x"));
}

#[test]
fn table_beyond_the_row_cap_is_truncated_and_warns() {
    let mut source = String::from("<table>");
    for _ in 0..(ROW_CAP + 5) {
        source.push_str("<tr><td>x</td></tr>");
    }
    source.push_str("</table>");

    let table = first_table(&source);

    assert_eq!(rows(&table).len(), ROW_CAP, "rows are capped");
    assert!(document_has_warning(&source), "truncation emits a warning");
}

#[test]
fn row_beyond_the_column_cap_is_truncated_and_warns() {
    let mut source = String::from("<table><tr>");
    for _ in 0..(COLUMN_CAP + 5) {
        source.push_str("<td>x</td>");
    }
    source.push_str("</tr></table>");

    let table = first_table(&source);

    assert_eq!(
        cells(&rows(&table)[0]).len(),
        COLUMN_CAP,
        "columns are capped"
    );
    assert!(document_has_warning(&source), "truncation emits a warning");
}
