// @file crates/browser-storage/src/config.rs
// @description Loads the user TOML config over built-in defaults into a typed BrowserConfig.
// @layer storage
// @created meerita <meerita@icloud.com>

use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use serde::Deserialize;

use crate::error::StorageError;

/// File name of the browser configuration inside the platform config directory.
const CONFIG_FILE_NAME: &str = "config.toml";

/// Built-in default history mode when the file omits it.
const DEFAULT_HISTORY_MODE: &str = "persistent";

/// Built-in default retention window in days when the file omits it.
const DEFAULT_RETENTION_DAYS: u32 = 90;

/// Built-in default for whether page titles are stored, when the file omits it.
const DEFAULT_STORE_TITLES: bool = true;

/// Built-in default cookie policy for both scopes when the file omits them: cookies are
/// rejected unless the user opts a scope or a site in.
const DEFAULT_COOKIE_POLICY: &str = "reject";

/// The on-disk representation of the configuration file.
///
/// Every field is optional so a partial or empty file is valid; missing values fall
/// back to the built-in defaults during overlay. Unknown keys and sections are ignored
/// rather than rejected, so a newer file remains loadable by an older binary. This is an
/// adapter type owning the wire shape, so it derives `serde`; the resolved domain values
/// live in [`BrowserConfig`].
#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    data_dir: Option<String>,
    history: Option<HistoryFile>,
    cookies: Option<CookiesFile>,
}

/// The `[history]` table of the configuration file.
#[derive(Debug, Default, Deserialize)]
struct HistoryFile {
    mode: Option<String>,
    retention_days: Option<u32>,
    store_titles: Option<bool>,
}

/// The `[cookies]` table of the configuration file.
#[derive(Debug, Default, Deserialize)]
struct CookiesFile {
    first_party: Option<String>,
    third_party: Option<String>,
}

/// The resolved browser configuration after overlaying the file onto the defaults.
///
/// `history_mode` is kept as the raw string here; the domain enum it maps to lives in
/// `browser-core`, which owns history semantics. Storage neither interprets the mode nor
/// depends on the core crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserConfig {
    history_mode: String,
    retention_days: u32,
    store_titles: bool,
    cookie_first_party: String,
    cookie_third_party: String,
    data_dir: Option<PathBuf>,
}

impl Default for BrowserConfig {
    fn default() -> Self {
        Self {
            history_mode: DEFAULT_HISTORY_MODE.to_string(),
            retention_days: DEFAULT_RETENTION_DAYS,
            store_titles: DEFAULT_STORE_TITLES,
            cookie_first_party: DEFAULT_COOKIE_POLICY.to_string(),
            cookie_third_party: DEFAULT_COOKIE_POLICY.to_string(),
            data_dir: None,
        }
    }
}

impl BrowserConfig {
    /// The raw history-mode string, mapped to the domain enum by `browser-core`.
    pub fn history_mode(&self) -> &str {
        &self.history_mode
    }

    /// The retention window in days before old history is pruned.
    pub fn retention_days(&self) -> u32 {
        self.retention_days
    }

    /// Whether page titles are stored alongside visited URLs.
    pub fn store_titles(&self) -> bool {
        self.store_titles
    }

    /// The raw first-party cookie policy string, mapped to the domain enum by
    /// `browser-core`.
    pub fn cookie_first_party(&self) -> &str {
        &self.cookie_first_party
    }

    /// The raw third-party cookie policy string, mapped to the domain enum by
    /// `browser-core`.
    pub fn cookie_third_party(&self) -> &str {
        &self.cookie_third_party
    }

    /// The user-configured data directory override, if one is set.
    pub fn data_dir(&self) -> Option<&Path> {
        self.data_dir.as_deref()
    }

    /// Overlays a parsed file onto these defaults, returning the resolved configuration.
    ///
    /// A missing section keeps every default: an absent `[history]` or `[cookies]`
    /// table resolves to an all-`None` file struct, so each field falls back through
    /// `unwrap_or`.
    fn overlay(self, file: ConfigFile) -> Self {
        let data_dir = file.data_dir.map(PathBuf::from).or(self.data_dir);
        let history = file.history.unwrap_or_default();
        let cookies = file.cookies.unwrap_or_default();
        Self {
            history_mode: history.mode.unwrap_or(self.history_mode),
            retention_days: history.retention_days.unwrap_or(self.retention_days),
            store_titles: history.store_titles.unwrap_or(self.store_titles),
            cookie_first_party: cookies.first_party.unwrap_or(self.cookie_first_party),
            cookie_third_party: cookies.third_party.unwrap_or(self.cookie_third_party),
            data_dir,
        }
    }
}

/// Loads the configuration at `config_path`, overlaying it onto the built-in defaults.
///
/// A missing file is not an error: the built-in defaults are returned unchanged so a
/// first run needs no configuration. A file that exists but cannot be parsed maps to
/// [`StorageError::ConfigInvalid`], whose message carries neither the path nor the file
/// contents so nothing sensitive reaches a caller or a log.
pub fn load_config(config_path: &Path) -> Result<BrowserConfig, StorageError> {
    let contents = match std::fs::read_to_string(config_path) {
        Ok(contents) => contents,
        Err(_) => return Ok(BrowserConfig::default()),
    };
    let file: ConfigFile = toml::from_str(&contents).map_err(|_| StorageError::ConfigInvalid)?;
    Ok(BrowserConfig::default().overlay(file))
}

/// Returns the default path of the configuration file inside the platform config
/// directory.
///
/// The `ProjectDirs` lookup can fail on a platform with no resolvable home directory;
/// that maps to [`StorageError::OpenFailed`], because from a caller's point of view the
/// configuration location could not be established.
pub fn default_config_path() -> Result<PathBuf, StorageError> {
    let project_dirs = ProjectDirs::from("", "", "puma").ok_or(StorageError::OpenFailed)?;
    Ok(project_dirs.config_dir().join(CONFIG_FILE_NAME))
}
