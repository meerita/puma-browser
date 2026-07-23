// @file crates/browser-html/src/document.rs
// @description Semantic document type and its sanitized, length-bounded title value object.
// @layer html
// @created meerita <meerita@icloud.com>

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
        let sanitized: String = raw
            .chars()
            .filter(|character| !character.is_control())
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
/// Holds the page's semantic nodes and an optional sanitized title. v0.1 defines the
/// shape only; population happens once HTML parsing lands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    nodes: Vec<SemanticNode>,
    title: Option<DocumentTitle>,
}

impl Document {
    pub fn new(nodes: Vec<SemanticNode>, title: Option<DocumentTitle>) -> Self {
        Self { nodes, title }
    }

    pub fn nodes(&self) -> &[SemanticNode] {
        &self.nodes
    }

    pub fn title(&self) -> Option<&DocumentTitle> {
        self.title.as_ref()
    }
}
