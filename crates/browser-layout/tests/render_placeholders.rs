// @file crates/browser-layout/tests/render_placeholders.rs
// @description Behavior tests for rendering inert form controls, disclosures, landmarks, and figures.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_html::{Document, InlineRun, InputKind, LandmarkRole, SemanticNode};
use browser_layout::{render_document, CellBuffer};

fn document_of(nodes: Vec<SemanticNode>) -> Document {
    Document::new(nodes, None, 0)
}

fn paragraph(text: &str) -> SemanticNode {
    SemanticNode::Paragraph {
        runs: vec![InlineRun::plain(text.to_string())],
        inline_style: None,
    }
}

fn row_text(buffer: &CellBuffer, row: u16) -> String {
    (0..buffer.width())
        .filter_map(|column| buffer.cell_at(column, row))
        .map(|cell| cell.grapheme())
        .collect()
}

fn buffer_text(buffer: &CellBuffer) -> String {
    (0..buffer.height())
        .map(|row| row_text(buffer, row))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn text_input_renders_a_labelled_blank_field() {
    let input = SemanticNode::Input {
        kind: InputKind::Text,
        label: Some("Name".to_string()),
        sensitive: false,
    };
    let buffer = render_document(&document_of(vec![input]), 40).expect("input must lay out");
    assert!(buffer_text(&buffer).contains("[Name: ____]"));
}

#[test]
fn password_input_is_masked_and_never_shows_a_value() {
    let input = SemanticNode::Input {
        kind: InputKind::Password,
        label: Some("Password".to_string()),
        sensitive: true,
    };
    let buffer = render_document(&document_of(vec![input]), 40).expect("input must lay out");
    let text = buffer_text(&buffer);
    assert!(text.contains("[Password: ••••]"), "the field is masked");
    assert!(
        !text.contains('_'),
        "a masked field draws no blank underscores"
    );
}

#[test]
fn select_renders_its_first_option_and_a_dropdown_marker() {
    let select = SemanticNode::Select {
        label: Some("Country".to_string()),
        options: vec!["Spain".to_string(), "France".to_string()],
    };
    let buffer = render_document(&document_of(vec![select]), 40).expect("select must lay out");
    assert!(buffer_text(&buffer).contains("[Country: Spain ▾]"));
}

#[test]
fn button_renders_its_label_in_brackets() {
    let button = SemanticNode::Button {
        runs: vec![InlineRun::plain("Submit".to_string())],
        inline_style: None,
    };
    let buffer = render_document(&document_of(vec![button]), 40).expect("button must lay out");
    assert!(buffer_text(&buffer).contains("[ Submit ]"));
}

#[test]
fn embedded_content_renders_a_kind_placeholder() {
    let embedded = SemanticNode::EmbeddedContent {
        label: "video".to_string(),
    };
    let buffer = render_document(&document_of(vec![embedded]), 40).expect("embed must lay out");
    assert!(buffer_text(&buffer).contains("[Embedded: video]"));
}

#[test]
fn details_renders_the_summary_then_the_body_expanded() {
    let details = SemanticNode::Details {
        open: false,
        children: vec![
            SemanticNode::Summary {
                runs: vec![InlineRun::plain("More".to_string())],
                inline_style: None,
            },
            paragraph("Hidden body text"),
        ],
    };
    let buffer = render_document(&document_of(vec![details]), 40).expect("details must lay out");
    let text = buffer_text(&buffer);
    assert!(text.contains("More"), "the summary label renders");
    assert!(
        text.contains("Hidden body text"),
        "a closed details still renders its body in text"
    );
}

#[test]
fn landmark_renders_its_children_structurally() {
    let landmark = SemanticNode::Landmark {
        role: LandmarkRole::Navigation,
        children: vec![paragraph("Home")],
    };
    let buffer = render_document(&document_of(vec![landmark]), 40).expect("landmark must lay out");
    assert!(buffer_text(&buffer).contains("Home"));
}

#[test]
fn figure_renders_its_content_then_the_caption() {
    let figure = SemanticNode::Figure {
        children: vec![SemanticNode::ImagePlaceholder {
            alt: "Chart".to_string(),
            title: None,
            source: None,
        }],
        caption: Some(vec![InlineRun::plain("Quarterly sales".to_string())]),
    };
    let buffer = render_document(&document_of(vec![figure]), 40).expect("figure must lay out");
    let text = buffer_text(&buffer);
    let image_row = text.find("[Chart]").expect("the image label renders");
    let caption_row = text.find("Quarterly sales").expect("the caption renders");
    assert!(image_row < caption_row, "the caption follows the content");
}

#[test]
fn image_placeholder_includes_the_title_when_present() {
    let image = SemanticNode::ImagePlaceholder {
        alt: "Logo".to_string(),
        title: Some("Company logo".to_string()),
        source: None,
    };
    let buffer = render_document(&document_of(vec![image]), 40).expect("image must lay out");
    assert!(buffer_text(&buffer).contains("[Logo (Company logo)]"));
}
