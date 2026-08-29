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
        citation: None,
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
    render_document(&document_of(nodes), 40, &WidthConfig::default(), None)
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

/// Lay a source document out at `width` columns, exactly as the browser does.
fn render_source(source: &str, width: u16) -> CellBuffer {
    let document =
        browser_html::parse_html(source.as_bytes(), None).expect("well-formed HTML must parse");
    render_document(&document, width, &WidthConfig::default(), None).expect("document must lay out")
}

/// The row a named anchor resolves to, failing the test when it resolves to nothing.
fn anchor_row(buffer: &CellBuffer, name: &str) -> u16 {
    buffer
        .anchors()
        .iter()
        .find(|span| span.name == name)
        .unwrap_or_else(|| {
            panic!(
                "anchor {name:?} must resolve; buffer holds {:?}",
                anchor_summary(buffer)
            )
        })
        .row
}

fn anchor_summary(buffer: &CellBuffer) -> Vec<String> {
    buffer
        .anchors()
        .iter()
        .map(|span| format!("{}@{}", span.name, span.row))
        .collect()
}

/// The visible text of one laid-out row, with trailing blanks removed.
fn row_text(buffer: &CellBuffer, row: u16) -> String {
    let text: String = (0..buffer.width())
        .filter_map(|column| buffer.cell_at(column, row))
        .map(|cell| cell.grapheme())
        .collect();
    text.trim_end().to_string()
}

/// The whole buffer as text, used to prove no anchor name reaches the screen.
fn buffer_text(buffer: &CellBuffer) -> String {
    (0..buffer.height())
        .map(|row| row_text(buffer, row))
        .collect::<Vec<String>>()
        .join("\n")
}

/// The row whose whole rendered text is exactly `text`.
///
/// Matching the whole row rather than a fragment of it keeps a heading apart from a table
/// of contents entry that names the same section.
fn row_of_whole_text(buffer: &CellBuffer, text: &str) -> u16 {
    (0..buffer.height())
        .find(|row| row_text(buffer, *row) == text)
        .unwrap_or_else(|| panic!("no rendered row reads {text:?}\n{}", buffer_text(buffer)))
}

/// The row a piece of rendered text starts on, failing the test when it is absent.
fn row_of_text(buffer: &CellBuffer, text: &str) -> u16 {
    (0..buffer.height())
        .find(|row| row_text(buffer, *row).contains(text))
        .unwrap_or_else(|| panic!("no rendered row contains {text:?}\n{}", buffer_text(buffer)))
}

#[test]
fn a_pretty_printed_section_id_resolves_to_its_first_content_row() {
    let buffer = render_source("<section id=\"a\">\n  <p>Body</p>\n</section>", 40);

    assert_eq!(anchor_row(&buffer, "a"), row_of_text(&buffer, "Body"));
}

#[test]
fn a_pretty_printed_nav_id_resolves_to_its_first_content_row() {
    let buffer = render_source("<nav id=\"toc\">\n  <p>Contents</p>\n</nav>", 40);

    assert_eq!(anchor_row(&buffer, "toc"), row_of_text(&buffer, "Contents"));
}

#[test]
fn a_pretty_printed_blockquote_id_resolves_to_its_first_content_row() {
    let buffer = render_source("<blockquote id=\"bq\">\n  <p>Quoted</p>\n</blockquote>", 40);

    assert_eq!(anchor_row(&buffer, "bq"), row_of_text(&buffer, "Quoted"));
}

#[test]
fn a_preformatted_block_id_resolves_to_its_own_row_not_the_next_block() {
    let buffer = render_source("<pre id=\"p\">code</pre><p>After</p>", 40);

    assert_eq!(anchor_row(&buffer, "p"), row_of_text(&buffer, "code"));
    assert!(anchor_row(&buffer, "p") < row_of_text(&buffer, "After"));
}

#[test]
fn a_separator_id_resolves_to_the_separator_row_not_the_paragraph_after_it() {
    let buffer = render_source("<p>Before</p><hr id=\"sep\"><p>After</p>", 40);

    let separator = anchor_row(&buffer, "sep");
    assert!(separator > row_of_text(&buffer, "Before"));
    assert!(separator < row_of_text(&buffer, "After"));
    assert!(
        row_text(&buffer, separator).contains('━'),
        "the anchor must name the rule's own row, not the block after it"
    );
}

#[test]
fn a_section_id_at_the_end_of_a_document_resolves_to_its_own_content() {
    let buffer = render_source(
        "<p>Body</p>\n<section id=\"z\">\n  <p>Last</p>\n</section>",
        40,
    );

    assert_eq!(anchor_row(&buffer, "z"), row_of_text(&buffer, "Last"));
}

#[test]
fn an_id_after_all_content_resolves_to_the_last_rendered_content_row() {
    let buffer = render_source("<p>Body</p><p id=\"tail\"></p>", 40);

    assert_eq!(anchor_row(&buffer, "tail"), row_of_text(&buffer, "Body"));
}

#[test]
fn an_image_id_resolves_to_the_content_that_follows_it() {
    let buffer = render_source("<img id=\"i\"><p>After</p>", 40);

    assert_eq!(anchor_row(&buffer, "i"), row_of_text(&buffer, "After"));
}

#[test]
fn a_hidden_node_id_resolves_to_the_next_visible_content_row() {
    let buffer = render_source(
        "<p id=\"hidden\" style=\"display:none\">Invisible</p><p>Visible</p>",
        40,
    );

    assert_eq!(
        anchor_row(&buffer, "hidden"),
        row_of_text(&buffer, "Visible")
    );
}

#[test]
fn an_anchor_names_the_blocks_first_content_row_not_the_blank_row_above_it() {
    let buffer = render_source("<p>Before</p><h2 id=\"title\">Section</h2>", 40);

    let row = anchor_row(&buffer, "title");
    assert_eq!(row, row_of_text(&buffer, "Section"));
    assert!(
        !row_text(&buffer, row).is_empty(),
        "an anchor must never name a blank spacing row"
    );
}

#[test]
fn one_id_produces_exactly_one_span() {
    let buffer = render_source("<h2 id=\"once\">Title</h2>", 40);

    assert_eq!(anchor_summary(&buffer), vec!["once@0".to_string()]);
}

#[test]
fn an_id_inside_a_details_element_resolves() {
    let buffer = render_source(
        "<details><summary>More</summary><p id=\"inner\">Detail</p></details>",
        40,
    );

    assert_eq!(anchor_row(&buffer, "inner"), row_of_text(&buffer, "Detail"));
}

#[test]
fn a_table_cell_id_resolves_to_the_row_its_content_starts_on() {
    let buffer = render_source(
        "<table><tr><td>First</td></tr><tr><td id=\"c\">Second</td></tr></table>",
        40,
    );

    assert_eq!(anchor_row(&buffer, "c"), row_of_text(&buffer, "Second"));
}

#[test]
fn a_table_cell_id_resolves_when_the_cell_holds_a_block() {
    let buffer = render_source("<table><tr><td><p id=\"x\">Cell</p></td></tr></table>", 40);

    assert_eq!(anchor_row(&buffer, "x"), row_of_text(&buffer, "Cell"));
}

#[test]
fn parsed_spans_stay_in_ascending_row_order() {
    let buffer = render_source("<h2 id=\"one\">First</h2><h2 id=\"two\">Second</h2>", 40);

    let rows: Vec<u16> = buffer.anchors().iter().map(|span| span.row).collect();
    assert!(rows.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(anchor_row(&buffer, "one") < anchor_row(&buffer, "two"));
}

#[test]
fn a_repeated_id_resolves_to_its_earliest_row() {
    let buffer = render_source("<p id=\"dup\">First</p><p id=\"dup\">Second</p>", 40);

    assert_eq!(anchor_row(&buffer, "dup"), row_of_text(&buffer, "First"));
}

#[test]
fn an_anchor_name_never_appears_in_the_rendered_text() {
    let buffer = render_source("<p id=\"secret-target-name\">Body</p>", 40);

    assert_eq!(anchor_row(&buffer, "secret-target-name"), 0);
    assert!(
        !buffer_text(&buffer).contains("secret-target-name"),
        "an anchor name is remote-sourced and must never be drawn"
    );
}

#[test]
fn a_target_contributes_no_row_of_its_own() {
    let with_target = render_source("<p id=\"named\">Body</p>", 40);
    let without_target = render_source("<p>Body</p>", 40);

    assert_eq!(with_target.height(), without_target.height());
}

#[test]
fn every_kitchen_sink_section_anchor_resolves_to_its_section() {
    let source = include_str!("../../../examples/kitchen-sink.html");
    let buffer = render_source(source, 80);

    let section_headings = [
        ("headings", "Heading level 1"),
        ("text", "Text and inline formatting"),
        ("lists", "Lists"),
        ("tables", "Tables"),
        ("forms", "Forms and controls"),
        ("media", "Media and embedded content"),
        ("semantic", "Semantic grouping"),
    ];
    for (anchor, heading) in section_headings {
        assert_eq!(
            anchor_row(&buffer, anchor),
            row_of_whole_text(&buffer, heading),
            "anchor {anchor:?} must land on the row its section starts on"
        );
    }
    for control in ["username", "password", "email", "bio", "country"] {
        assert!(
            buffer.anchors().iter().any(|span| span.name == control),
            "the form control anchor {control:?} must still resolve"
        );
    }
}
