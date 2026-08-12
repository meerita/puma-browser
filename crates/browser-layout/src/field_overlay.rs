// @file crates/browser-layout/src/field_overlay.rs
// @description Live form-field values applied over static placeholders at render time.
// @layer layout
// @created meerita <meerita@icloud.com>

use std::collections::HashMap;

use browser_html::NodeId;

/// The live value to substitute for a control's static placeholder.
///
/// `MaskedLength` carries a character count, never the revealed value, so a sensitive
/// field's contents never cross into this crate or into `browser-terminal`: the caller
/// that builds the overlay reveals a sensitive value only long enough to count it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldRenderValue {
    Text(String),
    MaskedLength(usize),
    Checked(bool),
    SelectedLabels(Vec<String>),
}

/// Live form-field values to overlay onto a document's rendered placeholders, keyed by
/// each control's stable [`NodeId`].
///
/// `browser-layout` cannot depend on `browser-core`, where the live values themselves
/// live, so `browser-core` translates its own per-page state into this crate's own
/// overlay type at the `render_document` call site instead.
#[derive(Debug, Clone, Default)]
pub struct FieldOverlay {
    values: HashMap<NodeId, FieldRenderValue>,
}

impl FieldOverlay {
    /// An empty overlay, populated one control at a time by the caller.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records the live value to show in place of `node_id`'s static placeholder.
    pub fn insert(&mut self, node_id: NodeId, value: FieldRenderValue) {
        self.values.insert(node_id, value);
    }

    /// The live value recorded for `node_id`, or `None` when the overlay carries no
    /// entry for it, so the caller falls back to the static placeholder.
    pub fn get(&self, node_id: NodeId) -> Option<&FieldRenderValue> {
        self.values.get(&node_id)
    }
}

#[cfg(test)]
#[path = "field_overlay_tests.rs"]
mod tests;
