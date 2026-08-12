// @file crates/browser-layout/src/field_overlay_tests.rs
// @description Unit tests for FieldOverlay's insert/get lookup.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_html::NodeId;

use super::{FieldOverlay, FieldRenderValue};

#[test]
fn unknown_node_id_returns_none() {
    let overlay = FieldOverlay::new();
    assert_eq!(overlay.get(NodeId::new(1)), None);
}

#[test]
fn inserted_value_is_returned_for_its_node_id() {
    let mut overlay = FieldOverlay::new();
    overlay.insert(NodeId::new(3), FieldRenderValue::Text("hello".to_string()));
    assert_eq!(
        overlay.get(NodeId::new(3)),
        Some(&FieldRenderValue::Text("hello".to_string()))
    );
    assert_eq!(overlay.get(NodeId::new(4)), None);
}
