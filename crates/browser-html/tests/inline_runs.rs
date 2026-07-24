// @file crates/browser-html/tests/inline_runs.rs
// @description Behavior tests for inline run splitting: emphasis, links, and base-URL resolution.
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

fn strong() -> InlineEmphasis {
    InlineEmphasis {
        strong: true,
        emphasis: false,
        code: false,
    }
}

#[test]
fn emphasis_boundaries_split_a_block_into_separate_runs() {
    let runs = paragraph_runs("<p>plain <strong>bold</strong> tail</p>");

    assert_eq!(
        runs,
        vec![
            InlineRun::plain("plain ".to_string()),
            InlineRun {
                text: "bold".to_string(),
                emphasis: strong(),
                link: None,
            },
            InlineRun::plain(" tail".to_string()),
        ]
    );
}

#[test]
fn acceptance_paragraph_splits_into_plain_bold_space_and_link_runs() {
    let runs = paragraph_runs(r#"<p>plain <strong>bold</strong> <a href="/x">link</a></p>"#);

    assert_eq!(
        runs,
        vec![
            InlineRun::plain("plain ".to_string()),
            InlineRun {
                text: "bold".to_string(),
                emphasis: strong(),
                link: None,
            },
            InlineRun::plain(" ".to_string()),
            InlineRun {
                text: "link".to_string(),
                emphasis: InlineEmphasis::none(),
                link: Some("/x".to_string()),
            },
        ]
    );
}

#[test]
fn nested_strong_and_emphasis_union_their_flags_on_one_run() {
    let runs = paragraph_runs("<p><strong><em>both</em></strong></p>");

    assert_eq!(
        runs,
        vec![InlineRun {
            text: "both".to_string(),
            emphasis: InlineEmphasis {
                strong: true,
                emphasis: true,
                code: false,
            },
            link: None,
        }]
    );
}

#[test]
fn inline_code_marks_its_run_with_the_code_flag() {
    let runs = paragraph_runs("<p>use <code>fn</code> here</p>");

    assert_eq!(
        runs,
        vec![
            InlineRun::plain("use ".to_string()),
            InlineRun {
                text: "fn".to_string(),
                emphasis: InlineEmphasis {
                    strong: false,
                    emphasis: false,
                    code: true,
                },
                link: None,
            },
            InlineRun::plain(" here".to_string()),
        ]
    );
}

#[test]
fn inline_anchor_sets_the_link_on_its_run() {
    let runs = paragraph_runs(r#"<p><a href="https://example.com/">home</a></p>"#);

    assert_eq!(
        runs,
        vec![InlineRun {
            text: "home".to_string(),
            emphasis: InlineEmphasis::none(),
            link: Some("https://example.com/".to_string()),
        }]
    );
}

#[test]
fn anchor_without_href_contributes_a_plain_run() {
    let runs = paragraph_runs("<p><a>bare</a></p>");

    assert_eq!(runs, vec![InlineRun::plain("bare".to_string())]);
}

#[test]
fn base_href_resolves_a_relative_link_reference() {
    let runs =
        paragraph_runs(r#"<base href="https://example.com/docs/"><p><a href="page">next</a></p>"#);

    assert_eq!(
        runs,
        vec![InlineRun {
            text: "next".to_string(),
            emphasis: InlineEmphasis::none(),
            link: Some("https://example.com/docs/page".to_string()),
        }]
    );
}

#[test]
fn base_href_resolves_a_relative_image_source() {
    let document = parse_html(
        r#"<base href="https://example.com/docs/"><img alt="A diagram" src="pic.png">"#.as_bytes(),
        None,
    )
    .expect("well-formed HTML must parse");

    let source = document.children().iter().find_map(|node| match node {
        SemanticNode::ImagePlaceholder { source, .. } => source.clone(),
        _ => None,
    });

    assert_eq!(source, Some("https://example.com/docs/pic.png".to_string()));
}

#[test]
fn a_reference_is_kept_as_authored_when_no_base_is_present() {
    let runs = paragraph_runs(r#"<p><a href="page">next</a></p>"#);

    assert_eq!(
        runs,
        vec![InlineRun {
            text: "next".to_string(),
            emphasis: InlineEmphasis::none(),
            link: Some("page".to_string()),
        }]
    );
}

#[test]
fn unmarked_text_still_yields_a_single_plain_run() {
    let runs = paragraph_runs("<p>just some words</p>");

    assert_eq!(runs, vec![InlineRun::plain("just some words".to_string())]);
}
