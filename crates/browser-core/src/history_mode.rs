// @file crates/browser-core/src/history_mode.rs
// @description History recording mode and resolved history settings for the navigation core.
// @layer core
// @created meerita <meerita@icloud.com>

/// How the browser records navigation history.
///
/// This is a domain enum and is deliberately not `serde`-derived: the storage adapter
/// owns the on-disk string representation and maps to and from this enum, so a
/// configuration-format change never forces a change here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HistoryMode {
    /// Records nothing and offers no suggestions. The default so a controller built
    /// without an injected store never records.
    #[default]
    Disabled,
    /// Keeps suggestions for the running session but never writes to disk.
    InMemory,
    /// Records visits durably and offers suggestions across sessions.
    Persistent,
}

/// Maps a configuration string to a [`HistoryMode`], case-insensitively.
///
/// An unrecognized value resolves to [`HistoryMode::Persistent`], the safe default, so a
/// typo in the configuration never silently disables history recording.
pub fn history_mode_from_str(value: &str) -> HistoryMode {
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" => HistoryMode::Disabled,
        "in-memory" => HistoryMode::InMemory,
        _ => HistoryMode::Persistent,
    }
}

/// The resolved history configuration the navigation core acts on.
///
/// It carries the mode plus the retention window and the title-storage toggle, resolved
/// from defaults, the configuration file, and environment overrides before construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HistorySettings {
    mode: HistoryMode,
    retention_days: u32,
    store_titles: bool,
}

impl HistorySettings {
    pub fn new(mode: HistoryMode, retention_days: u32, store_titles: bool) -> Self {
        Self {
            mode,
            retention_days,
            store_titles,
        }
    }

    /// The history recording mode.
    pub fn mode(&self) -> HistoryMode {
        self.mode
    }

    /// The retention window in days before old history is pruned.
    pub fn retention_days(&self) -> u32 {
        self.retention_days
    }

    /// Whether page titles are stored alongside visited URLs.
    pub fn store_titles(&self) -> bool {
        self.store_titles
    }
}
