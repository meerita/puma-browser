// @file crates/browser-html/src/semantic_node.rs
// @description Semantic document node enum consumed by the css, layout, terminal, and mcp layers.
// @layer html
// @created meerita <meerita@icloud.com>

/// A single node in the browser's semantic document tree.
///
/// The tree is the browser's internal representation of a page, produced from parsed
/// HTML and consumed downstream instead of the raw DOM. Each variant carries only the
/// fields its meaning already requires at this stage; `href` and `source` are plain
/// `String` because URL validation belongs to the network layer, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SemanticNode {
    Document,
    Heading {
        level: u8,
        text: String,
    },
    Paragraph {
        text: String,
    },
    Link {
        text: String,
        href: String,
    },
    List,
    ListItem,
    Table,
    TableRow,
    TableCell,
    Quote,
    CodeBlock,
    PreformattedBlock,
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
