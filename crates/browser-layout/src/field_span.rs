// @file crates/browser-layout/src/field_span.rs
// @description Interactive form-control span geometry mirroring link spans in the laid-out cell buffer.
// @layer layout
// @created meerita <meerita@icloud.com>

/// Which control kind a [`FieldSpan`] marks.
///
/// No `Hidden` kind exists: a hidden input never renders a row, so it never has a span
/// to mark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSpanKind {
    Input,
    Select,
    Textarea,
    Button,
}

/// The terminal-row extent of a single interactive form control in the laid-out buffer.
///
/// A control that wraps across multiple lines produces one `FieldSpan` per row,
/// mirroring [`crate::LinkSpan`]. `col_end` is the inclusive last column of the span on
/// that row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldSpan {
    pub node_id: browser_html::NodeId,
    pub kind: FieldSpanKind,
    pub row: u16,
    pub col_start: u16,
    pub col_end: u16,
}
