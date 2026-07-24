// @file crates/browser-html/src/semantic_node.rs
// @description Semantic document node enum consumed by the css, layout, terminal, and mcp layers.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::inline_run::InlineRun;

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
    Table,
    TableRow,
    TableCell,
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
    Form,
    Input,
    Select,
    Button,
    Separator,
    Landmark,
    Details,
    Summary,
    EmbeddedContent,
    Warning {
        message: String,
    },
}
