// @file crates/browser-html/tests/node_id.rs
// @description Behavior tests for the NodeId newtype.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::NodeId;

#[test]
fn node_id_round_trips_its_value() {
    let node_id = NodeId::new(42);
    assert_eq!(node_id.value(), 42);
}

#[test]
fn distinct_values_produce_unequal_ids() {
    assert_ne!(NodeId::new(1), NodeId::new(2));
}
