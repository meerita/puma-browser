// @file crates/browser-terminal/src/command_tests.rs
// @description Unit tests for the slash-command registry, resolver, and ranked matcher.
// @layer terminal
// @created meerita <meerita@icloud.com>

use super::{filter, registry, resolve, CommandKind, MatchRank};

#[test]
fn registry_holds_exactly_the_six_commands() {
    let names: Vec<&str> = registry().iter().map(|spec| spec.name).collect();
    assert_eq!(
        names,
        vec!["open", "reload", "back", "help", "quit", "settings"]
    );
}

#[test]
fn settings_carries_the_config_alias_and_coming_soon_description() {
    let settings = resolve("settings").expect("settings command must exist");
    assert_eq!(settings.aliases, &["config"]);
    assert!(settings.description.contains("coming soon"));
    assert_eq!(settings.kind, CommandKind::Settings);
}

#[test]
fn open_declares_a_single_url_argument() {
    let open = resolve("open").expect("open command must exist");
    assert_eq!(open.args.len(), 1);
    assert_eq!(open.args[0].name, "url");
}

#[test]
fn commands_without_arguments_declare_no_arguments() {
    for name in ["reload", "back", "help", "quit", "settings"] {
        let spec = resolve(name).expect("command must exist");
        assert!(spec.args.is_empty(), "{name} should take no arguments");
    }
}

#[test]
fn resolve_matches_a_canonical_name() {
    let reload = resolve("reload").expect("reload must resolve");
    assert_eq!(reload.kind, CommandKind::Reload);
}

#[test]
fn resolve_matches_an_alias() {
    let config = resolve("config").expect("config alias must resolve");
    assert_eq!(config.kind, CommandKind::Settings);
    assert_eq!(config.name, "settings");
}

#[test]
fn resolve_returns_none_for_an_unknown_token() {
    assert!(resolve("nope").is_none());
}

#[test]
fn empty_query_returns_all_commands_in_registry_order() {
    let matches = filter("");
    let names: Vec<&str> = matches.iter().map(|found| found.spec.name).collect();
    assert_eq!(
        names,
        vec!["open", "reload", "back", "help", "quit", "settings"]
    );
    assert!(matches.iter().all(|found| found.rank == MatchRank::Prefix));
}

#[test]
fn prefix_query_matches_the_command_that_starts_with_it() {
    let matches = filter("re");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].spec.name, "reload");
    assert_eq!(matches[0].rank, MatchRank::Prefix);
}

#[test]
fn subsequence_query_matches_non_adjacent_characters() {
    let matches = filter("st");
    let settings = matches
        .iter()
        .find(|found| found.spec.name == "settings")
        .expect("settings must match the subsequence s..t");
    assert_eq!(settings.rank, MatchRank::Subsequence);
}

#[test]
fn prefix_matches_rank_above_subsequence_matches() {
    // "e" prefixes nothing but is a subsequence of several names; "o" prefixes "open".
    let matches = filter("o");
    let prefix_first = matches
        .iter()
        .position(|found| found.rank == MatchRank::Prefix);
    let first_subsequence = matches
        .iter()
        .position(|found| found.rank == MatchRank::Subsequence);
    assert_eq!(prefix_first, Some(0));
    if let Some(subsequence_index) = first_subsequence {
        assert!(
            subsequence_index > 0,
            "subsequence must follow prefix matches"
        );
    }
}

#[test]
fn a_command_matching_by_both_name_and_alias_appears_once() {
    // "co" prefixes the alias "config"; settings must appear exactly once.
    let matches = filter("co");
    let settings_count = matches
        .iter()
        .filter(|found| found.spec.name == "settings")
        .count();
    assert_eq!(settings_count, 1);
}

#[test]
fn a_query_matching_nothing_returns_empty() {
    assert!(filter("zzz").is_empty());
}
