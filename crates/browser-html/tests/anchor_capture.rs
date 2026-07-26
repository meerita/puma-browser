// @file crates/browser-html/tests/anchor_capture.rs
// @description Behavior tests for capturing element id and <a name> anchors onto inline runs.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, InlineRun, SemanticNode};

/// The inline runs of the document's first text block, whether a heading or a paragraph.
fn first_block_runs(source: &str) -> Vec<InlineRun> {
    let document = parse_html(source.as_bytes(), None).expect("well-formed HTML must parse");
    let first = document
        .children()
        .first()
        .expect("the document must hold at least one node");
    runs_of(first).expect("the first node must be a text block with runs")
}

/// The runs of a heading or paragraph, or `None` for any other node.
fn runs_of(node: &SemanticNode) -> Option<Vec<InlineRun>> {
    match node {
        SemanticNode::Heading { runs, .. } => Some(runs.clone()),
        SemanticNode::Paragraph { runs, .. } => Some(runs.clone()),
        _ => None,
    }
}

/// The anchors of the first run whose text equals `text`.
fn anchors_of_run_with_text(runs: &[InlineRun], text: &str) -> Vec<String> {
    runs.iter()
        .find(|run| run.text == text)
        .map(|run| run.anchors.clone())
        .unwrap_or_else(|| panic!("no run with text {text:?} in {runs:?}"))
}

#[test]
fn heading_id_attaches_to_its_run() {
    let runs = first_block_runs(r#"<h2 id="section">Title</h2>"#);

    assert_eq!(anchors_of_run_with_text(&runs, "Title"), vec!["section"]);
}

#[test]
fn container_id_attaches_to_the_first_run_inside_it() {
    let runs = first_block_runs(r#"<div id="intro"><p>hello</p></div>"#);

    assert_eq!(anchors_of_run_with_text(&runs, "hello"), vec!["intro"]);
}

#[test]
fn anchor_name_before_text_attaches_to_the_following_run() {
    let runs = first_block_runs(r#"<p><a name="marker"></a>body</p>"#);

    assert_eq!(anchors_of_run_with_text(&runs, "body"), vec!["marker"]);
}

#[test]
fn anchor_id_on_a_hrefless_link_attaches_to_its_text() {
    let runs = first_block_runs(r#"<p><a id="target">body</a></p>"#);

    assert_eq!(anchors_of_run_with_text(&runs, "body"), vec!["target"]);
}

#[test]
fn trailing_empty_anchor_lands_on_the_last_run() {
    let runs = first_block_runs(r#"<p>body <a name="end"></a></p>"#);

    assert_eq!(anchors_of_run_with_text(&runs, "body"), vec!["end"]);
}

#[test]
fn a_paragraph_holding_only_an_anchor_still_emits_a_run_carrying_it() {
    let runs = first_block_runs(r#"<p><a name="lonely"></a></p>"#);

    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "");
    assert_eq!(runs[0].anchors, vec!["lonely"]);
}

#[test]
fn two_anchors_before_one_run_both_land_on_it() {
    let runs = first_block_runs(r#"<p><a name="old"></a><b id="new">body</b></p>"#);

    assert_eq!(anchors_of_run_with_text(&runs, "body"), vec!["old", "new"]);
}

#[test]
fn a_percent_encoded_id_is_stored_decoded() {
    let runs = first_block_runs(r#"<h2 id="a%20b">Title</h2>"#);

    assert_eq!(anchors_of_run_with_text(&runs, "Title"), vec!["a b"]);
}

#[test]
fn a_run_with_no_anchor_carries_no_names() {
    let runs = first_block_runs("<p>plain body</p>");

    assert!(runs.iter().all(|run| run.anchors.is_empty()));
}

#[test]
fn a_control_character_in_an_id_is_stripped() {
    let runs = first_block_runs("<h2 id=\"sec\u{1b}tion\">Title</h2>");

    assert_eq!(anchors_of_run_with_text(&runs, "Title"), vec!["section"]);
}
