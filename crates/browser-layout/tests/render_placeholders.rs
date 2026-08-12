// @file crates/browser-layout/tests/render_placeholders.rs
// @description Behavior tests for rendering inert form controls, disclosures, landmarks, and figures.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_html::{
    ButtonElement, ButtonKind, Document, InlineRun, InputElement, InputKind, LandmarkRole, NodeId,
    SelectElement, SelectOption, SemanticNode, TextareaElement,
};
use browser_layout::{render_document, CellBuffer, FieldSpanKind, WidthConfig};

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

fn select_option(label: &str, selected: bool) -> SelectOption {
    SelectOption {
        value: label.to_string(),
        label: label.to_string(),
        selected,
    }
}

#[test]
fn text_input_renders_a_labelled_blank_field() {
    let input = SemanticNode::Input(InputElement {
        id: NodeId::new(0),
        kind: InputKind::Text,
        name: None,
        value: String::new(),
        checked: false,
        label: Some("Name".to_string()),
        sensitive: false,
    });
    let buffer = render_document(&document_of(vec![input]), 40, &WidthConfig::default())
        .expect("input must lay out");
    assert!(buffer_text(&buffer).contains("[Name: ____]"));
}

#[test]
fn password_input_is_masked_and_never_shows_a_value() {
    let input = SemanticNode::Input(InputElement {
        id: NodeId::new(0),
        kind: InputKind::Password,
        name: None,
        value: String::new(),
        checked: false,
        label: Some("Password".to_string()),
        sensitive: true,
    });
    let buffer = render_document(&document_of(vec![input]), 40, &WidthConfig::default())
        .expect("input must lay out");
    let text = buffer_text(&buffer);
    assert!(text.contains("[Password: ••••]"), "the field is masked");
    assert!(
        !text.contains('_'),
        "a masked field draws no blank underscores"
    );
}

#[test]
fn hidden_input_produces_no_row_and_no_field_span() {
    let input = SemanticNode::Input(InputElement {
        id: NodeId::new(0),
        kind: InputKind::Hidden,
        name: Some("csrf".to_string()),
        value: "token".to_string(),
        checked: false,
        label: None,
        sensitive: false,
    });
    let buffer = render_document(&document_of(vec![input]), 40, &WidthConfig::default())
        .expect("hidden input must not error during layout");
    assert_eq!(buffer.height(), 0, "a hidden input produces no rows");
    assert!(
        buffer.field_spans().is_empty(),
        "a hidden input produces no field span"
    );
}

#[test]
fn text_input_produces_exactly_one_field_span_carrying_its_node_id() {
    let input = SemanticNode::Input(InputElement {
        id: NodeId::new(7),
        kind: InputKind::Text,
        name: None,
        value: String::new(),
        checked: false,
        label: Some("Name".to_string()),
        sensitive: false,
    });
    let buffer = render_document(&document_of(vec![input]), 40, &WidthConfig::default())
        .expect("input must lay out");
    let spans = buffer.field_spans();
    assert_eq!(spans.len(), 1, "one span per row the control occupies");
    assert_eq!(spans[0].node_id, NodeId::new(7));
    assert_eq!(spans[0].kind, FieldSpanKind::Input);
}

#[test]
fn select_renders_its_selected_option_and_a_dropdown_marker() {
    let select = SemanticNode::Select(SelectElement {
        id: NodeId::new(0),
        name: None,
        label: Some("Country".to_string()),
        multiple: false,
        options: vec![
            select_option("Spain", false),
            select_option("France", false),
        ],
    });
    let buffer = render_document(&document_of(vec![select]), 40, &WidthConfig::default())
        .expect("select must lay out");
    assert!(buffer_text(&buffer).contains("[Country: Spain ▾]"));
    let spans = buffer.field_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, FieldSpanKind::Select);
}

#[test]
fn button_renders_its_label_in_brackets() {
    let button = SemanticNode::Button(ButtonElement {
        id: NodeId::new(0),
        kind: ButtonKind::Button,
        name: None,
        value: None,
        runs: vec![InlineRun::plain("Submit".to_string())],
        inline_style: None,
    });
    let buffer = render_document(&document_of(vec![button]), 40, &WidthConfig::default())
        .expect("button must lay out");
    assert!(buffer_text(&buffer).contains("[ Submit ]"));
    let spans = buffer.field_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, FieldSpanKind::Button);
}

#[test]
fn textarea_renders_a_labelled_blank_field_with_a_field_span() {
    let textarea = SemanticNode::Textarea(TextareaElement {
        id: NodeId::new(0),
        name: None,
        value: "ignored for the static placeholder".to_string(),
        label: Some("Bio".to_string()),
    });
    let buffer = render_document(&document_of(vec![textarea]), 40, &WidthConfig::default())
        .expect("textarea must lay out");
    assert!(buffer_text(&buffer).contains("[Bio: ____]"));
    let spans = buffer.field_spans();
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].kind, FieldSpanKind::Textarea);
}

#[test]
fn embedded_content_renders_a_kind_placeholder() {
    let embedded = SemanticNode::EmbeddedContent {
        label: "video".to_string(),
    };
    let buffer = render_document(&document_of(vec![embedded]), 40, &WidthConfig::default())
        .expect("embed must lay out");
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
    let buffer = render_document(&document_of(vec![details]), 40, &WidthConfig::default())
        .expect("details must lay out");
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
    let buffer = render_document(&document_of(vec![landmark]), 40, &WidthConfig::default())
        .expect("landmark must lay out");
    assert!(buffer_text(&buffer).contains("Home"));
}

#[test]
fn figure_with_suppressed_image_renders_only_the_caption() {
    let figure = SemanticNode::Figure {
        children: vec![SemanticNode::ImagePlaceholder {
            alt: "Chart".to_string(),
            title: None,
            source: None,
        }],
        caption: Some(vec![InlineRun::plain("Quarterly sales".to_string())]),
    };
    let buffer = render_document(&document_of(vec![figure]), 40, &WidthConfig::default())
        .expect("figure must lay out");
    let text = buffer_text(&buffer);
    assert!(text.contains("Quarterly sales"), "the caption renders");
    assert!(!text.contains("[Chart]"), "the image label is suppressed");
}

#[test]
fn image_placeholder_produces_no_output() {
    let image = SemanticNode::ImagePlaceholder {
        alt: "Logo".to_string(),
        title: Some("Company logo".to_string()),
        source: None,
    };
    let buffer = render_document(&document_of(vec![image]), 40, &WidthConfig::default())
        .expect("image must not error during layout");
    assert_eq!(buffer.height(), 0, "an image placeholder produces no rows");
}
