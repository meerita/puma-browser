// @file crates/browser-html/src/form_element.rs
// @description Submission data and content of a parsed <form> element.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::form_method::FormMethod;
use crate::node_id::NodeId;
use crate::semantic_node::SemanticNode;

/// A parsed `<form>` element: its stable identity, submission target, and content.
///
/// `action` is always a resolved URL, never empty: a form with no `action` attribute
/// resolves to the document's own URL, matching how a browser submits a form with no
/// declared action.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormElement {
    pub id: NodeId,
    pub action: String,
    pub method: FormMethod,
    pub children: Vec<SemanticNode>,
}
