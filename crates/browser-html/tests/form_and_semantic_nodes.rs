// @file crates/browser-html/tests/form_and_semantic_nodes.rs
// @description Behavior tests for parsing details, landmarks, figures, embedded content, and form controls.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, InputKind, LandmarkRole, SemanticNode};

fn first_matching(source: &str, wanted: impl Fn(&SemanticNode) -> bool) -> SemanticNode {
    let document = parse_html(source.as_bytes(), None).expect("well-formed HTML must parse");
    find_matching(document.children(), &wanted).expect("a matching node must be produced")
}

fn find_matching(
    nodes: &[SemanticNode],
    wanted: &impl Fn(&SemanticNode) -> bool,
) -> Option<SemanticNode> {
    for node in nodes {
        if wanted(node) {
            return Some(node.clone());
        }
        if let Some(found) = find_matching(children_of(node), wanted) {
            return Some(found);
        }
    }
    None
}

fn children_of(node: &SemanticNode) -> &[SemanticNode] {
    match node {
        SemanticNode::Form { children }
        | SemanticNode::Landmark { children, .. }
        | SemanticNode::Details { children, .. }
        | SemanticNode::Figure { children, .. } => children,
        _ => &[],
    }
}

#[test]
fn details_yields_a_summary_and_body_child_in_order() {
    let details = first_matching(
        "<details><summary>More</summary><p>body</p></details>",
        |node| matches!(node, SemanticNode::Details { .. }),
    );

    let SemanticNode::Details { open, children } = details else {
        panic!("expected details");
    };
    assert!(!open, "a details without the open attribute is closed");
    assert!(
        matches!(&children[0], SemanticNode::Summary { .. }),
        "the summary comes first"
    );
    assert!(
        matches!(&children[1], SemanticNode::Paragraph { .. }),
        "the body paragraph follows the summary"
    );
}

#[test]
fn details_with_open_attribute_is_marked_open() {
    let details = first_matching(
        "<details open><summary>S</summary><p>b</p></details>",
        |node| matches!(node, SemanticNode::Details { .. }),
    );
    assert!(matches!(details, SemanticNode::Details { open: true, .. }));
}

#[test]
fn nav_element_becomes_a_navigation_landmark() {
    let landmark = first_matching("<nav><p>Menu</p></nav>", |node| {
        matches!(node, SemanticNode::Landmark { .. })
    });
    assert!(matches!(
        landmark,
        SemanticNode::Landmark {
            role: LandmarkRole::Navigation,
            ..
        }
    ));
}

#[test]
fn aside_and_footer_map_to_their_landmark_roles() {
    let aside = first_matching("<aside><p>x</p></aside>", |node| {
        matches!(node, SemanticNode::Landmark { .. })
    });
    assert!(matches!(
        aside,
        SemanticNode::Landmark {
            role: LandmarkRole::Complementary,
            ..
        }
    ));

    let footer = first_matching("<footer><p>x</p></footer>", |node| {
        matches!(node, SemanticNode::Landmark { .. })
    });
    assert!(matches!(
        footer,
        SemanticNode::Landmark {
            role: LandmarkRole::ContentInfo,
            ..
        }
    ));
}

#[test]
fn aria_role_attribute_overrides_the_element_landmark_role() {
    let landmark = first_matching(r#"<section role="search"><p>x</p></section>"#, |node| {
        matches!(node, SemanticNode::Landmark { .. })
    });
    assert!(
        matches!(
            landmark,
            SemanticNode::Landmark {
                role: LandmarkRole::Search,
                ..
            }
        ),
        "an explicit ARIA role wins over the section default"
    );
}

#[test]
fn password_input_is_sensitive_and_carries_no_value() {
    let input = first_matching(
        r#"<form><input type="password" value="hunter2" aria-label="Password"></form>"#,
        |node| matches!(node, SemanticNode::Input { .. }),
    );

    // The Input variant has no value field at all, so no typed or authored value can
    // ever enter the tree; the debug form is the strongest available check.
    let debug = format!("{input:?}");
    let SemanticNode::Input {
        kind,
        label,
        sensitive,
    } = input
    else {
        panic!("expected an input");
    };
    assert_eq!(kind, InputKind::Password);
    assert!(sensitive, "a password input is sensitive");
    assert_eq!(label.as_deref(), Some("Password"));
    assert!(
        !debug.contains("hunter2"),
        "no input value reaches the tree"
    );
}

#[test]
fn text_input_defaults_to_text_kind_and_is_not_sensitive() {
    let input = first_matching("<form><input></form>", |node| {
        matches!(node, SemanticNode::Input { .. })
    });
    assert!(matches!(
        input,
        SemanticNode::Input {
            kind: InputKind::Text,
            sensitive: false,
            ..
        }
    ));
}

#[test]
fn label_for_attribute_associates_a_control_label() {
    let input = first_matching(
        r#"<form><label for="e">Email</label><input id="e" type="email"></form>"#,
        |node| matches!(node, SemanticNode::Input { .. }),
    );
    let SemanticNode::Input { label, .. } = input else {
        panic!("expected an input");
    };
    assert_eq!(label.as_deref(), Some("Email"));
}

#[test]
fn select_options_are_captured_in_order() {
    let select = first_matching(
        "<form><select><option>Spain</option><option>France</option></select></form>",
        |node| matches!(node, SemanticNode::Select { .. }),
    );
    let SemanticNode::Select { options, .. } = select else {
        panic!("expected a select");
    };
    assert_eq!(options, vec!["Spain".to_string(), "France".to_string()]);
}

#[test]
fn button_text_survives_as_inline_runs() {
    let button = first_matching("<form><button>Send</button></form>", |node| {
        matches!(node, SemanticNode::Button { .. })
    });
    let SemanticNode::Button { runs, .. } = button else {
        panic!("expected a button");
    };
    assert_eq!(
        runs.iter().map(|run| run.text.as_str()).collect::<String>(),
        "Send"
    );
}

#[test]
fn embedded_elements_map_to_their_kind_label() {
    let iframe = first_matching(r#"<iframe src="/x"></iframe>"#, |node| {
        matches!(node, SemanticNode::EmbeddedContent { .. })
    });
    assert!(matches!(
        iframe,
        SemanticNode::EmbeddedContent { label } if label == "inline frame"
    ));

    let video = first_matching("<video></video>", |node| {
        matches!(node, SemanticNode::EmbeddedContent { .. })
    });
    assert!(matches!(
        video,
        SemanticNode::EmbeddedContent { label } if label == "video"
    ));
}

#[test]
fn figure_lifts_its_figcaption_out_as_the_caption() {
    let figure = first_matching(
        r#"<figure><img alt="Chart"><figcaption>Sales</figcaption></figure>"#,
        |node| matches!(node, SemanticNode::Figure { .. }),
    );
    let SemanticNode::Figure { children, caption } = figure else {
        panic!("expected a figure");
    };
    assert!(
        matches!(&children[0], SemanticNode::ImagePlaceholder { .. }),
        "the image stays a figure child"
    );
    let caption = caption.expect("the figcaption becomes the caption");
    assert_eq!(
        caption
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "Sales"
    );
}
