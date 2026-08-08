// @file crates/browser-terminal/src/settings_view.rs
// @description Section and row model for the full-screen settings panel, built from live state.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::{CookiePolicy, CookiePolicyPair, SearchEngine};

use crate::TerminalSettings;

/// The cookie policy options a radio row offers, in the order they are shown. The words match
/// the values the config store persists, so a later edit path can map a chosen option straight
/// back to a stored value without a second table.
const POLICY_OPTIONS: [(&str, CookiePolicy); 4] = [
    ("allow", CookiePolicy::Allow),
    ("session", CookiePolicy::Session),
    ("ask", CookiePolicy::Ask),
    ("reject", CookiePolicy::Reject),
];

/// Stable identity of a panel-controlled setting.
///
/// Each variant names one row and maps to one config-store key. The read-only scaffold does
/// not consult it; the instant-apply and text-input phases key their writes off it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingId {
    CookiesFirstParty,
    CookiesThirdParty,
    CopyOnSelect,
    ForceOsc52,
    SearchEnabled,
    UnwrapTracking,
    SearchBaseUrl,
    SearchQueryParameter,
}

/// One option within a radio control: the value word shown and persisted, and whether it is
/// the option currently in effect.
pub(crate) struct RadioOption {
    pub(crate) label: String,
    pub(crate) selected: bool,
}

/// The control a settings row presents, carrying its current value.
pub(crate) enum SettingsControl {
    Checkbox { checked: bool },
    Radio { options: Vec<RadioOption> },
    TextInput { value: String },
}

/// One setting shown in the panel: a stable identity, a human label, its control with the
/// current value, and whether a `PUMA_*` environment variable currently fixes it for the run.
pub(crate) struct SettingsRow {
    // Read by the editing phases, not the read-only scaffold; keeps the row's stable identity
    // available so an applied change knows which setting it changed.
    #[allow(dead_code)]
    pub(crate) id: SettingId,
    pub(crate) label: String,
    pub(crate) control: SettingsControl,
    pub(crate) env_overridden: bool,
}

/// A titled group of rows.
pub(crate) struct SettingsSection {
    pub(crate) title: String,
    pub(crate) rows: Vec<SettingsRow>,
}

/// The whole panel: every section in display order.
pub(crate) struct SettingsModel {
    pub(crate) sections: Vec<SettingsSection>,
}

impl SettingsModel {
    /// The total number of rows across all sections, so focus can wrap over a flat index.
    pub(crate) fn row_count(&self) -> usize {
        self.sections.iter().map(|section| section.rows.len()).sum()
    }
}

/// Builds the panel model from the terminal settings, the controller's global cookie policy,
/// and its configured search engine. Every value is a local, browser-owned string; no remote
/// content reaches the model.
pub(crate) fn build_settings_model(
    settings: &TerminalSettings,
    cookie_policy: CookiePolicyPair,
    search_engine: &SearchEngine,
) -> SettingsModel {
    let cookies = SettingsSection {
        title: "Cookies".to_string(),
        rows: vec![
            radio_row(
                SettingId::CookiesFirstParty,
                "First-party policy",
                cookie_policy.first_party,
            ),
            radio_row(
                SettingId::CookiesThirdParty,
                "Third-party policy",
                cookie_policy.third_party,
            ),
        ],
    };
    let interface = SettingsSection {
        title: "Interface".to_string(),
        rows: vec![
            checkbox_row(
                SettingId::CopyOnSelect,
                "Copy on select",
                settings.copy_on_select,
                settings.env_overridden.copy_on_select,
            ),
            checkbox_row(
                SettingId::ForceOsc52,
                "Force OSC 52 clipboard",
                settings.force_osc52,
                settings.env_overridden.force_osc52,
            ),
        ],
    };
    let search = SettingsSection {
        title: "Search".to_string(),
        rows: vec![
            checkbox_row(
                SettingId::SearchEnabled,
                "Search enabled",
                settings.search_enabled,
                settings.env_overridden.search_enabled,
            ),
            text_row(
                SettingId::SearchBaseUrl,
                "Search base URL",
                search_engine.base_url(),
            ),
            text_row(
                SettingId::SearchQueryParameter,
                "Query parameter",
                search_engine.query_parameter(),
            ),
        ],
    };
    let network = SettingsSection {
        title: "Network".to_string(),
        rows: vec![checkbox_row(
            SettingId::UnwrapTracking,
            "Unwrap tracking redirects",
            settings.unwrap_tracking,
            settings.env_overridden.unwrap_tracking,
        )],
    };
    SettingsModel {
        sections: vec![cookies, interface, search, network],
    }
}

/// A radio row marking the option matching `policy` as selected. Cookie policy has no
/// environment override, so the row is always editable.
fn radio_row(id: SettingId, label: &str, policy: CookiePolicy) -> SettingsRow {
    let options = POLICY_OPTIONS
        .iter()
        .map(|(word, option_policy)| RadioOption {
            label: (*word).to_string(),
            selected: *option_policy == policy,
        })
        .collect();
    SettingsRow {
        id,
        label: label.to_string(),
        control: SettingsControl::Radio { options },
        env_overridden: false,
    }
}

/// A checkbox row seeded with its current value and environment-override flag.
fn checkbox_row(id: SettingId, label: &str, checked: bool, env_overridden: bool) -> SettingsRow {
    SettingsRow {
        id,
        label: label.to_string(),
        control: SettingsControl::Checkbox { checked },
        env_overridden,
    }
}

/// A text-input row seeded with its current value. Text settings have no environment override.
fn text_row(id: SettingId, label: &str, value: &str) -> SettingsRow {
    SettingsRow {
        id,
        label: label.to_string(),
        control: SettingsControl::TextInput {
            value: value.to_string(),
        },
        env_overridden: false,
    }
}

#[cfg(test)]
#[path = "settings_view_tests.rs"]
mod tests;
