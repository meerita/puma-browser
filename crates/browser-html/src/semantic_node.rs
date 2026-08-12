// @file crates/browser-html/src/semantic_node.rs
// @description Semantic document node enum consumed by the css, layout, terminal, and mcp layers.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::button_element::ButtonElement;
use crate::form_element::FormElement;
use crate::inline_run::InlineRun;
use crate::input_element::InputElement;
use crate::landmark_role::LandmarkRole;
use crate::select_element::SelectElement;
use crate::textarea_element::TextareaElement;

/// A single node in the browser's semantic document tree.
///
/// The tree is the browser's internal representation of a page, produced from parsed
/// HTML and consumed downstream instead of the raw DOM. It is a recursive, owned tree:
/// container variants hold their `children` directly, and text-bearing variants hold
/// their text as inline runs. Each variant carries only the fields its meaning already
/// requires at this stage; `source` is a plain `String` because URL validation belongs
/// to the network layer, not here. The document root is the [`Document`] struct itself,
/// which owns the top-level children.
///
/// Style-bearing variants carry `inline_style`: the raw, control-character-stripped
/// value of the element's `style` attribute, or `None` when the element had none. The
/// string is kept unparsed on purpose so the CSS layer, not the HTML layer, owns CSS
/// interpretation; `browser-html` never depends on `browser-css`.
///
/// [`Document`]: crate::Document
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticNode {
    Heading {
        level: u8,
        runs: Vec<InlineRun>,
        inline_style: Option<String>,
    },
    Paragraph {
        runs: Vec<InlineRun>,
        inline_style: Option<String>,
    },
    List {
        ordered: bool,
        children: Vec<SemanticNode>,
        inline_style: Option<String>,
    },
    ListItem {
        children: Vec<SemanticNode>,
        inline_style: Option<String>,
    },
    Table {
        children: Vec<SemanticNode>,
    },
    TableRow {
        children: Vec<SemanticNode>,
    },
    TableCell {
        header: bool,
        children: Vec<SemanticNode>,
        inline_style: Option<String>,
    },
    Quote {
        children: Vec<SemanticNode>,
        inline_style: Option<String>,
    },
    CodeBlock {
        text: String,
    },
    PreformattedBlock {
        text: String,
    },
    ImagePlaceholder {
        alt: String,
        title: Option<String>,
        source: Option<String>,
    },
    Figure {
        children: Vec<SemanticNode>,
        caption: Option<Vec<InlineRun>>,
    },
    Form(FormElement),
    Input(InputElement),
    Select(SelectElement),
    Textarea(TextareaElement),
    Button(ButtonElement),
    Separator,
    Landmark {
        role: LandmarkRole,
        children: Vec<SemanticNode>,
    },
    Details {
        open: bool,
        children: Vec<SemanticNode>,
    },
    Summary {
        runs: Vec<InlineRun>,
        inline_style: Option<String>,
    },
    EmbeddedContent {
        label: String,
    },
    Warning {
        message: String,
    },
}
