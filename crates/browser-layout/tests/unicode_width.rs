// @file crates/browser-layout/tests/unicode_width.rs
// @description Behavior tests for Unicode-correct layout: CJK width, combining marks, ambiguous mode.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_html::{Document, InlineRun, SemanticNode};
use browser_layout::{render_document, AmbiguousWidth, CellBuffer, WidthConfig};

fn document_of(nodes: Vec<SemanticNode>) -> Document {
    Document::new(nodes, None, 0)
}

fn paragraph(text: &str) -> SemanticNode {
    SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(text.to_string())],
        inline_style: None,
    }
}

fn row_text(buffer: &CellBuffer, row: u16) -> String {
    (0..buffer.width())
        .filter_map(|column| buffer.cell_at(column, row))
        .map(|cell| cell.grapheme())
        .collect()
}

fn rows(buffer: &CellBuffer) -> Vec<String> {
    (0..buffer.height())
        .map(|row| row_text(buffer, row).trim_end().to_string())
        .collect()
}

#[test]
fn a_cjk_word_wraps_counting_each_ideograph_as_two_columns() {
    // Six ideographs are one unspaced word of twelve columns; at width six it force-breaks
    // after three ideographs per row (each ideograph spans two columns), never five and a
    // half, so the whole word lays out on exactly two rows.
    let document = document_of(vec![paragraph("界界界界界界")]);

    let buffer = render_document(&document, 6, &WidthConfig::default()).expect("CJK must lay out");

    assert_eq!(buffer.height(), 2);
    for row in 0..2 {
        for column in [0u16, 2, 4] {
            assert_eq!(
                buffer
                    .cell_at(column, row)
                    .expect("ideograph cell")
                    .grapheme(),
                "界",
                "row {row} column {column} must hold an ideograph"
            );
        }
    }
}

#[test]
fn a_wide_grapheme_at_the_right_edge_wraps_instead_of_splitting() {
    // Width five leaves one column free after "abcd"; the wide ideograph cannot fit there
    // and moves to the next row rather than being placed in a single column.
    let document = document_of(vec![paragraph("abcd界")]);

    let buffer = render_document(&document, 5, &WidthConfig::default()).expect("word must lay out");

    assert_eq!(rows(&buffer), vec!["abcd", "界"]);
}

#[test]
fn the_trailing_column_of_a_wide_grapheme_is_blank() {
    let document = document_of(vec![paragraph("界x")]);

    let buffer =
        render_document(&document, 10, &WidthConfig::default()).expect("paragraph must lay out");

    assert_eq!(buffer.cell_at(0, 0).expect("wide cell").grapheme(), "界");
    assert_eq!(buffer.cell_at(1, 0).expect("spanned cell").grapheme(), " ");
    assert_eq!(buffer.cell_at(2, 0).expect("next cell").grapheme(), "x");
}

#[test]
fn a_base_and_its_combining_marks_stay_one_cluster_under_force_break() {
    // "e" plus two combining marks is one grapheme cluster of one column. Force-broken at
    // width one alongside plain letters, the cluster never splits across rows and no
    // combining mark starts a row on its own.
    let clustered = "ae\u{0301}\u{0323}b";
    let document = document_of(vec![paragraph(clustered)]);

    let buffer =
        render_document(&document, 1, &WidthConfig::default()).expect("cluster must lay out");

    assert_eq!(rows(&buffer), vec!["a", "e\u{0301}\u{0323}", "b"]);
}

#[test]
fn an_ambiguous_width_grapheme_measures_one_column_in_narrow_mode() {
    // U+00A7 SECTION SIGN is East-Asian-ambiguous: one column on a Western terminal.
    let document = document_of(vec![paragraph("\u{00a7}x")]);

    let buffer = render_document(&document, 10, &WidthConfig::new(AmbiguousWidth::Narrow))
        .expect("paragraph must lay out");

    assert_eq!(
        buffer.cell_at(0, 0).expect("section cell").grapheme(),
        "\u{00a7}"
    );
    assert_eq!(buffer.cell_at(1, 0).expect("next cell").grapheme(), "x");
}

#[test]
fn an_ambiguous_width_grapheme_measures_two_columns_in_wide_mode() {
    let document = document_of(vec![paragraph("\u{00a7}x")]);

    let buffer = render_document(&document, 10, &WidthConfig::new(AmbiguousWidth::Wide))
        .expect("paragraph must lay out");

    assert_eq!(
        buffer.cell_at(0, 0).expect("section cell").grapheme(),
        "\u{00a7}"
    );
    // In wide mode the section sign spans two columns, so its trailing column is blank and
    // the following grapheme lands at column two.
    assert_eq!(buffer.cell_at(1, 0).expect("spanned cell").grapheme(), " ");
    assert_eq!(buffer.cell_at(2, 0).expect("next cell").grapheme(), "x");
}

#[test]
fn an_ambiguous_width_word_wraps_sooner_in_wide_mode_than_narrow() {
    let text = "\u{00a7}\u{00a7}\u{00a7}\u{00a7}";
    let document = document_of(vec![paragraph(text)]);

    let narrow = render_document(&document, 4, &WidthConfig::new(AmbiguousWidth::Narrow))
        .expect("narrow must lay out");
    let wide = render_document(&document, 4, &WidthConfig::new(AmbiguousWidth::Wide))
        .expect("wide must lay out");

    // Four ambiguous graphemes fit one row of four columns in narrow mode; in wide mode
    // they are eight columns and force-break two per row.
    assert_eq!(narrow.height(), 1);
    assert_eq!(wide.height(), 2);
    assert_eq!(
        wide.cell_at(0, 0).expect("first cell").grapheme(),
        "\u{00a7}"
    );
    assert_eq!(
        wide.cell_at(2, 0).expect("second cell").grapheme(),
        "\u{00a7}"
    );
}

#[test]
fn ascii_layout_is_unchanged_by_the_default_width_config() {
    let document = document_of(vec![paragraph("hello world")]);

    let buffer =
        render_document(&document, 20, &WidthConfig::default()).expect("paragraph must lay out");

    assert_eq!(rows(&buffer), vec!["hello world"]);
}
