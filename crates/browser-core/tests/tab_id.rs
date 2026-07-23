// @file crates/browser-core/tests/tab_id.rs
// @description Verifies TabId wraps and returns its underlying value.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::TabId;

#[test]
fn tab_id_round_trips_its_value() {
    let tab = TabId::new(42);
    assert_eq!(tab.value(), 42);
}
