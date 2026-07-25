// @file crates/browser-css/tests/cascade.rs
// @description Verifies the reduced cascade: inline declaration mapping, inheritance, and overrides.
// @layer css
// @created meerita <meerita@icloud.com>

use browser_css::{
    cascade, Color, DisplayMode, Emphasis, ListMarker, TextStyle, TextTransform, WhiteSpace,
};
use browser_html::{InlineRun, SemanticNode};

/// A paragraph carrying the given inline `style` string, which the cascade parses.
fn paragraph_with_style(inline_style: &str) -> SemanticNode {
    SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(String::from("text"))],
        inline_style: Some(String::from(inline_style)),
    }
}

/// A paragraph with no inline style, so its computed style is inherited plus user-agent.
fn plain_paragraph() -> SemanticNode {
    SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(String::from("text"))],
        inline_style: None,
    }
}

fn cascade_from_default(node: &SemanticNode) -> TextStyle {
    cascade(&TextStyle::default(), node)
}

#[test]
fn color_declaration_sets_the_foreground() {
    let style = cascade_from_default(&paragraph_with_style("color: red"));

    assert_eq!(style.foreground, Some(Color::Red));
}

#[test]
fn background_color_declaration_sets_the_background() {
    let style = cascade_from_default(&paragraph_with_style("background-color: blue"));

    assert_eq!(style.background, Some(Color::Blue));
}

#[test]
fn bold_font_weight_keyword_maps_to_bold_emphasis() {
    let style = cascade_from_default(&paragraph_with_style("font-weight: bold"));

    assert_eq!(style.emphasis, Emphasis::Bold);
}

#[test]
fn heavy_numeric_font_weight_maps_to_bold_emphasis() {
    let style = cascade_from_default(&paragraph_with_style("font-weight: 700"));

    assert_eq!(style.emphasis, Emphasis::Bold);
}

#[test]
fn pre_white_space_declaration_is_mapped() {
    let style = cascade_from_default(&paragraph_with_style("white-space: pre"));

    assert_eq!(style.white_space, WhiteSpace::Pre);
}

#[test]
fn uppercase_text_transform_is_mapped() {
    let style = cascade_from_default(&paragraph_with_style("text-transform: uppercase"));

    assert_eq!(style.text_transform, TextTransform::Uppercase);
}

#[test]
fn underline_text_decoration_sets_underline() {
    let style = cascade_from_default(&paragraph_with_style("text-decoration: underline"));

    assert!(style.underline);
    assert!(!style.strike);
}

#[test]
fn line_through_text_decoration_sets_strike() {
    let style = cascade_from_default(&paragraph_with_style("text-decoration: line-through"));

    assert!(style.strike);
    assert!(!style.underline);
}

#[test]
fn list_style_type_declaration_sets_the_marker() {
    let style = cascade_from_default(&paragraph_with_style("list-style-type: decimal"));

    assert_eq!(style.list_marker, Some(ListMarker::Decimal));
}

#[test]
fn display_none_makes_the_node_hidden() {
    let style = cascade_from_default(&paragraph_with_style("display: none"));

    assert_eq!(style.display_mode, DisplayMode::Hidden);
}

#[test]
fn visibility_hidden_clears_the_visible_flag() {
    let style = cascade_from_default(&paragraph_with_style("visibility: hidden"));

    assert!(!style.visible);
}

#[test]
fn unknown_property_is_ignored_and_leaves_the_user_agent_style() {
    let style = cascade_from_default(&paragraph_with_style("margin: 5px"));

    assert_eq!(style.emphasis, Emphasis::None);
    assert_eq!(style.foreground, None);
    assert!(!style.underline);
    assert_eq!(style.spacing_before, 0);
    assert_eq!(style.spacing_after, 1);
}

#[test]
fn unknown_color_value_is_ignored_and_leaves_no_foreground() {
    let style = cascade_from_default(&paragraph_with_style("color: chartreuse"));

    assert_eq!(style.foreground, None);
}

#[test]
fn malformed_style_string_leaves_the_user_agent_style() {
    let style = cascade_from_default(&paragraph_with_style("$$$ ;; : :"));

    assert_eq!(style.emphasis, Emphasis::None);
    assert_eq!(style.foreground, None);
    assert!(!style.underline);
    assert_eq!(style.spacing_before, 0);
    assert_eq!(style.spacing_after, 1);
}

#[test]
fn one_valid_declaration_survives_a_malformed_neighbor() {
    let style = cascade_from_default(&paragraph_with_style(
        "color: red; nonsense; font-weight: bold",
    ));

    assert_eq!(style.foreground, Some(Color::Red));
    assert_eq!(style.emphasis, Emphasis::Bold);
}

#[test]
fn inherited_foreground_flows_to_a_child_without_its_own_color() {
    let inherited = TextStyle {
        foreground: Some(Color::Red),
        ..TextStyle::default()
    };

    let style = cascade(&inherited, &plain_paragraph());

    assert_eq!(style.foreground, Some(Color::Red));
}

#[test]
fn spacing_does_not_inherit_from_the_parent() {
    let inherited = TextStyle {
        spacing_before: 4,
        spacing_after: 4,
        ..TextStyle::default()
    };

    let style = cascade(&inherited, &plain_paragraph());

    assert_eq!(style.spacing_before, 0);
    assert_eq!(style.spacing_after, 1);
}

#[test]
fn a_child_color_overrides_the_inherited_one() {
    let inherited = TextStyle {
        foreground: Some(Color::Red),
        ..TextStyle::default()
    };

    let style = cascade(&inherited, &paragraph_with_style("color: blue"));

    assert_eq!(style.foreground, Some(Color::Blue));
}

#[test]
fn inline_font_weight_overrides_the_user_agent_bold_heading() {
    let heading = SemanticNode::Heading {
        level: 2,
        runs: vec![InlineRun::plain(String::from("Title"))],
        inline_style: Some(String::from("font-weight: normal")),
    };

    let style = cascade_from_default(&heading);

    assert_eq!(style.emphasis, Emphasis::None);
}

#[test]
fn a_heading_with_no_inline_style_keeps_its_user_agent_bold() {
    let heading = SemanticNode::Heading {
        level: 2,
        runs: vec![InlineRun::plain(String::from("Title"))],
        inline_style: None,
    };

    let style = cascade_from_default(&heading);

    assert_eq!(style.emphasis, Emphasis::Bold);
    assert_eq!(style.spacing_before, 0);
}
