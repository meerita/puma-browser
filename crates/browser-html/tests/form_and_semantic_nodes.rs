// @file crates/browser-html/tests/form_and_semantic_nodes.rs
// @description Behavior tests for parsing details, landmarks, figures, embedded content, and form controls.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{
    parse_html, parse_html_with_base, ButtonKind, FormMethod, InputKind, LandmarkRole, SemanticNode,
};

fn first_matching(source: &str, wanted: impl Fn(&SemanticNode) -> bool) -> SemanticNode {
    let document = parse_html(source.as_bytes(), None).expect("well-formed HTML must parse");
    find_matching(document.children(), &wanted).expect("a matching node must be produced")
}

fn first_matching_with_base(source: &str, document_url: Option<&str>) -> SemanticNode {
    let document = parse_html_with_base(source.as_bytes(), None, document_url)
        .expect("well-formed HTML must parse");
    find_matching(document.children(), &|node| {
        matches!(node, SemanticNode::Form(_))
    })
    .expect("a form must be produced")
}

fn all_matching(source: &str, wanted: impl Fn(&SemanticNode) -> bool) -> Vec<SemanticNode> {
    let document = parse_html(source.as_bytes(), None).expect("well-formed HTML must parse");
    let mut matches = Vec::new();
    collect_matching(document.children(), &wanted, &mut matches);
    matches
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

fn collect_matching(
    nodes: &[SemanticNode],
    wanted: &impl Fn(&SemanticNode) -> bool,
    matches: &mut Vec<SemanticNode>,
) {
    for node in nodes {
        if wanted(node) {
            matches.push(node.clone());
        }
        collect_matching(children_of(node), wanted, matches);
    }
}

fn children_of(node: &SemanticNode) -> &[SemanticNode] {
    match node {
        SemanticNode::Landmark { children, .. }
        | SemanticNode::Details { children, .. }
        | SemanticNode::Figure { children, .. } => children,
        SemanticNode::Form(form) => &form.children,
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
    let node = first_matching(
        r#"<form><input type="password" value="hunter2" aria-label="Password"></form>"#,
        |node| matches!(node, SemanticNode::Input(_)),
    );

    // A password's value/checked attribute is never read, so the debug form is the
    // strongest available check that no source value reaches the tree.
    let debug = format!("{node:?}");
    let SemanticNode::Input(input) = node else {
        panic!("expected an input");
    };
    assert_eq!(input.kind, InputKind::Password);
    assert!(input.sensitive, "a password input is sensitive");
    assert_eq!(input.label.as_deref(), Some("Password"));
    assert_eq!(input.value, "", "a password value never enters the tree");
    assert!(!input.checked);
    assert!(
        !debug.contains("hunter2"),
        "no input value reaches the tree"
    );
}

#[test]
fn text_input_defaults_to_text_kind_and_is_not_sensitive() {
    let node = first_matching("<form><input></form>", |node| {
        matches!(node, SemanticNode::Input(_))
    });
    let SemanticNode::Input(input) = node else {
        panic!("expected an input");
    };
    assert_eq!(input.kind, InputKind::Text);
    assert!(!input.sensitive);
}

#[test]
fn label_for_attribute_associates_a_control_label() {
    let node = first_matching(
        r#"<form><label for="e">Email</label><input id="e" type="email"></form>"#,
        |node| matches!(node, SemanticNode::Input(_)),
    );
    let SemanticNode::Input(input) = node else {
        panic!("expected an input");
    };
    assert_eq!(input.label.as_deref(), Some("Email"));
}

#[test]
fn select_options_are_captured_in_order() {
    let node = first_matching(
        "<form><select><option>Spain</option><option>France</option></select></form>",
        |node| matches!(node, SemanticNode::Select(_)),
    );
    let SemanticNode::Select(select) = node else {
        panic!("expected a select");
    };
    let labels: Vec<&str> = select
        .options
        .iter()
        .map(|option| option.label.as_str())
        .collect();
    assert_eq!(labels, vec!["Spain", "France"]);
}

#[test]
fn button_text_survives_as_inline_runs() {
    let node = first_matching("<form><button>Send</button></form>", |node| {
        matches!(node, SemanticNode::Button(_))
    });
    let SemanticNode::Button(button) = node else {
        panic!("expected a button");
    };
    // A <button> with no type attribute defaults to Submit, matching HTML.
    assert_eq!(button.kind, ButtonKind::Submit);
    assert_eq!(
        button
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "Send"
    );
}

#[test]
fn a_buttons_own_type_attribute_selects_its_kind() {
    let submit = first_matching(
        r#"<form><button type="submit">Go</button></form>"#,
        |node| matches!(node, SemanticNode::Button(_)),
    );
    let SemanticNode::Button(submit) = submit else {
        panic!("expected a button");
    };
    assert_eq!(submit.kind, ButtonKind::Submit);

    let reset = first_matching(
        r#"<form><button type="reset">Clear</button></form>"#,
        |node| matches!(node, SemanticNode::Button(_)),
    );
    let SemanticNode::Button(reset) = reset else {
        panic!("expected a button");
    };
    assert_eq!(reset.kind, ButtonKind::Reset);

    let plain = first_matching(
        r#"<form><button type="button">Toggle</button></form>"#,
        |node| matches!(node, SemanticNode::Button(_)),
    );
    let SemanticNode::Button(plain) = plain else {
        panic!("expected a button");
    };
    assert_eq!(plain.kind, ButtonKind::Button);
}

#[test]
fn node_ids_are_assigned_in_document_order_across_mixed_control_types() {
    let node = first_matching(
        r#"<form><input name="a"><select><option>One</option></select><input type="submit" value="Go"></form>"#,
        |node| matches!(node, SemanticNode::Form(_)),
    );
    let SemanticNode::Form(form) = node else {
        panic!("expected a form");
    };
    let ids: Vec<u32> = form
        .children
        .iter()
        .map(|child| match child {
            SemanticNode::Input(input) => input.id.value(),
            SemanticNode::Select(select) => select.id.value(),
            SemanticNode::Button(button) => button.id.value(),
            other => panic!("unexpected node: {other:?}"),
        })
        .collect();
    assert!(
        ids.windows(2).all(|pair| pair[0] < pair[1]),
        "ids increase in document order: {ids:?}"
    );
    assert!(
        form.id.value() < ids[0],
        "the form's own id precedes its controls' ids"
    );
}

#[test]
fn submit_reset_and_button_inputs_normalize_to_button_not_input() {
    let submit = first_matching(r#"<form><input type="submit" value="Go"></form>"#, |node| {
        matches!(node, SemanticNode::Button(_))
    });
    let SemanticNode::Button(submit) = submit else {
        panic!("expected a button");
    };
    assert_eq!(submit.kind, ButtonKind::Submit);
    assert_eq!(
        submit
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "Go"
    );

    let reset = first_matching(r#"<form><input type="reset"></form>"#, |node| {
        matches!(node, SemanticNode::Button(_))
    });
    let SemanticNode::Button(reset) = reset else {
        panic!("expected a button");
    };
    assert_eq!(reset.kind, ButtonKind::Reset);
    assert_eq!(
        reset
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "Reset"
    );

    let button = first_matching(r#"<form><input type="button" value="X"></form>"#, |node| {
        matches!(node, SemanticNode::Button(_))
    });
    let SemanticNode::Button(button) = button else {
        panic!("expected a button");
    };
    assert_eq!(button.kind, ButtonKind::Button);
    assert_eq!(
        button
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect::<String>(),
        "X"
    );

    assert!(
        all_matching(r#"<form><input type="submit" value="Go"></form>"#, |node| {
            matches!(node, SemanticNode::Input(_))
        })
        .is_empty(),
        "a normalized submit input never produces an Input node"
    );
}

#[test]
fn form_action_resolves_to_the_document_url_when_absent() {
    let node = first_matching_with_base(
        "<form><button>Go</button></form>",
        Some("https://example.com/docs/index.html"),
    );
    let SemanticNode::Form(form) = node else {
        panic!("expected a form");
    };
    assert_eq!(form.action, "https://example.com/docs/index.html");
}

#[test]
fn form_action_resolves_a_relative_reference_against_the_base_url() {
    let node = first_matching_with_base(
        r#"<form action="submit.php"><button>Go</button></form>"#,
        Some("https://example.com/docs/index.html"),
    );
    let SemanticNode::Form(form) = node else {
        panic!("expected a form");
    };
    assert_eq!(form.action, "https://example.com/docs/submit.php");
}

#[test]
fn form_method_post_and_uppercase_post_both_yield_post() {
    for markup in [
        r#"<form method="POST"><button>Go</button></form>"#,
        r#"<form method="post"><button>Go</button></form>"#,
    ] {
        let node = first_matching(markup, |node| matches!(node, SemanticNode::Form(_)));
        let SemanticNode::Form(form) = node else {
            panic!("expected a form");
        };
        assert_eq!(form.method, FormMethod::Post);
    }
}

#[test]
fn form_method_defaults_to_get_when_absent_or_unrecognized() {
    for markup in [
        "<form><button>Go</button></form>",
        r#"<form method="put"><button>Go</button></form>"#,
    ] {
        let node = first_matching(markup, |node| matches!(node, SemanticNode::Form(_)));
        let SemanticNode::Form(form) = node else {
            panic!("expected a form");
        };
        assert_eq!(form.method, FormMethod::Get);
    }
}

#[test]
fn multiple_select_keeps_every_selected_option() {
    let node = first_matching(
        r#"<form><select multiple><option selected>A</option><option selected>B</option><option>C</option></select></form>"#,
        |node| matches!(node, SemanticNode::Select(_)),
    );
    let SemanticNode::Select(select) = node else {
        panic!("expected a select");
    };
    let selected: Vec<&str> = select
        .options
        .iter()
        .filter(|option| option.selected)
        .map(|option| option.label.as_str())
        .collect();
    assert_eq!(selected, vec!["A", "B"]);
}

#[test]
fn non_multiple_select_keeps_only_the_last_selected_option() {
    let node = first_matching(
        r#"<form><select><option selected>A</option><option selected>B</option></select></form>"#,
        |node| matches!(node, SemanticNode::Select(_)),
    );
    let SemanticNode::Select(select) = node else {
        panic!("expected a select");
    };
    let selected: Vec<&str> = select
        .options
        .iter()
        .filter(|option| option.selected)
        .map(|option| option.label.as_str())
        .collect();
    assert_eq!(selected, vec!["B"]);
}

#[test]
fn select_with_more_than_the_option_limit_is_truncated_not_rejected() {
    let options: String = (0..600)
        .map(|index| format!("<option>{index}</option>"))
        .collect();
    let source = format!("<form><select>{options}</select></form>");
    let node = first_matching(&source, |node| matches!(node, SemanticNode::Select(_)));
    let SemanticNode::Select(select) = node else {
        panic!("expected a select");
    };
    assert_eq!(select.options.len(), 500, "collection stops at the limit");
}

#[test]
fn checkbox_input_captures_its_name_and_checked_state() {
    let node = first_matching(
        r#"<form><input type="checkbox" name="agree" checked value="yes"></form>"#,
        |node| matches!(node, SemanticNode::Input(_)),
    );
    let SemanticNode::Input(input) = node else {
        panic!("expected an input");
    };
    assert_eq!(input.kind, InputKind::Checkbox);
    assert_eq!(input.name.as_deref(), Some("agree"));
    assert!(input.checked);
    assert_eq!(input.value, "yes");
}

#[test]
fn textarea_value_comes_from_its_text_content_not_an_attribute() {
    let node = first_matching(
        r#"<form><textarea name="bio">Hello  world</textarea></form>"#,
        |node| matches!(node, SemanticNode::Textarea(_)),
    );
    let SemanticNode::Textarea(textarea) = node else {
        panic!("expected a textarea");
    };
    assert_eq!(textarea.name.as_deref(), Some("bio"));
    assert_eq!(textarea.value, "Hello world");
}

#[test]
fn button_captures_its_name_and_value_attributes() {
    let node = first_matching(
        r#"<form><button name="action" value="save">Save</button></form>"#,
        |node| matches!(node, SemanticNode::Button(_)),
    );
    let SemanticNode::Button(button) = node else {
        panic!("expected a button");
    };
    assert_eq!(button.name.as_deref(), Some("action"));
    assert_eq!(button.value.as_deref(), Some("save"));
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
