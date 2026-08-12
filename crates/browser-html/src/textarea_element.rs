// @file crates/browser-html/src/textarea_element.rs
// @description Submission data of a parsed <textarea> element.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::node_id::NodeId;

/// A parsed `<textarea>` element's submission data.
///
/// `value` is the element's text content, not an attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextareaElement {
    pub id: NodeId,
    pub name: Option<String>,
    pub value: String,
    pub label: Option<String>,
}
