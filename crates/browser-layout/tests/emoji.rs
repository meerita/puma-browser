// @file crates/browser-layout/tests/emoji.rs
// @description Behavior tests: emoji clusters survive wrap and truncation and honour the emoji-width mode.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_html::{Document, InlineRun, SemanticNode};
use browser_layout::{render_document, CellBuffer, EmojiWidth, WidthConfig};

/// A zero-width-joiner family sequence: four emoji joined into one grapheme cluster.
const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";

/// A warning sign followed by the emoji variation selector U+FE0F.
const WARNING_WITH_VARIATION_SELECTOR: &str = "\u{26A0}\u{FE0F}";

/// The neutral placeholder substituted for an emoji cluster in replace mode.
const PLACEHOLDER: &str = "\u{25AF}";

fn document_of(nodes: Vec<SemanticNode>) -> Document {
    Document::new(nodes, None, 0)
}

fn paragraph(text: &str) -> SemanticNode {
    SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(text.to_string())],
        inline_style: None,
    }
}

fn code_block(text: &str) -> SemanticNode {
    SemanticNode::CodeBlock {
        text: text.to_string(),
    }
}

fn grapheme_at(buffer: &CellBuffer, column: u16, row: u16) -> String {
    buffer
        .cell_at(column, row)
        .expect("cell must exist")
        .grapheme()
        .to_string()
}

fn wide() -> WidthConfig {
    WidthConfig::default().with_emoji_width(EmojiWidth::Wide)
}

fn narrow() -> WidthConfig {
    WidthConfig::default().with_emoji_width(EmojiWidth::Narrow)
}

fn replace() -> WidthConfig {
    WidthConfig::default().with_emoji_width(EmojiWidth::Replace)
}

#[test]
fn a_family_emoji_stays_one_cell_when_it_fits_on_a_line() {
    let document = document_of(vec![paragraph(&format!("{FAMILY}x"))]);

    let buffer = render_document(&document, 10, &wide(), None).expect("emoji must lay out");

    assert_eq!(grapheme_at(&buffer, 0, 0), FAMILY);
    // The family spans two columns in wide mode, so its trailing column is blank and the
    // following grapheme lands at column two, never inside the cluster.
    assert_eq!(grapheme_at(&buffer, 1, 0), " ");
    assert_eq!(grapheme_at(&buffer, 2, 0), "x");
}

#[test]
fn a_family_emoji_advances_one_column_in_narrow_mode() {
    let document = document_of(vec![paragraph(&format!("{FAMILY}x"))]);

    let buffer = render_document(&document, 10, &narrow(), None).expect("emoji must lay out");

    assert_eq!(grapheme_at(&buffer, 0, 0), FAMILY);
    assert_eq!(grapheme_at(&buffer, 1, 0), "x");
}

#[test]
fn an_emoji_with_a_variation_selector_stays_intact() {
    let document = document_of(vec![paragraph(WARNING_WITH_VARIATION_SELECTOR)]);

    let buffer = render_document(&document, 10, &wide(), None).expect("emoji must lay out");

    assert_eq!(grapheme_at(&buffer, 0, 0), WARNING_WITH_VARIATION_SELECTOR);
}

#[test]
fn replace_mode_substitutes_the_placeholder_and_advances_by_its_width() {
    let document = document_of(vec![paragraph(&format!("{FAMILY}x"))]);

    let buffer = render_document(&document, 10, &replace(), None).expect("emoji must lay out");

    // The placeholder is one column, so the following grapheme lands immediately after it.
    assert_eq!(grapheme_at(&buffer, 0, 0), PLACEHOLDER);
    assert_eq!(grapheme_at(&buffer, 1, 0), "x");
}

#[test]
fn a_family_emoji_is_never_split_across_rows_by_a_force_break() {
    // Three families with no separating space are one unspaced word wider than the line; at
    // width four in wide mode they force-break two families (four columns) per row. Each
    // family must stay whole in one cell, never split at a joiner.
    let word = FAMILY.repeat(3);
    let document = document_of(vec![paragraph(&word)]);

    let buffer = render_document(&document, 4, &wide(), None).expect("emoji must lay out");

    assert_eq!(buffer.height(), 3); // 2 content rows + 1 blank from paragraph spacing_after
    assert_eq!(grapheme_at(&buffer, 0, 0), FAMILY);
    assert_eq!(grapheme_at(&buffer, 2, 0), FAMILY);
    assert_eq!(grapheme_at(&buffer, 0, 1), FAMILY);
}

#[test]
fn truncation_drops_an_emoji_cluster_that_would_cross_the_width() {
    // A code block clips rather than wraps. "abcd" fills four columns; the two-column family
    // cannot fit in the single remaining column and is dropped whole, never half-written.
    let document = document_of(vec![code_block(&format!("abcd{FAMILY}"))]);

    let buffer = render_document(&document, 5, &wide(), None).expect("code must lay out");

    assert_eq!(grapheme_at(&buffer, 0, 0), "a");
    assert_eq!(grapheme_at(&buffer, 3, 0), "d");
    assert_eq!(grapheme_at(&buffer, 4, 0), " ");
}

#[test]
fn truncation_keeps_an_emoji_cluster_whole_when_it_fits_exactly() {
    let document = document_of(vec![code_block(&format!("abcd{FAMILY}"))]);

    let buffer = render_document(&document, 6, &wide(), None).expect("code must lay out");

    assert_eq!(grapheme_at(&buffer, 3, 0), "d");
    assert_eq!(grapheme_at(&buffer, 4, 0), FAMILY);
    assert_eq!(grapheme_at(&buffer, 5, 0), " ");
}

#[test]
fn a_paragraph_of_plain_text_is_unaffected_by_replace_mode() {
    let document = document_of(vec![paragraph("hello world")]);

    let buffer = render_document(&document, 20, &replace(), None).expect("text must lay out");

    let row: String = (0..buffer.width())
        .map(|column| grapheme_at(&buffer, column, 0))
        .collect();
    assert_eq!(row.trim_end(), "hello world");
}
