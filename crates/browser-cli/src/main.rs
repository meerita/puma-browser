//! @file crates/browser-cli/src/main.rs
//! @description Composition root: resolve arguments, load once, wire the core to an adapter, run.
//! @layer cli
//! @created meerita <meerita@icloud.com>

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use browser_core::{
    history_mode_from_str, parse_policy, CookiePolicy, CookiePolicyPair, HistoryMode,
    HistorySettings, NavigationController, NavigationSource, SitePolicyStore,
};
use browser_mcp::McpServer;
use browser_network::BrowserUrl;
use browser_storage::{
    default_config_path, history_database_path, load_config, BrowserConfig, HistoryStore,
    SqliteStorage, StorageError, SuggestionEntry,
};
use browser_terminal::{TerminalApp, TerminalError, TerminalSettings, ViewState};

/// Seconds in one day, used to turn the retention window in days into a prune cutoff.
const SECONDS_PER_DAY: i64 = 86_400;

/// The mode keyword that selects the stdio MCP server instead of the terminal.
const MCP_MODE_KEYWORD: &str = "mcp";

/// Environment variable that disables copy-on-select when set to `0` or `false`.
const COPY_ON_SELECT_ENV: &str = "PUMA_COPY_ON_SELECT";

/// Environment variable that forces clipboard writes through OSC 52 when set to `1` or
/// `true`, for terminals reached over SSH where the native clipboard path is absent.
const CLIPBOARD_OSC52_ENV: &str = "PUMA_CLIPBOARD_OSC52";

/// Environment variable that disables the `/search` command when set to `0` or `false`.
const SEARCH_ENV: &str = "PUMA_SEARCH";

/// Environment variable that disables tracking-redirect unwrapping when set to `0` or
/// `false`.
const UNWRAP_TRACKING_ENV: &str = "PUMA_UNWRAP_TRACKING";

/// Environment variable that overrides the configured history mode, accepting
/// `disabled`, `in-memory`, or `persistent`. When set, it wins over the config file.
const HISTORY_MODE_ENV: &str = "PUMA_HISTORY_MODE";

/// The mode keyword that selects the terminal adapter; kept for symmetry with `mcp`.
const TERMINAL_MODE_KEYWORD: &str = "terminal";

/// A one-line reminder of how the binary is invoked, shown when an argument is rejected.
const USAGE_HINT: &str = "usage: puma [mcp | <url> | <path>]";

/// What the process should do, resolved from the command-line arguments alone.
///
/// This is the pure result of argument parsing: it names the adapter to run and, for a
/// terminal run started from a URL, carries the parsed load target. Resolving it does no
/// I/O, so it is tested without a runtime or a terminal.
enum ResolvedMode {
    Mcp,
    TerminalBlank,
    TerminalUrl(BrowserUrl),
    UsageError(String),
}

/// Resolves the process arguments into a [`ResolvedMode`].
///
/// The `mcp` keyword selects the MCP server. The `terminal` keyword is skipped so a later
/// argument can still be an address. The first argument that is neither keyword is treated
/// as the initial address and resolved against `working_directory`: a local file path or a
/// URL becomes the load target, and input that resolves to neither becomes a fail-fast
/// usage error. With no such argument the terminal opens on a blank page.
///
/// The working directory is threaded in rather than read here, so resolving the mode does
/// no filesystem probing of its own beyond what the resolver performs and stays testable
/// against a temporary directory.
fn resolve_mode(arguments: impl Iterator<Item = String>, working_directory: &Path) -> ResolvedMode {
    for argument in arguments {
        match argument.as_str() {
            MCP_MODE_KEYWORD => return ResolvedMode::Mcp,
            TERMINAL_MODE_KEYWORD => continue,
            _ => return resolve_address_argument(&argument, working_directory),
        }
    }
    ResolvedMode::TerminalBlank
}

/// Resolves a non-keyword argument to a load target, or reports a usage error.
///
/// The argument is resolved by [`browser_core::resolve_address`], which decides between a
/// local file path and a web address and validates the result. Any resolution failure
/// becomes the fail-fast usage error; the CLI holds no path-detection policy of its own.
fn resolve_address_argument(argument: &str, working_directory: &Path) -> ResolvedMode {
    match browser_core::resolve_address(argument, working_directory) {
        Ok(url) => ResolvedMode::TerminalUrl(url),
        Err(_) => ResolvedMode::UsageError(usage_error_message(argument)),
    }
}

/// A short, safe message for an argument that resolves to neither a file nor a URL.
///
/// The argument is the text the user typed, not remote content, so echoing it back is
/// safe; no network response or page text is involved.
fn usage_error_message(argument: &str) -> String {
    format!("Not a valid address: {argument}\n{USAGE_HINT}")
}

#[tokio::main]
async fn main() -> Result<()> {
    run(resolve_arguments()).await
}

/// Reads the working directory and resolves the process arguments into a [`ResolvedMode`].
///
/// The working directory is needed to resolve a relative or bare-token file argument. If
/// it cannot be read (for example the directory was removed), resolution fails fast with a
/// usage error rather than panicking.
fn resolve_arguments() -> ResolvedMode {
    let Ok(working_directory) = std::env::current_dir() else {
        return ResolvedMode::UsageError(format!(
            "Could not read the working directory\n{USAGE_HINT}"
        ));
    };
    resolve_mode(std::env::args().skip(1), &working_directory)
}

async fn run(resolved: ResolvedMode) -> Result<()> {
    // Resolve config up front so a malformed file fails fast with a safe message before
    // any adapter starts.
    let config = load_browser_config()?;
    let history_settings = resolve_history_settings(&config);
    match resolved {
        ResolvedMode::Mcp => run_mcp().await,
        ResolvedMode::TerminalBlank => {
            let controller = build_terminal_controller(&config, history_settings).await;
            run_terminal_app(controller, ViewState::Blank, None).await
        }
        ResolvedMode::TerminalUrl(url) => {
            let controller = build_terminal_controller(&config, history_settings).await;
            run_terminal_with_url(controller, url).await
        }
        ResolvedMode::UsageError(message) => Err(anyhow!(message)),
    }
}

/// Builds the terminal controller, wiring the history store, the cookie default, and the
/// per-site exception store.
///
/// The history store follows the resolved history mode. The cookie default comes from the
/// `[cookies]` config, and the per-site exception store opens on disk whenever a database
/// path resolves, independent of the history mode, so exceptions persist even when history
/// is in-memory or disabled. A failure to open either store degrades to no store rather
/// than blocking startup, so browsing always works.
async fn build_terminal_controller(
    config: &BrowserConfig,
    history_settings: HistorySettings,
) -> NavigationController {
    let (history, initial_suggestions) = open_history(history_settings, config.data_dir()).await;
    let (site_policies, initial_exceptions) = open_site_policy_store(config.data_dir()).await;
    let default_cookie_policy = resolve_cookie_policy(config);
    NavigationController::with_history(history, history_settings, initial_suggestions).with_cookies(
        default_cookie_policy,
        site_policies,
        initial_exceptions,
    )
}

/// Resolves the default cookie policy pair from the `[cookies]` config section.
///
/// Each scope word is mapped independently; an unrecognized value resolves to reject and
/// logs a warning. With no `[cookies]` section both scopes default to reject, so the
/// browser accepts no cookie until the user opts a scope or a site in.
fn resolve_cookie_policy(config: &BrowserConfig) -> CookiePolicyPair {
    CookiePolicyPair {
        first_party: resolve_scope_policy(config.cookie_first_party(), "cookies.first_party"),
        third_party: resolve_scope_policy(config.cookie_third_party(), "cookies.third_party"),
    }
}

/// Maps one scope's policy word to a [`CookiePolicy`], warning when it is unrecognized.
///
/// A recognized word maps through `parse_policy`. Any other value resolves to `Reject`, the
/// private default, and logs a warning naming the config key. The key is a fixed field name,
/// never the value, so nothing a user typed reaches the log.
fn resolve_scope_policy(word: &str, config_key: &str) -> CookiePolicy {
    let policy = parse_policy(word);
    if policy_word_is_unrecognized(word, policy) {
        tracing::warn!(
            config_key,
            "unrecognized cookie policy value; defaulting to reject"
        );
    }
    policy
}

/// Whether `word` was an unrecognized policy value that `parse_policy` fell back to reject.
///
/// `parse_policy` maps every unknown word to `Reject`, so a `Reject` result is genuine only
/// when the word actually spells "reject"; any other word that produced `Reject` was
/// unrecognized and worth warning about.
fn policy_word_is_unrecognized(word: &str, policy: CookiePolicy) -> bool {
    matches!(policy, CookiePolicy::Reject) && !word.trim().eq_ignore_ascii_case("reject")
}

/// Opens the history store and initial suggestions for the resolved mode.
async fn open_history(
    history_settings: HistorySettings,
    data_dir: Option<&Path>,
) -> (
    Option<Arc<dyn HistoryStore + Send + Sync>>,
    Vec<SuggestionEntry>,
) {
    match history_settings.mode() {
        HistoryMode::Disabled => (None, Vec::new()),
        HistoryMode::InMemory => open_in_memory_history(),
        HistoryMode::Persistent => open_persistent_history(history_settings, data_dir).await,
    }
}

/// Opens an ephemeral in-memory database, starting with an empty index.
///
/// The database is discarded on exit, so recorded history never reaches disk. A failure
/// to open degrades to no store.
fn open_in_memory_history() -> (
    Option<Arc<dyn HistoryStore + Send + Sync>>,
    Vec<SuggestionEntry>,
) {
    match SqliteStorage::open_in_memory() {
        Ok(storage) => (Some(into_history_store(storage)), Vec::new()),
        Err(_) => (None, Vec::new()),
    }
}

/// Opens the on-disk database, prunes to the retention window, and loads the index.
///
/// The blocking SQLite work runs on a blocking thread. A failure at any step degrades to
/// no store and an empty index rather than blocking startup.
async fn open_persistent_history(
    history_settings: HistorySettings,
    data_dir: Option<&Path>,
) -> (
    Option<Arc<dyn HistoryStore + Send + Sync>>,
    Vec<SuggestionEntry>,
) {
    let Ok(path) = history_database_path(data_dir) else {
        return (None, Vec::new());
    };
    let cutoff = retention_cutoff(history_settings.retention_days());
    let prepared = tokio::task::spawn_blocking(move || open_and_prune(&path, cutoff)).await;
    match prepared {
        Ok(Ok((storage, suggestions))) => (Some(into_history_store(storage)), suggestions),
        _ => (None, Vec::new()),
    }
}

/// Wraps a concrete storage backend as a shared trait object for the controller.
fn into_history_store(storage: SqliteStorage) -> Arc<dyn HistoryStore + Send + Sync> {
    Arc::new(storage)
}

/// Per-site cookie policy exceptions as domain-and-policy pairs, as loaded from the store.
type SiteExceptions = Vec<(String, CookiePolicy)>;

/// A shared per-site policy store paired with the exceptions loaded from it, as the
/// composition root hands them to the controller.
type SitePolicyStoreWithExceptions = (
    Option<Arc<dyn SitePolicyStore + Send + Sync>>,
    SiteExceptions,
);

/// Opens the per-site cookie policy exception store and loads the exceptions already saved.
///
/// The store opens on disk whenever a database path resolves, independent of the history
/// mode, because exceptions persist across runs even when history is in-memory or disabled.
/// When no on-disk path resolves, an in-memory store keeps exceptions for the run only. A
/// failure to open degrades to no store rather than blocking startup. Only domain-plus-policy
/// rows are ever read or written here; no cookie value is stored.
async fn open_site_policy_store(data_dir: Option<&Path>) -> SitePolicyStoreWithExceptions {
    match history_database_path(data_dir) {
        Ok(path) => open_persistent_site_policies(path).await,
        Err(_) => open_in_memory_site_policies(),
    }
}

/// Opens the on-disk database and loads the saved exceptions into policy pairs.
///
/// The blocking SQLite work runs on a blocking thread. A failure at any step degrades to no
/// store and no exceptions rather than blocking startup.
async fn open_persistent_site_policies(path: PathBuf) -> SitePolicyStoreWithExceptions {
    let prepared = tokio::task::spawn_blocking(move || open_and_load_site_policies(&path)).await;
    match prepared {
        Ok(Ok((storage, exceptions))) => (Some(into_site_policy_store(storage)), exceptions),
        _ => (None, Vec::new()),
    }
}

/// Opens an ephemeral in-memory database for exceptions that do not persist across runs.
///
/// The database is discarded on exit, so exceptions set this run are not saved. A failure
/// to open degrades to no store.
fn open_in_memory_site_policies() -> SitePolicyStoreWithExceptions {
    match SqliteStorage::open_in_memory() {
        Ok(storage) => (Some(into_site_policy_store(storage)), Vec::new()),
        Err(_) => (None, Vec::new()),
    }
}

/// Opens the database at `path` and reads the saved exceptions, mapping each policy word
/// through `parse_policy`.
///
/// Runs on a blocking thread because every call is synchronous SQLite.
fn open_and_load_site_policies(
    path: &Path,
) -> Result<(SqliteStorage, SiteExceptions), StorageError> {
    let storage = SqliteStorage::open(path)?;
    let exceptions = storage
        .all_site_policies()?
        .into_iter()
        .map(|(domain, policy)| (domain, parse_policy(&policy)))
        .collect();
    Ok((storage, exceptions))
}

/// Wraps a concrete storage backend as a shared site-policy trait object for the controller.
fn into_site_policy_store(storage: SqliteStorage) -> Arc<dyn SitePolicyStore + Send + Sync> {
    Arc::new(storage)
}

/// Opens the database at `path`, prunes visits older than `cutoff`, and reads the index.
///
/// Runs on a blocking thread because every call is synchronous SQLite.
fn open_and_prune(
    path: &Path,
    cutoff: i64,
) -> Result<(SqliteStorage, Vec<SuggestionEntry>), StorageError> {
    let storage = SqliteStorage::open(path)?;
    storage.prune_older_than(cutoff)?;
    let suggestions = storage.load_suggestions()?;
    Ok((storage, suggestions))
}

/// The Unix-epoch cutoff before which visits are pruned, given a retention window in days.
fn retention_cutoff(retention_days: u32) -> i64 {
    now_unix_seconds() - i64::from(retention_days) * SECONDS_PER_DAY
}

/// The current time as Unix epoch seconds, or zero if the clock predates the epoch.
fn now_unix_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// Loads the page at `url` on `controller`, then opens the terminal on the result.
///
/// A fragment on the startup URL is split off before the load and carried into the
/// terminal, which positions the opening viewport on the matching anchor once the page
/// renders. The load runs once here, before the synchronous event loop starts. A failed
/// load still opens the terminal on an error page so the user sees a safe message and
/// quits with `Esc Esc`; it is never a hard exit.
async fn run_terminal_with_url(
    mut controller: NavigationController,
    url: BrowserUrl,
) -> Result<()> {
    let fragment = url.fragment().map(str::to_string);
    let base = url.without_fragment();
    let view_state = load_initial_view(&mut controller, base).await;
    run_terminal_app(controller, view_state, fragment).await
}

/// Resolves a load into the initial view the terminal opens on.
///
/// Success becomes [`ViewState::Page`]. Failure becomes [`ViewState::Error`] carrying
/// only the safe terminal `user_message` for the error, never raw error detail.
async fn load_initial_view(controller: &mut NavigationController, url: BrowserUrl) -> ViewState {
    match controller.load(url, NavigationSource::AddressBar).await {
        Ok(()) => ViewState::Page,
        Err(core_error) => ViewState::Error(TerminalError::from(core_error).user_message()),
    }
}

async fn run_terminal_app(
    controller: NavigationController,
    view_state: ViewState,
    initial_fragment: Option<String>,
) -> Result<()> {
    let settings = terminal_settings_from_env();
    let mut app =
        TerminalApp::new(controller, view_state, settings).with_initial_fragment(initial_fragment);
    // Surface only the adapter's safe status message, never raw error detail.
    app.run()
        .await
        .map_err(|error| anyhow!(error.user_message()))
}

/// Reads the terminal settings from the environment once, at terminal startup.
///
/// Only the two documented variables are consulted; nothing here can enable a
/// remote-triggered behavior, and a disabled `copy_on_select` fully suppresses the
/// clipboard write in the adapter.
fn terminal_settings_from_env() -> TerminalSettings {
    let copy_on_select = copy_on_select_enabled(std::env::var(COPY_ON_SELECT_ENV).ok().as_deref());
    let force_osc52 = force_osc52_enabled(std::env::var(CLIPBOARD_OSC52_ENV).ok().as_deref());
    let search_enabled = search_enabled(std::env::var(SEARCH_ENV).ok().as_deref());
    let unwrap_tracking =
        unwrap_tracking_enabled(std::env::var(UNWRAP_TRACKING_ENV).ok().as_deref());
    TerminalSettings {
        copy_on_select,
        force_osc52,
        search_enabled,
        unwrap_tracking,
    }
}

/// Whether copy-on-select is enabled given the raw value of `PUMA_COPY_ON_SELECT`.
///
/// Enabled by default and when the variable is unset; only an explicit `0` or `false`
/// turns it off. Taking the value as an argument keeps this testable without touching
/// the real process environment.
fn copy_on_select_enabled(value: Option<&str>) -> bool {
    !matches!(value, Some("0") | Some("false"))
}

/// Whether clipboard writes are forced through OSC 52 given the raw value of
/// `PUMA_CLIPBOARD_OSC52`.
///
/// Off by default and when the variable is unset; only an explicit `1` or `true` enables
/// it. Taking the value as an argument keeps this testable without the real environment.
fn force_osc52_enabled(value: Option<&str>) -> bool {
    matches!(value, Some("1") | Some("true"))
}

/// Whether the `/search` command is enabled given the raw value of `PUMA_SEARCH`.
///
/// Enabled by default and when the variable is unset; only an explicit `0` or `false`
/// turns it off. Taking the value as an argument keeps this testable without touching
/// the real process environment.
fn search_enabled(value: Option<&str>) -> bool {
    !matches!(value, Some("0") | Some("false"))
}

/// Whether tracking-redirect unwrapping is enabled given the raw value of
/// `PUMA_UNWRAP_TRACKING`.
///
/// Enabled by default and when the variable is unset; only an explicit `0` or `false`
/// turns it off. Taking the value as an argument keeps this testable without touching the
/// real process environment.
fn unwrap_tracking_enabled(value: Option<&str>) -> bool {
    !matches!(value, Some("0") | Some("false"))
}

/// Resolves the history settings from `config` overlaid with the env override.
///
/// The mode env override wins over the file value when set; the retention window and
/// title toggle come from the resolved config.
fn resolve_history_settings(config: &BrowserConfig) -> HistorySettings {
    let mode = resolve_history_mode(
        config.history_mode(),
        std::env::var(HISTORY_MODE_ENV).ok().as_deref(),
    );
    HistorySettings::new(mode, config.retention_days(), config.store_titles())
}

/// Loads the browser config, falling back to defaults when the path cannot be resolved.
///
/// Only a malformed file is an error, and it is re-described with a safe message so no
/// path or file content reaches the caller. A missing file already resolves to defaults
/// inside `load_config`.
fn load_browser_config() -> Result<BrowserConfig> {
    let Ok(config_path) = default_config_path() else {
        return Ok(BrowserConfig::default());
    };
    load_config(&config_path).map_err(|_| anyhow!("Configuration file is invalid"))
}

/// Resolves the effective history mode from the file value and the optional env override.
///
/// The environment variable wins when set; otherwise the file value applies. Either
/// string is mapped through `history_mode_from_str`, so an unrecognized value resolves to
/// the persistent default instead of being rejected. Taking both as arguments keeps this
/// testable without the real process environment.
fn resolve_history_mode(file_mode: &str, env_override: Option<&str>) -> HistoryMode {
    let selected = env_override.unwrap_or(file_mode);
    history_mode_from_str(selected)
}

async fn run_mcp() -> Result<()> {
    // The MCP controller takes no site-policy store and no config: it keeps the
    // reject-by-default pair and an in-memory jar, so no cookie value ever persists to disk
    // or leaks through an MCP response.
    let server = McpServer::new(NavigationController::new());
    server
        .run()
        .await
        .map_err(|error| anyhow!("MCP server failed: {}", error.reason_code()))
}

#[cfg(test)]
#[path = "run_mode_tests.rs"]
mod tests;
