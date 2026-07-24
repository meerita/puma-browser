// @file crates/browser-html/src/semantic_node.rs
// @description Semantic document node enum consumed by the css, layout, terminal, and mcp layers.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::inline_run::InlineRun;
use crate::input_kind::InputKind;
use crate::landmark_role::LandmarkRole;

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
/// [`Document`]: crate::Document
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticNode {
    Heading {
        level: u8,
        runs: Vec<InlineRun>,
    },
    Paragraph {
        runs: Vec<InlineRun>,
    },
    List {
        ordered: bool,
        children: Vec<SemanticNode>,
    },
    ListItem {
        children: Vec<SemanticNode>,
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
    },
    Quote {
        children: Vec<SemanticNode>,
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
    Form {
        children: Vec<SemanticNode>,
    },
    Input {
        kind: InputKind,
        label: Option<String>,
        sensitive: bool,
    },
    Select {
        label: Option<String>,
        options: Vec<String>,
    },
    Button {
        runs: Vec<InlineRun>,
    },
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
    },
    EmbeddedContent {
        label: String,
    },
    Warning {
        message: String,
    },
}
