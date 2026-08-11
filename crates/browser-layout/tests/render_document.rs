// @file crates/browser-layout/tests/render_document.rs
// @description Behavior tests for render_document: wrapping, markers, verbatim code, widths.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::Emphasis;
use browser_html::{Document, InlineEmphasis, InlineRun, SemanticNode};
use browser_layout::{render_document, CellBuffer, LayoutError, WidthConfig};
use unicode_width::UnicodeWidthStr;

fn document_of(nodes: Vec<SemanticNode>) -> Document {
    Document::new(nodes, None, 0)
}

fn paragraph(text: &str) -> SemanticNode {
    SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(text.to_string())],
        inline_style: None,
    }
}

fn list_item(text: &str) -> SemanticNode {
    SemanticNode::ListItem {
        children: vec![paragraph(text)],
        inline_style: None,
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

    let outcome = render_document(&document, 0, &WidthConfig::default());

    assert!(matches!(outcome, Err(LayoutError::ZeroWidth)));
}

#[test]
fn long_paragraph_wraps_so_no_row_exceeds_the_width() {
    let width = 10u16;
    let words = vec!["word"; 40].join(" ");
    let document = document_of(vec![paragraph(&words)]);

    let buffer =
        render_document(&document, width, &WidthConfig::default()).expect("paragraph must lay out");

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
            inline_style: None,
        },
        SemanticNode::List {
            ordered: false,
            children: vec![list_item("one"), list_item("two")],
            inline_style: None,
        },
    ]);

    let buffer =
        render_document(&document, 40, &WidthConfig::default()).expect("document must lay out");

    // One blank row after the heading, then one row per list item, then one blank from list.
    assert_eq!(buffer.height(), 5);
    assert_eq!(row_text(&buffer, 0).trim_end(), "Title");
    assert_eq!(buffer.cell_at(0, 2).expect("bullet cell").grapheme(), "•");
    assert_eq!(buffer.cell_at(0, 3).expect("bullet cell").grapheme(), "•");
    assert_eq!(row_text(&buffer, 2).trim_end(), "• one");
}

#[test]
fn ordered_list_items_render_running_numbers() {
    let document = document_of(vec![SemanticNode::List {
        ordered: true,
        children: vec![list_item("x"), list_item("y")],
        inline_style: None,
    }]);

    let buffer =
        render_document(&document, 40, &WidthConfig::default()).expect("ordered list must lay out");

    assert_eq!(row_text(&buffer, 0).trim_end(), "1. x");
    assert_eq!(row_text(&buffer, 1).trim_end(), "2. y");
}

#[test]
fn a_nested_unordered_list_indents_one_level_under_its_parent() {
    let inner = SemanticNode::List {
        ordered: false,
        children: vec![list_item("b")],
        inline_style: None,
    };
    let document = document_of(vec![SemanticNode::List {
        ordered: false,
        children: vec![SemanticNode::ListItem {
            children: vec![paragraph("a"), inner],
            inline_style: None,
        }],
        inline_style: None,
    }]);

    let buffer =
        render_document(&document, 40, &WidthConfig::default()).expect("nested list must lay out");

    assert_eq!(row_text(&buffer, 0).trim_end(), "• a");
    // row 1 is blank: paragraph spacing_after inside the list item before the nested list
    assert_eq!(row_text(&buffer, 2).trim_end(), "  • b");
}

#[test]
fn ordered_numbering_restarts_within_each_nested_list() {
    let inner = SemanticNode::List {
        ordered: true,
        children: vec![list_item("b"), list_item("c")],
        inline_style: None,
    };
    let document = document_of(vec![SemanticNode::List {
        ordered: true,
        children: vec![
            SemanticNode::ListItem {
                children: vec![paragraph("a"), inner],
                inline_style: None,
            },
            list_item("d"),
        ],
        inline_style: None,
    }]);

    let buffer = render_document(&document, 40, &WidthConfig::default())
        .expect("nested ordered list must lay out");

    let rows: Vec<String> = (0..buffer.height())
        .map(|row| row_text(&buffer, row).trim_end().to_string())
        .collect();
    // The nested list restarts at 1, and the outer list resumes at 2 after it.
    // Row 1 is blank: paragraph spacing_after inside the list item, before the nested list.
    assert_eq!(rows, vec!["1. a", "", "   1. b", "   2. c", "2. d", ""]);
}

#[test]
fn a_wrapped_list_item_aligns_its_continuation_under_the_item_text() {
    let document = document_of(vec![SemanticNode::List {
        ordered: false,
        children: vec![list_item("alpha beta")],
        inline_style: None,
    }]);

    let buffer = render_document(&document, 7, &WidthConfig::default()).expect("list must lay out");

    // "• alpha" fills the width; "beta" wraps and indents under the item text, not the marker.
    assert_eq!(row_text(&buffer, 0).trim_end(), "• alpha");
    assert_eq!(row_text(&buffer, 1).trim_end(), "  beta");
}

#[test]
fn code_block_is_rendered_verbatim_and_clipped_not_wrapped() {
    let document = document_of(vec![SemanticNode::CodeBlock {
        text: String::from("abcdefghij\nkl"),
    }]);

    let buffer =
        render_document(&document, 5, &WidthConfig::default()).expect("code block must lay out");

    // Two source lines stay two rows, followed by one blank from code block spacing_after.
    assert_eq!(buffer.height(), 3);
    assert_eq!(row_text(&buffer, 0), "abcde");
    assert_eq!(row_text(&buffer, 1).trim_end(), "kl");
}

#[test]
fn combining_mark_grapheme_occupies_a_single_cell() {
    let document = document_of(vec![paragraph("e\u{0301}x")]);

    let buffer =
        render_document(&document, 10, &WidthConfig::default()).expect("paragraph must lay out");

    assert_eq!(
        buffer.cell_at(0, 0).expect("cluster cell").grapheme(),
        "e\u{0301}"
    );
    assert_eq!(buffer.cell_at(1, 0).expect("next cell").grapheme(), "x");
}

#[test]
fn double_width_grapheme_advances_two_columns() {
    let document = document_of(vec![paragraph("x界y")]);

    let buffer =
        render_document(&document, 10, &WidthConfig::default()).expect("paragraph must lay out");

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
            inline_style: None,
        },
        paragraph("Body text"),
        SemanticNode::List {
            ordered: false,
            children: vec![list_item("one"), list_item("two")],
            inline_style: None,
        },
        SemanticNode::Quote {
            children: vec![paragraph("Quoted")],
            inline_style: None,
        },
        SemanticNode::Separator,
    ]);

    let buffer =
        render_document(&document, 20, &WidthConfig::default()).expect("document must lay out");

    let rows: Vec<String> = (0..buffer.height())
        .map(|row| row_text(&buffer, row).trim_end().to_string())
        .collect();

    assert_eq!(
        rows,
        vec![
            String::from("Title"),     // heading
            String::new(),             // heading spacing_after
            String::from("Body text"), // paragraph
            String::new(),             // paragraph spacing_after
            String::from("• one"),     // list items run tight
            String::from("• two"),
            String::new(),                  // list spacing_after
            String::from("  Quoted"),       // quote indented two columns
            String::new(),                  // quote spacing_after
            String::from("      ━━━━━━━━"), // separator: 6-column left pad, 8-column centered rule
            String::new(),                  // separator spacing_after
        ]
    );
}

#[test]
fn a_separator_draws_a_centered_thirty_percent_rule_at_a_wide_width() {
    let document = document_of(vec![SemanticNode::Separator]);

    let buffer =
        render_document(&document, 100, &WidthConfig::default()).expect("document must lay out");

    // clamp(100 * 30 / 100 = 30, 8, 40) = 30 glyphs, centered with a (100 - 30) / 2 = 35 pad.
    assert_eq!(
        row_text(&buffer, 0).trim_end(),
        format!("{}{}", " ".repeat(35), "━".repeat(30))
    );
}

#[test]
fn a_separator_rule_is_clamped_to_the_ceiling_on_an_ultrawide_terminal() {
    let document = document_of(vec![SemanticNode::Separator]);

    let buffer =
        render_document(&document, 200, &WidthConfig::default()).expect("document must lay out");

    // clamp(200 * 30 / 100 = 60, 8, 40) = 40 glyphs, not 60.
    let rule = row_text(&buffer, 0);
    assert_eq!(rule.trim_end().chars().filter(|c| *c == '━').count(), 40);
}

#[test]
fn a_separator_rule_is_clamped_to_the_floor_on_a_narrow_terminal() {
    let document = document_of(vec![SemanticNode::Separator]);

    let buffer =
        render_document(&document, 10, &WidthConfig::default()).expect("document must lay out");

    // clamp(10 * 30 / 100 = 3, 8, 40) = 8 glyphs, centered with a (10 - 8) / 2 = 1 pad.
    assert_eq!(
        row_text(&buffer, 0).trim_end(),
        format!(" {}", "━".repeat(8))
    );
}

#[test]
fn a_separator_rule_is_capped_at_the_content_width_and_does_not_panic() {
    let document = document_of(vec![SemanticNode::Separator]);

    let buffer =
        render_document(&document, 4, &WidthConfig::default()).expect("document must lay out");

    // The floor 8 exceeds the width 4, so the rule is capped at 4 glyphs with no left pad.
    assert_eq!(row_text(&buffer, 0).trim_end(), "━".repeat(4));
}

#[test]
fn a_separator_uses_the_heavy_glyph_never_the_light_one() {
    let document = document_of(vec![SemanticNode::Separator]);

    let buffer =
        render_document(&document, 40, &WidthConfig::default()).expect("document must lay out");

    let rule = row_text(&buffer, 0);
    assert!(rule.contains('━'));
    assert!(!rule.contains('─'));
}

fn strong_run(text: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis {
            strong: true,
            emphasis: false,
            code: false,
        },
        link: None,
        citation: None,
        anchors: Vec::new(),
    }
}

fn linked_run(text: &str, href: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: Some(href.to_string()),
        citation: None,
        anchors: Vec::new(),
    }
}

#[test]
fn a_multi_run_paragraph_applies_each_runs_own_emphasis() {
    let document = document_of(vec![SemanticNode::Paragraph {
        runs: vec![
            InlineRun::plain(String::from("word ")),
            strong_run("bold"),
            InlineRun::plain(String::from(" tail")),
        ],
        inline_style: None,
    }]);

    let buffer =
        render_document(&document, 40, &WidthConfig::default()).expect("paragraph must lay out");

    assert_eq!(row_text(&buffer, 0).trim_end(), "word bold tail");
    // "word " occupies columns 0..5, "bold" columns 5..9, " tail" from column 9.
    assert_eq!(
        buffer.cell_at(0, 0).expect("plain cell").emphasis(),
        Emphasis::None
    );
    assert_eq!(
        buffer.cell_at(5, 0).expect("bold cell").emphasis(),
        Emphasis::Bold
    );
    assert_eq!(
        buffer.cell_at(8, 0).expect("bold cell").emphasis(),
        Emphasis::Bold
    );
    assert_eq!(
        buffer.cell_at(10, 0).expect("plain cell").emphasis(),
        Emphasis::None
    );
}

#[test]
fn a_linked_run_is_underlined_while_plain_text_is_not() {
    let document = document_of(vec![SemanticNode::Paragraph {
        runs: vec![
            InlineRun::plain(String::from("see ")),
            linked_run("link", "/x"),
        ],
        inline_style: None,
    }]);

    let buffer =
        render_document(&document, 40, &WidthConfig::default()).expect("paragraph must lay out");

    assert_eq!(row_text(&buffer, 0).trim_end(), "see link");
    // "see " occupies columns 0..4, "link" columns 4..8.
    assert!(!buffer.cell_at(0, 0).expect("plain cell").underline());
    assert!(buffer.cell_at(4, 0).expect("link cell").underline());
    assert!(buffer.cell_at(7, 0).expect("link cell").underline());
}

#[test]
fn run_boundaries_do_not_change_word_wrapping() {
    let width = 12u16;
    let plain = document_of(vec![paragraph("hello wonderful world")]);
    // The same visible text, split into runs mid-word and at a space, must wrap the same.
    let marked = document_of(vec![SemanticNode::Paragraph {
        runs: vec![
            InlineRun::plain(String::from("hello won")),
            strong_run("der"),
            InlineRun::plain(String::from("ful world")),
        ],
        inline_style: None,
    }]);

    let plain_buffer = render_document(&plain, width, &WidthConfig::default())
        .expect("plain paragraph must lay out");
    let marked_buffer = render_document(&marked, width, &WidthConfig::default())
        .expect("marked paragraph must lay out");

    let plain_rows: Vec<String> = (0..plain_buffer.height())
        .map(|row| row_text(&plain_buffer, row).trim_end().to_string())
        .collect();
    let marked_rows: Vec<String> = (0..marked_buffer.height())
        .map(|row| row_text(&marked_buffer, row).trim_end().to_string())
        .collect();

    assert_eq!(plain_rows, marked_rows);
    assert_eq!(plain_rows, vec!["hello", "wonderful", "world", ""]);
}

#[test]
fn document_taller_than_the_addressable_range_returns_dimension_overflow() {
    let nodes = vec![SemanticNode::Separator; usize::from(u16::MAX) + 1];
    let document = document_of(nodes);

    let outcome = render_document(&document, 4, &WidthConfig::default());

    assert!(matches!(outcome, Err(LayoutError::DimensionOverflow)));
}
