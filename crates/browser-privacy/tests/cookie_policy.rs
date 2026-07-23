// @file crates/browser-privacy/tests/cookie_policy.rs
// @description Verifies CookiePolicy defaults to Reject and has four distinct variants.
// @layer privacy
// @created meerita <meerita@icloud.com>

use browser_privacy::CookiePolicy;

#[test]
fn default_cookie_policy_is_reject() {
    assert_eq!(CookiePolicy::default(), CookiePolicy::Reject);
}

#[test]
fn cookie_policy_has_four_distinct_variants() {
    let variants = [
        CookiePolicy::Allow,
        CookiePolicy::Session,
        CookiePolicy::Ask,
        CookiePolicy::Reject,
    ];
    for (first_index, first) in variants.iter().enumerate() {
        for (second_index, second) in variants.iter().enumerate() {
            let is_same_position = first_index == second_index;
            assert_eq!(
                first == second,
                is_same_position,
                "variants at {first_index} and {second_index} compare incorrectly"
            );
        }
    }
}
