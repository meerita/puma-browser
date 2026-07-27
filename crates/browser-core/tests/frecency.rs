// @file crates/browser-core/tests/frecency.rs
// @description Verifies frecency scoring: recency ordering, typed weighting, buckets, clamping.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::frecency;

const SECONDS_PER_DAY: i64 = 86_400;

/// A fixed "now" so tests read as absolute ages rather than relative arithmetic.
const NOW: i64 = 1_000_000_000;

#[test]
fn a_fresh_frequent_page_outranks_an_old_frequent_page() {
    let fresh = frecency(10, 0, NOW - SECONDS_PER_DAY, NOW);
    let stale = frecency(10, 0, NOW - 200 * SECONDS_PER_DAY, NOW);
    assert!(
        fresh > stale,
        "recent visits must rank above old ones at equal frequency"
    );
}

#[test]
fn a_typed_visit_counts_double_a_followed_visit() {
    let followed = frecency(1, 0, NOW, NOW);
    let typed = frecency(1, 1, NOW, NOW);
    // The typed page shares the same recency bucket, so the difference is the *2 weight:
    // followed weight 1, typed weight 1 + 1*2 = 3, both times the recent multiplier 100.
    assert_eq!(followed, 100);
    assert_eq!(typed, 300);
}

#[test]
fn zero_counts_yield_a_zero_score() {
    assert_eq!(frecency(0, 0, NOW, NOW), 0);
}

#[test]
fn a_future_last_visit_clamps_age_to_zero() {
    let future = frecency(1, 0, NOW + 5 * SECONDS_PER_DAY, NOW);
    let present = frecency(1, 0, NOW, NOW);
    assert_eq!(
        future, present,
        "a last-visit in the future must score as if it were now"
    );
}

#[test]
fn the_four_day_bucket_boundary_uses_the_recent_multiplier() {
    assert_eq!(frecency(1, 0, NOW - 4 * SECONDS_PER_DAY, NOW), 100);
}

#[test]
fn just_past_four_days_drops_to_the_near_multiplier() {
    assert_eq!(frecency(1, 0, NOW - 4 * SECONDS_PER_DAY - 1, NOW), 70);
}

#[test]
fn the_fourteen_day_bucket_boundary_uses_the_near_multiplier() {
    assert_eq!(frecency(1, 0, NOW - 14 * SECONDS_PER_DAY, NOW), 70);
}

#[test]
fn the_thirty_one_day_bucket_boundary_uses_the_month_multiplier() {
    assert_eq!(frecency(1, 0, NOW - 31 * SECONDS_PER_DAY, NOW), 50);
}

#[test]
fn the_ninety_day_bucket_boundary_uses_the_quarter_multiplier() {
    assert_eq!(frecency(1, 0, NOW - 90 * SECONDS_PER_DAY, NOW), 30);
}

#[test]
fn beyond_ninety_days_uses_the_stale_multiplier() {
    assert_eq!(frecency(1, 0, NOW - 91 * SECONDS_PER_DAY, NOW), 10);
}
