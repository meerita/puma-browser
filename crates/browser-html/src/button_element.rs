// @file crates/browser-html/src/button_element.rs
// @description Submission data of a parsed <button> element or a normalized submit/reset/button input.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::button_kind::ButtonKind;
use crate::inline_run::InlineRun;
use crate::node_id::NodeId;

/// A parsed `<button>` element, or an `<input type=submit|reset|button>` normalized into
/// the same shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonElement {
    pub id: NodeId,
    pub kind: ButtonKind,
    pub name: Option<String>,
    pub value: Option<String>,
    pub runs: Vec<InlineRun>,
    pub inline_style: Option<String>,
}
