// @file crates/browser-core/tests/history_mode.rs
// @description Verifies history-mode string mapping and HistorySettings accessors.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::{history_mode_from_str, HistoryMode, HistorySettings};

#[test]
fn disabled_string_maps_to_disabled() {
    assert_eq!(history_mode_from_str("disabled"), HistoryMode::Disabled);
}

#[test]
fn in_memory_string_maps_to_in_memory() {
    assert_eq!(history_mode_from_str("in-memory"), HistoryMode::InMemory);
}

#[test]
fn persistent_string_maps_to_persistent() {
    assert_eq!(history_mode_from_str("persistent"), HistoryMode::Persistent);
}

#[test]
fn mapping_is_case_insensitive() {
    assert_eq!(history_mode_from_str("DISABLED"), HistoryMode::Disabled);
    assert_eq!(history_mode_from_str("In-Memory"), HistoryMode::InMemory);
    assert_eq!(history_mode_from_str("Persistent"), HistoryMode::Persistent);
}

#[test]
fn surrounding_whitespace_is_ignored() {
    assert_eq!(history_mode_from_str("  disabled  "), HistoryMode::Disabled);
}

#[test]
fn an_unknown_string_maps_to_persistent() {
    assert_eq!(history_mode_from_str("nonsense"), HistoryMode::Persistent);
    assert_eq!(history_mode_from_str(""), HistoryMode::Persistent);
}

#[test]
fn history_settings_accessors_return_the_constructed_values() {
    let settings = HistorySettings::new(HistoryMode::InMemory, 45, false);
    assert_eq!(settings.mode(), HistoryMode::InMemory);
    assert_eq!(settings.retention_days(), 45);
    assert!(!settings.store_titles());
}
