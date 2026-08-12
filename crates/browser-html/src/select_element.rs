// @file crates/browser-html/src/select_element.rs
// @description Submission data of a parsed <select> element and its options.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::node_id::NodeId;

/// A parsed `<select>` element's submission data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectElement {
    pub id: NodeId,
    pub name: Option<String>,
    pub label: Option<String>,
    pub multiple: bool,
    pub options: Vec<SelectOption>,
}

/// A single `<option>` within a [`SelectElement`].
///
/// `value` defaults to the option's own text when the source carries no `value`
/// attribute, matching HTML.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
}
