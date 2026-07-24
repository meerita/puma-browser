// @file crates/browser-html/src/document.rs
// @description Semantic document type and its sanitized, length-bounded title value object.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::encoding::{DetectedEncoding, Encoding};
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
/// Holds the top-level children of the semantic tree, an optional sanitized title, and
/// the number of `<script>` elements the parser suppressed. The document is the root of
/// the tree; its children own their own descendants. The script count is kept so an
/// adapter can tell the user how many scripts were ignored without ever seeing their
/// content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    children: Vec<SemanticNode>,
    title: Option<DocumentTitle>,
    script_count: usize,
    encoding: DetectedEncoding,
}

impl Document {
    /// Build a document whose bytes were already Unicode.
    ///
    /// The encoding defaults to UTF-8, matching a document assembled directly from
    /// Unicode rather than decoded from bytes. The parser records the real detected
    /// encoding through [`Document::with_encoding`].
    pub fn new(
        children: Vec<SemanticNode>,
        title: Option<DocumentTitle>,
        script_count: usize,
    ) -> Self {
        let utf8 = Encoding::utf8();
        Self {
            children,
            title,
            script_count,
            encoding: DetectedEncoding::new(utf8, utf8),
        }
    }

    /// Record the encoding detection resolved for this document's bytes.
    pub fn with_encoding(mut self, encoding: DetectedEncoding) -> Self {
        self.encoding = encoding;
        self
    }

    pub fn children(&self) -> &[SemanticNode] {
        &self.children
    }

    pub fn title(&self) -> Option<&DocumentTitle> {
        self.title.as_ref()
    }

    /// How many `<script>` elements the parser detected and suppressed.
    pub fn script_count(&self) -> usize {
        self.script_count
    }

    /// The encoding detected for this document and the one used to decode it.
    pub fn encoding(&self) -> DetectedEncoding {
        self.encoding
    }
}
