// @file crates/browser-core/tests/tab_state.rs
// @description Verifies the TabState variants are distinct from one another.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::TabState;

#[test]
fn tab_state_variants_are_distinct() {
    let variants = [
        TabState::Loading,
        TabState::Loaded,
        TabState::Error,
        TabState::Blank,
    ];
    for (first_index, first) in variants.iter().enumerate() {
        for (second_index, second) in variants.iter().enumerate() {
            if first_index == second_index {
                assert_eq!(first, second, "a variant must equal itself");
                continue;
            }
            assert_ne!(first, second, "distinct variants must not compare equal");
        }
    }
}
