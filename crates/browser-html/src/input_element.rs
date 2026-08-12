// @file crates/browser-html/src/input_element.rs
// @description Submission data of a parsed <input> element, excluding submit/reset/button kinds.
// @layer html
// @created meerita <meerita@icloud.com>

use crate::input_kind::InputKind;
use crate::node_id::NodeId;

/// A parsed `<input>` element's submission data.
///
/// A sensitive (`type="password"`) input never carries a `value` or `checked` read from
/// its source attributes: both stay at their default (`String::new()`, `false`)
/// regardless of what the markup declares, so a password value can never enter the tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputElement {
    pub id: NodeId,
    pub kind: InputKind,
    pub name: Option<String>,
    pub value: String,
    pub checked: bool,
    pub label: Option<String>,
    pub sensitive: bool,
}
