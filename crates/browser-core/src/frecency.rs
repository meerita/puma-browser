// @file crates/browser-core/src/frecency.rs
// @description Pure frecency scoring for ranking visited pages in the suggestion index.
// @layer core
// @created meerita <meerita@icloud.com>

/// How much more a typed visit counts than a followed-link visit. Typing a URL is a
/// stronger signal of intent to return than arriving by clicking a link.
const TYPED_WEIGHT_MULTIPLIER: i64 = 2;

/// Seconds in one day, used to convert the day thresholds below into an age in seconds.
const SECONDS_PER_DAY: i64 = 86_400;

/// Upper age bound, in days, of each recency bucket. A visit newer than the bound gets
/// the matching multiplier; anything past the last bound is treated as stale.
const RECENT_DAYS: i64 = 4;
const NEAR_DAYS: i64 = 14;
const MONTH_DAYS: i64 = 31;
const QUARTER_DAYS: i64 = 90;

/// Recency multiplier for each bucket, decreasing as a visit ages.
const RECENT_MULTIPLIER: i64 = 100;
const NEAR_MULTIPLIER: i64 = 70;
const MONTH_MULTIPLIER: i64 = 50;
const QUARTER_MULTIPLIER: i64 = 30;
const STALE_MULTIPLIER: i64 = 10;

/// Scores a page for autocomplete ranking from its raw counters and last-visit time.
///
/// Higher is better. The score combines how often the page was visited (typed visits
/// weighted more) with how recently, using the current time `now` so recency reflects the
/// moment of the query rather than the moment of the last write. All times are Unix epoch
/// seconds. The function is pure: it reads no clock and performs no I/O.
pub fn frecency(visit_count: u32, typed_count: u32, last_visit_at: i64, now: i64) -> i64 {
    let weight = i64::from(visit_count) + i64::from(typed_count) * TYPED_WEIGHT_MULTIPLIER;
    let age_seconds = (now - last_visit_at).max(0);
    weight * recency_multiplier(age_seconds)
}

/// Maps an age in seconds to its recency-bucket multiplier.
fn recency_multiplier(age_seconds: i64) -> i64 {
    if age_seconds <= RECENT_DAYS * SECONDS_PER_DAY {
        return RECENT_MULTIPLIER;
    }
    if age_seconds <= NEAR_DAYS * SECONDS_PER_DAY {
        return NEAR_MULTIPLIER;
    }
    if age_seconds <= MONTH_DAYS * SECONDS_PER_DAY {
        return MONTH_MULTIPLIER;
    }
    if age_seconds <= QUARTER_DAYS * SECONDS_PER_DAY {
        return QUARTER_MULTIPLIER;
    }
    STALE_MULTIPLIER
}
