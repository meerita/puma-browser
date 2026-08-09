// @file crates/browser-terminal/src/settings_view_tests.rs
// @description Unit tests for building the settings-panel model from live settings and state.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::{CookiePolicy, CookiePolicyPair, CookieScope, SearchEngine};

use super::{
    build_settings_model, checkbox_config_key, cookie_scope, CycleDirection, SettingId,
    SettingsControl, SettingsModel, SettingsRow,
};
use crate::{EnvOverrides, TerminalSettings};

/// Terminal settings with fixed toggle values and the given environment overrides, so a test
/// controls only the dimension it exercises.
fn terminal_settings(env_overridden: EnvOverrides) -> TerminalSettings {
    TerminalSettings {
        copy_on_select: true,
        force_osc52: false,
        search_enabled: true,
        unwrap_tracking: false,
        env_overridden,
    }
}

/// Builds a model from the given cookie policy and search engine with all toggles seeded true
/// and no environment overrides, for tests that do not vary those inputs.
fn model_with(cookie_policy: CookiePolicyPair, search_engine: &SearchEngine) -> SettingsModel {
    build_settings_model(
        &terminal_settings(EnvOverrides::default()),
        cookie_policy,
        search_engine,
    )
}

/// The row carrying `id`, panicking when the model has no such row so a test fails loudly
/// rather than silently skipping its assertion.
fn find_row(model: &SettingsModel, id: SettingId) -> &SettingsRow {
    model
        .sections
        .iter()
        .flat_map(|section| &section.rows)
        .find(|row| row.id == id)
        .expect("the model must contain the requested row")
}

/// The flat row index carrying `id`, panicking when the model has no such row.
fn flat_index(model: &SettingsModel, id: SettingId) -> usize {
    model
        .sections
        .iter()
        .flat_map(|section| &section.rows)
        .position(|row| row.id == id)
        .expect("the model must contain the requested row")
}

/// Whether the option labelled `label` is the selected one in the radio row carrying `id`,
/// panicking when the row is not a radio or the label is absent.
fn radio_option_selected(model: &SettingsModel, id: SettingId, label: &str) -> bool {
    match &find_row(model, id).control {
        SettingsControl::Radio { options } => {
            options
                .iter()
                .find(|option| option.label == label)
                .expect("the option must exist")
                .selected
        }
        _ => panic!("the row must be a radio control"),
    }
}

#[test]
fn model_has_one_row_per_config_key() {
    let model = model_with(CookiePolicyPair::default(), &SearchEngine::default());
    assert_eq!(model.row_count(), 8);
}

#[test]
fn cookie_rows_are_radio_controls() {
    let model = model_with(CookiePolicyPair::default(), &SearchEngine::default());
    assert!(matches!(
        find_row(&model, SettingId::CookiesFirstParty).control,
        SettingsControl::Radio { .. }
    ));
    assert!(matches!(
        find_row(&model, SettingId::CookiesThirdParty).control,
        SettingsControl::Radio { .. }
    ));
}

#[test]
fn toggle_rows_are_checkbox_controls() {
    let model = model_with(CookiePolicyPair::default(), &SearchEngine::default());
    for id in [
        SettingId::CopyOnSelect,
        SettingId::ForceOsc52,
        SettingId::SearchEnabled,
        SettingId::UnwrapTracking,
    ] {
        assert!(matches!(
            find_row(&model, id).control,
            SettingsControl::Checkbox { .. }
        ));
    }
}

#[test]
fn a_toggle_seeded_true_renders_checked() {
    let model = model_with(CookiePolicyPair::default(), &SearchEngine::default());
    match &find_row(&model, SettingId::CopyOnSelect).control {
        SettingsControl::Checkbox { checked } => assert!(*checked),
        _ => panic!("copy-on-select must be a checkbox"),
    }
}

#[test]
fn cookie_policy_reject_marks_the_reject_option_selected() {
    let policy = CookiePolicyPair {
        first_party: CookiePolicy::Reject,
        third_party: CookiePolicy::Allow,
    };
    let model = model_with(policy, &SearchEngine::default());
    match &find_row(&model, SettingId::CookiesFirstParty).control {
        SettingsControl::Radio { options } => {
            let reject = options
                .iter()
                .find(|option| option.label == "reject")
                .expect("a reject option must exist");
            assert!(reject.selected);
            let allow = options
                .iter()
                .find(|option| option.label == "allow")
                .expect("an allow option must exist");
            assert!(!allow.selected);
        }
        _ => panic!("the first-party cookie row must be a radio control"),
    }
}

#[test]
fn an_env_overridden_toggle_is_flagged() {
    let env_overridden = EnvOverrides {
        force_osc52: true,
        ..EnvOverrides::default()
    };
    let model = build_settings_model(
        &terminal_settings(env_overridden),
        CookiePolicyPair::default(),
        &SearchEngine::default(),
    );
    assert!(find_row(&model, SettingId::ForceOsc52).env_overridden);
    assert!(!find_row(&model, SettingId::CopyOnSelect).env_overridden);
}

#[test]
fn search_rows_carry_the_engine_values() {
    let engine = SearchEngine::new("https://example.com/".to_string(), "query".to_string())
        .expect("a valid search engine must build");
    let model = model_with(CookiePolicyPair::default(), &engine);
    match &find_row(&model, SettingId::SearchBaseUrl).control {
        SettingsControl::TextInput { value } => assert_eq!(value, "https://example.com/"),
        _ => panic!("the search base URL must be a text input"),
    }
    match &find_row(&model, SettingId::SearchQueryParameter).control {
        SettingsControl::TextInput { value } => assert_eq!(value, "query"),
        _ => panic!("the query parameter must be a text input"),
    }
}

#[test]
fn toggling_a_checkbox_flips_it_and_reports_the_new_state() {
    let mut model = model_with(CookiePolicyPair::default(), &SearchEngine::default());
    let index = flat_index(&model, SettingId::CopyOnSelect);
    // The row is seeded true, so a toggle turns it off.
    assert_eq!(
        model.toggle_checkbox(index),
        Some((SettingId::CopyOnSelect, false))
    );
    match &find_row(&model, SettingId::CopyOnSelect).control {
        SettingsControl::Checkbox { checked } => assert!(!checked),
        _ => panic!("copy-on-select must be a checkbox"),
    }
}

#[test]
fn toggling_a_non_checkbox_row_returns_none() {
    let mut model = model_with(CookiePolicyPair::default(), &SearchEngine::default());
    let index = flat_index(&model, SettingId::CookiesFirstParty);
    assert_eq!(model.toggle_checkbox(index), None);
}

#[test]
fn cycling_a_radio_forward_moves_to_the_next_option() {
    let policy = CookiePolicyPair {
        first_party: CookiePolicy::Allow,
        third_party: CookiePolicy::Reject,
    };
    let mut model = model_with(policy, &SearchEngine::default());
    let index = flat_index(&model, SettingId::CookiesFirstParty);
    assert_eq!(
        model.cycle_radio(index, CycleDirection::Next),
        Some((SettingId::CookiesFirstParty, CookiePolicy::Session))
    );
    assert!(radio_option_selected(
        &model,
        SettingId::CookiesFirstParty,
        "session"
    ));
    assert!(!radio_option_selected(
        &model,
        SettingId::CookiesFirstParty,
        "allow"
    ));
}

#[test]
fn cycling_a_radio_backward_wraps_to_the_last_option() {
    let policy = CookiePolicyPair {
        first_party: CookiePolicy::Allow,
        third_party: CookiePolicy::Reject,
    };
    let mut model = model_with(policy, &SearchEngine::default());
    let index = flat_index(&model, SettingId::CookiesFirstParty);
    assert_eq!(
        model.cycle_radio(index, CycleDirection::Prev),
        Some((SettingId::CookiesFirstParty, CookiePolicy::Reject))
    );
    assert!(radio_option_selected(
        &model,
        SettingId::CookiesFirstParty,
        "reject"
    ));
}

#[test]
fn cycling_a_non_radio_row_returns_none() {
    let mut model = model_with(CookiePolicyPair::default(), &SearchEngine::default());
    let index = flat_index(&model, SettingId::CopyOnSelect);
    assert_eq!(model.cycle_radio(index, CycleDirection::Next), None);
}

#[test]
fn checkbox_config_key_maps_each_toggle_and_rejects_radios_and_text() {
    assert_eq!(
        checkbox_config_key(SettingId::CopyOnSelect),
        Some("ui.copy_on_select")
    );
    assert_eq!(
        checkbox_config_key(SettingId::ForceOsc52),
        Some("ui.force_osc52")
    );
    assert_eq!(
        checkbox_config_key(SettingId::SearchEnabled),
        Some("search.enabled")
    );
    assert_eq!(
        checkbox_config_key(SettingId::UnwrapTracking),
        Some("network.unwrap_tracking")
    );
    assert_eq!(checkbox_config_key(SettingId::CookiesFirstParty), None);
    assert_eq!(checkbox_config_key(SettingId::SearchBaseUrl), None);
}

#[test]
fn cookie_scope_maps_radios_and_rejects_other_rows() {
    assert_eq!(
        cookie_scope(SettingId::CookiesFirstParty),
        Some(CookieScope::FirstParty)
    );
    assert_eq!(
        cookie_scope(SettingId::CookiesThirdParty),
        Some(CookieScope::ThirdParty)
    );
    assert_eq!(cookie_scope(SettingId::CopyOnSelect), None);
    assert_eq!(cookie_scope(SettingId::SearchBaseUrl), None);
}
