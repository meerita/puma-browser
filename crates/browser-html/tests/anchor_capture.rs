// @file crates/browser-html/tests/anchor_capture.rs
// @description Behavior tests for capturing element id and <a name> anchors as markers and inline run anchors.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, Document, InlineRun, SemanticNode};

fn parse(source: &str) -> Document {
    parse_html(source.as_bytes(), None).expect("well-formed HTML must parse")
}

/// The inline runs of the document's first text block, whether a heading or a paragraph.
fn first_block_runs(source: &str) -> Vec<InlineRun> {
    let document = parse(source);
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

/// The children of a container variant, or an empty slice for a leaf.
fn children_of(node: &SemanticNode) -> &[SemanticNode] {
    match node {
        SemanticNode::List { children, .. }
        | SemanticNode::ListItem { children, .. }
        | SemanticNode::Table { children }
        | SemanticNode::TableRow { children }
        | SemanticNode::TableCell { children, .. }
        | SemanticNode::Quote { children, .. }
        | SemanticNode::Figure { children, .. }
        | SemanticNode::Landmark { children, .. }
        | SemanticNode::Details { children, .. } => children,
        SemanticNode::Form(form) => &form.children,
        _ => &[],
    }
}

/// Every anchor-target marker in the tree, in document order.
fn markers(document: &Document) -> Vec<Vec<String>> {
    let mut found = Vec::new();
    collect_markers(document.children(), &mut found);
    found
}

fn collect_markers(nodes: &[SemanticNode], found: &mut Vec<Vec<String>>) {
    for node in nodes {
        if let SemanticNode::AnchorTarget { names } = node {
            found.push(names.clone());
        }
        collect_markers(children_of(node), found);
    }
}

/// Every anchor name carried by an inline run anywhere in the tree, in document order.
fn run_anchors(document: &Document) -> Vec<String> {
    let mut found = Vec::new();
    collect_run_anchors(document.children(), &mut found);
    found
}

fn collect_run_anchors(nodes: &[SemanticNode], found: &mut Vec<String>) {
    for node in nodes {
        if let Some(runs) = runs_of(node) {
            found.extend(runs.into_iter().flat_map(|run| run.anchors));
        }
        collect_run_anchors(children_of(node), found);
    }
}

#[test]
fn a_heading_id_becomes_a_marker_before_the_heading() {
    let document = parse(r#"<h2 id="section">Title</h2>"#);

    assert_eq!(markers(&document), vec![vec!["section".to_string()]]);
    assert!(run_anchors(&document).is_empty());
    assert!(matches!(
        document.children().first(),
        Some(SemanticNode::AnchorTarget { .. })
    ));
    assert!(matches!(
        document.children().get(1),
        Some(SemanticNode::Heading { .. })
    ));
}

#[test]
fn a_pretty_printed_section_id_becomes_a_marker_before_its_landmark() {
    let document = parse("<section id=\"a\">\n  <p>Body</p>\n</section>");

    assert_eq!(markers(&document), vec![vec!["a".to_string()]]);
    assert!(matches!(
        document.children().first(),
        Some(SemanticNode::AnchorTarget { .. })
    ));
    assert!(matches!(
        document.children().get(1),
        Some(SemanticNode::Landmark { .. })
    ));
    assert!(
        run_anchors(&document).is_empty(),
        "the section's name belongs to its marker, not to a run inside it"
    );
}

#[test]
fn one_paragraph_id_produces_exactly_one_marker_and_no_run_anchor() {
    let document = parse(r#"<p id="only">Body</p>"#);

    assert_eq!(markers(&document), vec![vec!["only".to_string()]]);
    assert!(
        run_anchors(&document).is_empty(),
        "the block walk and the inline walk must not both record the same id"
    );
}

#[test]
fn a_container_id_becomes_a_marker_before_the_block_inside_it() {
    let document = parse(r#"<div id="intro"><p>hello</p></div>"#);

    assert_eq!(markers(&document), vec![vec!["intro".to_string()]]);
    assert!(run_anchors(&document).is_empty());
}

#[test]
fn an_empty_paragraph_still_yields_its_marker() {
    let document = parse(r#"<p id="x"></p>"#);

    assert_eq!(markers(&document), vec![vec!["x".to_string()]]);
}

#[test]
fn an_inline_span_id_attaches_to_its_run_and_emits_no_marker() {
    let document = parse(r#"<p>Body <span id="s">inner</span></p>"#);

    assert!(markers(&document).is_empty());
    assert_eq!(run_anchors(&document), vec!["s".to_string()]);
}

#[test]
fn an_anchor_name_attaches_to_its_run_and_emits_no_marker() {
    let document = parse(r#"<p><a name="n"></a>body</p>"#);

    assert!(markers(&document).is_empty());
    assert_eq!(run_anchors(&document), vec!["n".to_string()]);
}

#[test]
fn a_table_cell_id_becomes_the_first_child_of_its_cell() {
    let document = parse(r#"<table><tr><td id="c">Cell</td></tr></table>"#);

    let table = document
        .children()
        .first()
        .expect("the table must be the first node");
    let row = children_of(table)
        .first()
        .expect("the table must hold one row");
    let cell = children_of(row)
        .first()
        .expect("the row must hold one cell");
    assert!(matches!(cell, SemanticNode::TableCell { .. }));
    assert_eq!(
        children_of(cell).first(),
        Some(&SemanticNode::AnchorTarget {
            names: vec!["c".to_string()]
        })
    );
}

#[test]
fn a_table_row_id_is_not_captured() {
    let document = parse(r#"<table><tr id="r"><td>Cell</td></tr></table>"#);

    assert!(markers(&document).is_empty());
    assert!(run_anchors(&document).is_empty());
}

#[test]
fn a_percent_encoded_id_is_stored_decoded_on_the_marker() {
    let document = parse(r#"<h2 id="a%20b">Title</h2>"#);

    assert_eq!(markers(&document), vec![vec!["a b".to_string()]]);
}

#[test]
fn a_control_character_in_an_id_is_stripped_from_the_marker() {
    let document = parse("<h2 id=\"sec\u{1b}tion\">Title</h2>");

    assert_eq!(markers(&document), vec![vec!["section".to_string()]]);
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
fn a_run_with_no_anchor_carries_no_names() {
    let runs = first_block_runs("<p>plain body</p>");

    assert!(runs.iter().all(|run| run.anchors.is_empty()));
}
