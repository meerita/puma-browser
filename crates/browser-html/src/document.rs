// @file crates/browser-html/src/document.rs
// @description Semantic document type and its sanitized, length-bounded title value object.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::sanitize::strip_control_characters;
use crate::semantic_node::SemanticNode;

/// Maximum number of characters retained in a [`DocumentTitle`].
///
/// A title comes from remote page content, so its length is bounded to keep an
/// oversized `<title>` from consuming unbounded memory or dominating the terminal.
const MAX_DOCUMENT_TITLE_LEN: usize = 256;

/// A page title derived from remote content, sanitized for safe display.
///
/// The constructor removes control characters so a title can never carry escape
/// sequences into later rendering, and bounds the length. The inner string is private;
/// callers read it through [`DocumentTitle::as_str`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentTitle(String);

impl DocumentTitle {
    /// Build a title from raw remote text.
    ///
    /// Control characters are stripped first so no escape sequence survives, then the
    /// result is truncated to at most `MAX_DOCUMENT_TITLE_LEN` characters.
    pub fn new(raw: &str) -> DocumentTitle {
        let sanitized: String = strip_control_characters(raw)
            .chars()
            .take(MAX_DOCUMENT_TITLE_LEN)
            .collect();
        DocumentTitle(sanitized)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The browser's semantic representation of a single page.
///
/// Holds the page's semantic nodes, an optional sanitized title, and the number of
/// `<script>` elements the parser suppressed. The script count is kept so an adapter
/// can tell the user how many scripts were ignored without ever seeing their content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    nodes: Vec<SemanticNode>,
    title: Option<DocumentTitle>,
    script_count: usize,
}

impl Document {
    pub fn new(
        nodes: Vec<SemanticNode>,
        title: Option<DocumentTitle>,
        script_count: usize,
    ) -> Self {
        Self {
            nodes,
            title,
            script_count,
        }
    }

    pub fn nodes(&self) -> &[SemanticNode] {
        &self.nodes
    }

    pub fn title(&self) -> Option<&DocumentTitle> {
        self.title.as_ref()
    }

    /// How many `<script>` elements the parser detected and suppressed.
    pub fn script_count(&self) -> usize {
        self.script_count
    }
}
