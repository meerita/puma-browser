// @file crates/browser-html/tests/quotation.rs
// @description Behavior tests for <q> quote-mark synthesis, nesting, and cite resolution.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, InlineEmphasis, InlineRun, SemanticNode};

/// The inline runs of the first node, which every case here builds as a single paragraph.
fn paragraph_runs(source: &str) -> Vec<InlineRun> {
    let document = parse_html(source.as_bytes(), None).expect("well-formed HTML must parse");
    match document
        .children()
        .first()
        .expect("the document must hold at least one node")
    {
        SemanticNode::Paragraph { runs, .. } => runs.clone(),
        other => panic!("expected a paragraph, found {other:?}"),
    }
}

/// The concatenated text of a run sequence, as `browser_read` would produce it.
fn joined_text(runs: &[InlineRun]) -> String {
    runs.iter().map(|run| run.text.as_str()).collect()
}

#[test]
fn a_bare_quote_element_brackets_its_content_with_curly_quotes() {
    let runs = paragraph_runs("<p><q>Hello</q></p>");

    assert_eq!(joined_text(&runs), "\u{201C}Hello\u{201D}");
    assert!(runs.iter().all(|run| run.citation.is_none()));
}

#[test]
fn a_cite_attribute_resolves_to_an_absolute_url_on_every_bracketed_run() {
    let runs = paragraph_runs(r#"<p><q cite="https://example.com/source">Hello</q></p>"#);

    assert_eq!(joined_text(&runs), "\u{201C}Hello\u{201D}");
    assert!(runs
        .iter()
        .all(|run| run.citation == Some("https://example.com/source".to_string())));
}

#[test]
fn a_relative_cite_resolves_against_the_document_base_url() {
    let runs = paragraph_runs(
        r#"<base href="https://example.com/docs/"><p><q cite="source.html">Hello</q></p>"#,
    );

    assert!(runs
        .iter()
        .all(|run| run.citation == Some("https://example.com/docs/source.html".to_string())));
}

#[test]
fn a_missing_cite_yields_no_citation_but_keeps_the_quote_marks() {
    let runs = paragraph_runs("<p><q>Hello</q></p>");

    assert_eq!(joined_text(&runs), "\u{201C}Hello\u{201D}");
    assert!(runs.iter().all(|run| run.citation.is_none()));
}

#[test]
fn an_empty_cite_yields_no_citation_but_keeps_the_quote_marks() {
    let runs = paragraph_runs(r#"<p><q cite="">Hello</q></p>"#);

    assert_eq!(joined_text(&runs), "\u{201C}Hello\u{201D}");
    assert!(runs.iter().all(|run| run.citation.is_none()));
}

#[test]
fn nested_quotes_alternate_between_double_and_single_curly_marks() {
    let runs = paragraph_runs("<p><q>Outer <q>inner</q> text</q></p>");

    assert_eq!(
        joined_text(&runs),
        "\u{201C}Outer \u{2018}inner\u{2019} text\u{201D}"
    );
}

#[test]
fn a_quote_inside_an_anchor_carries_both_link_and_citation() {
    let runs = paragraph_runs(
        r#"<p><a href="https://example.com/article"><q cite="https://example.com/source">Hello</q></a></p>"#,
    );

    assert_eq!(joined_text(&runs), "\u{201C}Hello\u{201D}");
    assert!(runs.iter().all(
        |run| run.link == Some("https://example.com/article".to_string())
            && run.citation == Some("https://example.com/source".to_string())
    ));
}

#[test]
fn emphasis_inside_a_quote_breaks_the_run_but_keeps_the_citation() {
    let runs = paragraph_runs(
        r#"<p><q cite="https://example.com/source">plain <strong>bold</strong> tail</q></p>"#,
    );

    assert_eq!(joined_text(&runs), "\u{201C}plain bold tail\u{201D}");
    let bold_run = runs
        .iter()
        .find(|run| run.text == "bold")
        .expect("a run with the bold text must exist");
    assert_eq!(
        bold_run.emphasis,
        InlineEmphasis {
            strong: true,
            emphasis: false,
            code: false,
        }
    );
    assert!(runs
        .iter()
        .all(|run| run.citation == Some("https://example.com/source".to_string())));
}
