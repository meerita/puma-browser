// @file crates/browser-core/src/form_field_values.rs
// @description Per-page live form field state, keyed by the field's stable NodeId.
// @layer core
// @created meerita <meerita@icloud.com>

use std::collections::HashMap;

use browser_html::NodeId;

use crate::field_value::FieldValue;

/// The live state of one form control, as the user has edited it.
///
/// Seeded from the control's parsed default when a page loads, then mutated as the
/// user edits it. A `Select`'s current choice is a one-element `Selected` vec for a
/// single-select; an empty vec means nothing selected.
#[derive(Debug)]
pub enum FieldState {
    Text(String),
    Sensitive(FieldValue),
    Checked(bool),
    Selected(Vec<String>),
}

/// Live form field state for the single page currently loaded.
///
/// Scoped to one page, matching [`crate::current_page::CurrentPage`]'s existing
/// single-page lifetime: a fresh, empty map is seeded on every navigation.
#[derive(Debug, Default)]
pub struct FormFieldValues {
    fields: HashMap<NodeId, FieldState>,
}

impl FormFieldValues {
    /// An empty map, seeded field by field as a page's controls are walked.
    pub fn new() -> Self {
        Self::default()
    }

    /// The current value of a `Text` state; `None` for any other state or an unknown id.
    pub fn text(&self, id: NodeId) -> Option<&str> {
        match self.fields.get(&id) {
            Some(FieldState::Text(value)) => Some(value.as_str()),
            _ => None,
        }
    }

    /// Whether the field is checked; `false` for any state other than `Checked(true)` or
    /// an unknown id.
    pub fn is_checked(&self, id: NodeId) -> bool {
        matches!(self.fields.get(&id), Some(FieldState::Checked(true)))
    }

    /// The current set of selected option values; an empty slice for any other state or
    /// an unknown id.
    pub fn selected_values(&self, id: NodeId) -> &[String] {
        match self.fields.get(&id) {
            Some(FieldState::Selected(values)) => values,
            _ => &[],
        }
    }

    /// The current value of a `Sensitive` state; `None` for any other state or an
    /// unknown id.
    pub fn sensitive_value(&self, id: NodeId) -> Option<&FieldValue> {
        match self.fields.get(&id) {
            Some(FieldState::Sensitive(value)) => Some(value),
            _ => None,
        }
    }

    /// Sets a non-sensitive text field's value.
    pub fn set_text(&mut self, id: NodeId, value: String) {
        self.fields.insert(id, FieldState::Text(value));
    }

    /// Sets a sensitive field's value, wrapping it into a [`FieldValue`] so the caller
    /// never constructs one directly.
    pub fn set_sensitive_text(&mut self, id: NodeId, value: String) {
        self.fields
            .insert(id, FieldState::Sensitive(FieldValue::new(value)));
    }

    /// Sets a checkbox or radio's checked state.
    pub fn set_checked(&mut self, id: NodeId, checked: bool) {
        self.fields.insert(id, FieldState::Checked(checked));
    }

    /// Replaces a select's current selection.
    pub fn set_selected(&mut self, id: NodeId, values: Vec<String>) {
        self.fields.insert(id, FieldState::Selected(values));
    }
}
