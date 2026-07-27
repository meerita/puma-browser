// @file crates/browser-core/tests/suggestion_index.rs
// @description Tests host-aware prefix matching and frecency ranking of the suggestion index.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{SuggestionEntry, SuggestionIndex};

/// A fixed query clock so ranking is deterministic across runs.
const NOW: i64 = 1_000_000_000;

/// Seconds in one day, for building ages relative to `NOW`.
const SECONDS_PER_DAY: i64 = 86_400;

fn entry(
    url: &str,
    host: &str,
    visit_count: u32,
    typed_count: u32,
    last_visit_at: i64,
) -> SuggestionEntry {
    SuggestionEntry::new(
        url.to_string(),
        host.to_string(),
        visit_count,
        typed_count,
        last_visit_at,
    )
}

#[test]
fn typing_git_surfaces_the_github_host() {
    let index = SuggestionIndex::from_entries(vec![
        entry(
            "https://github.com/",
            "github.com",
            5,
            2,
            NOW - SECONDS_PER_DAY,
        ),
        entry(
            "https://example.com/",
            "example.com",
            5,
            2,
            NOW - SECONDS_PER_DAY,
        ),
    ]);

    let suggestions = index.suggest("git", NOW, 8);

    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].host(), "github.com");
}

#[test]
fn typing_news_yc_surfaces_the_ycombinator_host() {
    let index = SuggestionIndex::from_entries(vec![
        entry(
            "https://news.ycombinator.com/",
            "news.ycombinator.com",
            9,
            3,
            NOW - SECONDS_PER_DAY,
        ),
        entry(
            "https://example.com/",
            "example.com",
            9,
            3,
            NOW - SECONDS_PER_DAY,
        ),
    ]);

    let suggestions = index.suggest("news.yc", NOW, 8);

    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].host(), "news.ycombinator.com");
}

#[test]
fn a_fresh_host_outranks_an_equally_frequent_stale_host() {
    let index = SuggestionIndex::from_entries(vec![
        entry(
            "https://example-old.com/",
            "example-old.com",
            10,
            0,
            NOW - 200 * SECONDS_PER_DAY,
        ),
        entry(
            "https://example-new.com/",
            "example-new.com",
            10,
            0,
            NOW - SECONDS_PER_DAY,
        ),
    ]);

    let suggestions = index.suggest("example", NOW, 8);

    assert_eq!(suggestions.len(), 2);
    assert_eq!(suggestions[0].host(), "example-new.com");
    assert_eq!(suggestions[1].host(), "example-old.com");
}

#[test]
fn a_typed_entry_outranks_a_followed_link_entry_with_equal_counts() {
    let last_visit_at = NOW - SECONDS_PER_DAY;
    let index = SuggestionIndex::from_entries(vec![
        entry(
            "https://example.com/link",
            "example.com",
            3,
            0,
            last_visit_at,
        ),
        entry(
            "https://example.com/typed",
            "example.com",
            3,
            3,
            last_visit_at,
        ),
    ]);

    let suggestions = index.suggest("example.com", NOW, 8);

    assert_eq!(suggestions.len(), 2);
    assert_eq!(suggestions[0].url(), "https://example.com/typed");
    assert_eq!(suggestions[1].url(), "https://example.com/link");
}

#[test]
fn the_result_is_capped_at_the_requested_limit() {
    let index = SuggestionIndex::from_entries(vec![
        entry(
            "https://example1.com/",
            "example1.com",
            5,
            0,
            NOW - SECONDS_PER_DAY,
        ),
        entry(
            "https://example2.com/",
            "example2.com",
            5,
            0,
            NOW - SECONDS_PER_DAY,
        ),
        entry(
            "https://example3.com/",
            "example3.com",
            5,
            0,
            NOW - SECONDS_PER_DAY,
        ),
        entry(
            "https://example4.com/",
            "example4.com",
            5,
            0,
            NOW - SECONDS_PER_DAY,
        ),
    ]);

    let suggestions = index.suggest("example", NOW, 2);

    assert_eq!(suggestions.len(), 2);
}

#[test]
fn a_prefix_matching_nothing_yields_no_suggestions() {
    let index = SuggestionIndex::from_entries(vec![entry(
        "https://github.com/",
        "github.com",
        5,
        2,
        NOW - SECONDS_PER_DAY,
    )]);

    let suggestions = index.suggest("nonexistent", NOW, 8);

    assert!(suggestions.is_empty());
}

#[test]
fn an_empty_input_yields_no_suggestions() {
    let index = SuggestionIndex::from_entries(vec![entry(
        "https://github.com/",
        "github.com",
        5,
        2,
        NOW - SECONDS_PER_DAY,
    )]);

    let suggestions = index.suggest("", NOW, 8);

    assert!(suggestions.is_empty());
}

#[test]
fn upsert_replaces_an_entry_for_the_same_url_rather_than_duplicating_it() {
    let mut index = SuggestionIndex::from_entries(vec![entry(
        "https://github.com/",
        "github.com",
        1,
        0,
        NOW - 10 * SECONDS_PER_DAY,
    )]);

    index.upsert(entry(
        "https://github.com/",
        "github.com",
        4,
        2,
        NOW - SECONDS_PER_DAY,
    ));

    let suggestions = index.suggest("github", NOW, 8);
    assert_eq!(suggestions.len(), 1);
    assert_eq!(suggestions[0].visit_count(), 4);
    assert_eq!(suggestions[0].typed_count(), 2);
}
