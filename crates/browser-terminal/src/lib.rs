//! @file crates/browser-terminal/src/lib.rs
//! @description Terminal adapter: scrollable read-only viewport over the navigation core.
//! @layer terminal
//! @created meerita <meerita@icloud.com>

mod clipboard;
// The registry and matcher land ahead of the dispatch and palette code that call them.
#[allow(dead_code)]
mod command;
mod command_bar;
mod cookie_view;
mod error;
mod hints_bar;
mod history_view;
mod input;
mod palette_menu;
mod selection;
mod settings_view;
mod title_bar;
mod ui_state;
mod view_state;
mod viewport;

pub use error::TerminalError;
pub use view_state::ViewState;

use std::io::Stdout;
use std::path::PathBuf;
use std::time::Instant;

use browser_core::{
    BrowserUrl, ButtonKind, CookiePolicy, CoreError, FormFieldValues, FormMethod, HistoryEntryId,
    InputKind, NavigationController, NavigationSource, NavigationTarget, NodeId, SelectElement,
    SelectOption,
};
use browser_css::{Color, Emphasis};
use browser_layout::{
    AnchorSpan, Cell, CellBuffer, CellPosition, FieldSpanKind, LinkKind, LinkSpan,
};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures_util::StreamExt;
use percent_encoding::percent_decode_str;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color as TerminalColor, Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

use clipboard::{copy_to_clipboard, ClipboardOutcome};
use command::{parse_command_input, parse_cookies_request, CommandKind, CookiesRequest};
use command_bar::{
    command_cursor_col, compose_command_bar_command, compose_command_bar_loading,
    compose_command_bar_reading,
};
use cookie_view::CookieFilter;
use hints_bar::compose_hints_bar;
use history_view::{
    compose_list_menu, format_history_label, now_unix_seconds, strip_control, ListMenu,
    HISTORY_QUERY_LIMIT, LIST_MENU_MAX_ROWS,
};
use input::{map_key_event, quit_armed_after, refresh_armed_after, InputAction};
use palette_menu::{compose_palette_menu, PaletteMenu, MENU_MAX_ROWS};
use selection::TextSelection;
use settings_view::{
    build_settings_model, checkbox_config_key, cookie_scope, CycleDirection, RadioOption,
    SettingId, SettingsControl, SettingsModel, SettingsRow,
};
use title_bar::compose_title_bar;
use ui_state::{SettingsEscOutcome, SettingsTextSave, SubmitChoice, UiState};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use viewport::{max_scroll_offset, scroll_percentage, ScrollState, ViewportBounds};

/// Rows consumed by the five fixed chrome zones (title + sep + cmd + sep + hints).
const CHROME_ROWS: u16 = 5;

/// Terminal columns of left and right padding on the content area.
const CONTENT_PADDING: u16 = 2;

/// The terminal row where the page body begins. The vertical layout places the content
/// area first, so the body starts at the top of the screen and mouse rows map directly
/// onto buffer rows once the scroll offset is added.
const BODY_AREA_TOP_ROW: u16 = 0;

/// The body text shown when the terminal opens with no page.
const BLANK_PLACEHOLDER: &str = "(blank page)";

/// The concrete terminal this adapter draws to: a crossterm backend over stdout.
type AppTerminal = Terminal<CrosstermBackend<Stdout>>;

/// Handle returned by the load task, carrying the controller back alongside the result.
type LoadHandle = JoinHandle<(NavigationController, Result<(), CoreError>)>;

/// Whether a page load is in progress and the data needed to animate and restore state.
enum LoadState {
    Idle,
    Active {
        handle: LoadHandle,
        progress_rx: watch::Receiver<usize>,
        spinner_frame: usize,
        loading_url: String,
    },
}

/// What activating a focused interactive target asks the event loop to do next: load a
/// link's URL through the ordinary navigation path, or submit a form's fields through
/// the submission path.
enum NavigationRequest {
    LoadUrl(String),
    Submit(NodeId),
}

/// The result of submitting a command-bar buffer: nothing further to do, a new load to
/// track, a request to leave the event loop, or a history query to run.
enum CommandOutcome {
    None,
    Load(LoadState),
    Quit,
    History(HistoryRequest),
}

/// A `/history` request parsed from the command buffer, run asynchronously by the event
/// loop because it reads or writes the store off the async task.
enum HistoryRequest {
    List,
    Search(String),
    ClearAll,
    ClearSite(String),
}

/// A settings-panel change the user asked for with a key while a row is focused: toggle the
/// focused checkbox, or move the focused radio group's selection one option in a direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsMutation {
    Toggle,
    CyclePrev,
    CycleNext,
}

/// The settings mutation a key action requests, or `None` when it is not a settings mutation.
/// Mirrors the palette and suggestion key maps: a small pure classifier the event loop reads.
fn settings_mutation_for(action: InputAction) -> Option<SettingsMutation> {
    match action {
        InputAction::SettingsToggle => Some(SettingsMutation::Toggle),
        InputAction::SettingsCyclePrev => Some(SettingsMutation::CyclePrev),
        InputAction::SettingsCycleNext => Some(SettingsMutation::CycleNext),
        _ => None,
    }
}

/// The message shown when the user tries to change a row a `PUMA_*` environment variable fixes
/// for the session.
const SETTINGS_ENV_LOCKED: &str = "This setting is fixed by an environment variable";

/// The message shown when a setting changed for the running session but could not be saved.
const SETTINGS_SAVE_FAILED: &str = "Setting changed for this session but could not be saved";

/// The confirmation shown briefly after a settings text field saves.
const SETTINGS_TEXT_SAVED: &str = "Setting saved";

/// The inline note shown on a settings text row when its value was rejected and not saved. The
/// message names no internals, only that the value is invalid.
const SETTINGS_TEXT_INVALID: &str = "invalid value, not saved";

/// The number of address-bar suggestions requested from the index on each edit.
const ADDRESS_SUGGESTION_LIMIT: usize = 8;

/// The hint shown while the history list is open, describing its controls.
const HISTORY_CONTROLS_HINT: &str = "↑↓ select · Enter open · Del delete · Esc close";

/// The usage line shown when a `/cookies` argument is not a recognized subcommand.
const COOKIES_USAGE: &str =
    "usage: /cookies [accepted | rejected | clear | allow-session <site> | reject <site>]";

/// Drives the terminal user interface over the navigation core.
///
/// This is the output adapter the terminal binary builds on. It reads the laid-out cell
/// buffer from the core and blits a vertical window of it into a Ratatui viewport; it
/// never runs the fetch/parse/layout pipeline itself. Every grapheme it draws for a page
/// already passed the parse-stage sanitizer, so no escape sequence from remote content
/// can reach the terminal.
#[derive(Debug)]
pub struct TerminalApp {
    controller: NavigationController,
    view_state: ViewState,
    settings: TerminalSettings,
    initial_fragment: Option<String>,
}

/// Runtime settings for the terminal adapter, resolved once at startup.
///
/// `copy_on_select` gates the copy-on-release behavior: when it is false a drag still
/// highlights text but releasing the button copies nothing and shows no confirmation.
/// `force_osc52` routes every clipboard write through the OSC 52 escape path instead of
/// the native clipboard, for terminals reached over SSH where the native path is absent.
/// `search_enabled` gates the `/search` command: when it is false the command is hidden
/// from the palette and rejected on dispatch.
/// `unwrap_tracking` gates tracking-redirect unwrapping: when it is true a navigation to a
/// known tracker wrapper goes straight to the decoded destination instead of the wrapper.
/// `env_overridden` records which toggles a `PUMA_*` environment variable currently fixes
/// for the session, so the settings panel can render those rows read-only.
#[derive(Debug, Clone, Copy)]
pub struct TerminalSettings {
    pub copy_on_select: bool,
    pub force_osc52: bool,
    pub search_enabled: bool,
    pub unwrap_tracking: bool,
    pub env_overridden: EnvOverrides,
}

/// Which toggle settings an environment variable currently overrides for the session.
///
/// A `true` field means a `PUMA_*` variable is set for that toggle, so its live value is
/// fixed for the run and the settings panel shows the row read-only rather than editable.
/// The default is all-false: with no variables set, every toggle is editable.
#[derive(Debug, Clone, Copy, Default)]
pub struct EnvOverrides {
    pub copy_on_select: bool,
    pub force_osc52: bool,
    pub search_enabled: bool,
    pub unwrap_tracking: bool,
}

/// A cell buffer cached alongside the content width it was laid out for, so the page
/// is re-rendered only when the content area width changes.
struct CachedPage {
    width: u16,
    buffer: CellBuffer,
}

impl TerminalApp {
    pub fn new(
        controller: NavigationController,
        view_state: ViewState,
        settings: TerminalSettings,
    ) -> Self {
        Self {
            controller,
            view_state,
            settings,
            initial_fragment: None,
        }
    }

    /// Sets the fragment to position the initial page on once it first renders.
    ///
    /// The startup page is loaded before the event loop begins, so its fragment cannot go
    /// through the normal load path. Seeding it here lets the same deferred positioning that
    /// serves cross-page links land the opening viewport on the anchor.
    pub fn with_initial_fragment(mut self, fragment: Option<String>) -> Self {
        self.initial_fragment = fragment;
        self
    }

    /// Borrows the navigation core this adapter drives.
    pub fn controller(&self) -> &NavigationController {
        &self.controller
    }

    /// The tracking-unwrap mode both `classify_navigation` call sites pass, derived from the
    /// resolved `unwrap_tracking` setting so the two paths never diverge.
    fn tracking_unwrap_mode(&self) -> browser_core::TrackingUnwrap {
        if self.settings.unwrap_tracking {
            browser_core::TrackingUnwrap::Enabled
        } else {
            browser_core::TrackingUnwrap::Disabled
        }
    }

    /// Runs the terminal event loop until the user quits.
    ///
    /// Enters the alternate screen and raw mode on start and restores the terminal on
    /// every exit path, including errors, so the shell is never left in raw mode. A
    /// draw or backend failure is reported as [`TerminalError::RenderFailed`] with no
    /// raw error detail.
    pub async fn run(&mut self) -> Result<(), TerminalError> {
        let mut terminal = install_terminal()?;
        let outcome = self.drive(&mut terminal).await;
        restore_terminal(&mut terminal);
        outcome
    }

    async fn drive(&mut self, terminal: &mut AppTerminal) -> Result<(), TerminalError> {
        // Only the initial search-enabled value is read here to seed the palette filter; the
        // copy-on-select and force-osc52 settings are read live from `self.settings` at each
        // mouse release so a panel toggle changes clipboard behavior at once.
        let TerminalSettings { search_enabled, .. } = self.settings;
        let mut scroll = ScrollState::new();
        let mut selection = TextSelection::new();
        let mut ui_state = UiState::new(search_enabled);
        // A fragment on the startup URL is honored through the same deferred path a
        // cross-page link uses: seed it here so the first render positions the viewport.
        ui_state.set_pending_fragment(self.initial_fragment.take());
        let mut cache: Option<CachedPage> = None;
        let mut load_state = LoadState::Idle;
        let mut tick = interval(Duration::from_millis(80));
        let mut event_stream = EventStream::new();
        let working_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

        loop {
            let now = Instant::now();
            ui_state.advance_hint_if_due(now);
            ui_state.clear_transient_if_expired(now);

            let size = terminal.size().map_err(|_| TerminalError::RenderFailed)?;
            let viewport_height = size.height.saturating_sub(CHROME_ROWS);
            let content_width = size.width.saturating_sub(CONTENT_PADDING * 2);

            if matches!(load_state, LoadState::Idle) {
                self.refresh_page_cache(&mut cache, content_width)?;
            }

            let content_rows = cache.as_ref().map_or(0, |cached| cached.buffer.height());
            let max_offset = max_scroll_offset(content_rows, viewport_height);
            scroll.clamp(max_offset);

            // A fragment remembered from a completed load positions the viewport once the
            // freshly rendered page is cached. Only when idle, so it never resolves against
            // the stale buffer still showing during a load.
            if matches!(load_state, LoadState::Idle) {
                self.apply_pending_fragment(&mut ui_state, &cache, &mut scroll, max_offset);
            }

            // Extract loading state as owned values to avoid borrow conflicts with the
            // mutable select! arm below.
            let loading_snapshot: Option<(usize, String, usize)> = if let LoadState::Active {
                ref progress_rx,
                spinner_frame,
                ref loading_url,
                ..
            } = load_state
            {
                Some((spinner_frame, loading_url.clone(), *progress_rx.borrow()))
            } else {
                None
            };

            let label = loading_snapshot
                .as_ref()
                .map(|(_, url, _)| url.clone())
                .unwrap_or_else(|| self.status_label());

            let loading_ref = loading_snapshot
                .as_ref()
                .map(|(frame, url, bytes)| (*frame, url.as_str(), *bytes));
            let page = cache.as_ref().map(|cached| &cached.buffer);
            let scroll_percent_val = scroll_percentage(scroll.offset(), max_offset);
            let selection_range = selection.range();
            self.draw(
                terminal,
                page,
                scroll.offset(),
                &ui_state,
                &label,
                scroll_percent_val,
                self.controller.script_count(),
                self.controller.page_byte_count(),
                self.controller.cookie_counts(),
                loading_ref,
                selection_range,
                self.controller.field_values(),
            )?;

            // Accumulators for state transitions; populated inside the select! arms and
            // applied after the borrow on load_state is released.
            let mut completed_load: Option<(NavigationController, Result<(), CoreError>)> = None;
            let mut command_to_submit: Option<String> = None;
            let mut reload_url: Option<BrowserUrl> = None;
            let mut navigation_request: Option<NavigationRequest> = None;
            let mut history_key_action: Option<InputAction> = None;
            let mut settings_mutation: Option<SettingsMutation> = None;

            if let LoadState::Active {
                ref mut handle,
                ref mut spinner_frame,
                ..
            } = load_state
            {
                tokio::select! {
                    join_result = handle => {
                        let (ctrl, result) = join_result.expect("load task must not panic");
                        completed_load = Some((ctrl, result));
                    }
                    _ = tick.tick() => {
                        *spinner_frame += 1;
                    }
                    maybe_event = event_stream.next() => {
                        if let Some(Ok(Event::Key(key))) = maybe_event {
                            if key.kind != KeyEventKind::Release
                                && key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL)
                            {
                                return Ok(());
                            }
                        }
                    }
                }
            } else {
                tokio::select! {
                    maybe_event = event_stream.next() => {
                        let Some(Ok(event)) = maybe_event else { continue; };
                        match event {
                            Event::Key(key) if key.kind != KeyEventKind::Release => {
                                let in_command_mode = ui_state.is_in_command_mode();
                                let in_interactive_navigation = ui_state.is_in_interactive_navigation();
                                let palette_active = ui_state.is_palette_active();
                                let address_suggestions_active = ui_state.has_address_suggestions();
                                let in_history = ui_state.is_in_history_mode();
                                let in_cookies = ui_state.is_in_cookies_mode();
                                let in_settings = ui_state.is_in_settings_mode();
                                let settings_text_field_focused =
                                    ui_state.is_settings_text_field_focused();
                                let in_field_text_edit = ui_state.is_in_field_text_edit();
                                let in_field_multi_select = ui_state.is_in_field_multi_select();
                                let in_submit_confirmation = ui_state.is_in_submit_confirmation();
                                let action = map_key_event(
                                    key,
                                    ui_state.quit_armed,
                                    ui_state.refresh_armed,
                                    in_command_mode,
                                    in_interactive_navigation,
                                    palette_active,
                                    address_suggestions_active,
                                    in_history,
                                    in_cookies,
                                    in_settings,
                                    settings_text_field_focused,
                                    in_field_text_edit,
                                    in_field_multi_select,
                                    in_submit_confirmation,
                                );
                                if matches!(action, InputAction::Quit) {
                                    return Ok(());
                                }
                                ui_state.clear_transient();
                                apply_scroll(action, &mut scroll, viewport_height, max_offset);
                                if matches!(action, InputAction::CommandSubmit) {
                                    command_to_submit = Some(match ui_state.take_selected_suggestion() {
                                        Some(url) => url,
                                        None => ui_state.take_submit_buffer(),
                                    });
                                } else {
                                    apply_command_action(action, &mut ui_state);
                                }
                                apply_settings_text_action(action, &mut ui_state, now);
                                apply_field_text_action(action, &mut ui_state);
                                if action_refreshes_suggestions(action) {
                                    self.refresh_address_suggestions(&mut ui_state);
                                }
                                if matches!(action, InputAction::RefreshArmed) {
                                    reload_url = self.controller.current_url().cloned();
                                }
                                if matches!(
                                    action,
                                    InputAction::HistoryDeleteSelected
                                        | InputAction::HistoryActivateSelected
                                ) {
                                    history_key_action = Some(action);
                                }
                                settings_mutation = settings_mutation_for(action);
                                if let Some(request) = handle_navigation_action(
                                    action,
                                    &mut self.controller,
                                    &mut ui_state,
                                    &mut cache,
                                    &mut scroll,
                                    &mut self.view_state,
                                    ViewportBounds {
                                        height: viewport_height,
                                        max_offset,
                                    },
                                ) {
                                    navigation_request = Some(request);
                                }
                                ui_state.quit_armed = quit_armed_after(action);
                                ui_state.refresh_armed = refresh_armed_after(action);
                                if matches!(action, InputAction::ArmQuit) {
                                    ui_state.set_transient_hint("Press Esc again to quit", now);
                                } else if matches!(action, InputAction::ArmRefresh) {
                                    ui_state.set_transient_hint("Press r again to refresh", now);
                                }
                            }
                            Event::Mouse(mouse) => {
                                let mut mouse_navigate_url: Option<String> = None;
                                handle_mouse_event(
                                    mouse,
                                    cache.as_ref().map(|cached| &cached.buffer),
                                    scroll.offset(),
                                    BODY_AREA_TOP_ROW,
                                    &mut selection,
                                    &mut mouse_navigate_url,
                                    &mut ui_state,
                                    now,
                                    self.settings.copy_on_select,
                                    self.settings.force_osc52,
                                );
                                if let Some(url) = mouse_navigate_url {
                                    navigation_request = Some(NavigationRequest::LoadUrl(url));
                                }
                            }
                            _ => {}
                        }
                    }
                    _ = tick.tick() => {}
                }
            }

            // Apply completed load — borrow on load_state has ended. A new page invalidates
            // any highlight held over document coordinates from the previous page.
            if let Some((ctrl, result)) = completed_load {
                self.controller = ctrl;
                load_state = LoadState::Idle;
                selection.clear();
                ui_state.clear_anchor_returns();
                match result {
                    Ok(()) => {
                        self.view_state = ViewState::Page;
                        scroll = ScrollState::new();
                        cache = None;
                    }
                    Err(error) => {
                        self.view_state =
                            ViewState::Error(TerminalError::from(error).user_message());
                        scroll = ScrollState::new();
                        cache = None;
                        // A failed load renders no page, so any fragment waiting for it is
                        // dropped rather than resolved against the error view.
                        ui_state.set_pending_fragment(None);
                    }
                }
            }

            // Apply command submission. A leading `/` routes to the command dispatcher;
            // anything else is a URL, exactly as before.
            if let Some(input) = command_to_submit {
                match self.submit_command_input(
                    input,
                    &working_dir,
                    &mut ui_state,
                    &mut cache,
                    &mut scroll,
                    max_offset,
                    now,
                ) {
                    CommandOutcome::Quit => return Ok(()),
                    CommandOutcome::Load(state) => load_state = state,
                    CommandOutcome::History(request) => {
                        self.handle_history_request(request, &mut ui_state, now)
                            .await;
                    }
                    CommandOutcome::None => {}
                }
            }

            // Apply a history-list key action: delete removes the selected entry through the
            // store, activate opens it through the shared address path.
            if let Some(history_action) = history_key_action {
                if let Some(state) = self
                    .apply_history_key_action(
                        history_action,
                        &mut ui_state,
                        &working_dir,
                        &mut cache,
                        &mut scroll,
                        max_offset,
                        now,
                    )
                    .await
                {
                    load_state = state;
                }
            }

            // Apply a settings-panel mutation: flip a checkbox or cycle a radio, applying the
            // change to the running session and persisting it through the controller.
            if let Some(mutation) = settings_mutation {
                self.apply_settings_mutation(mutation, &mut ui_state, now);
            }

            // Reconcile the text-field draft with the focused row (seed on entry, commit on
            // leave) and run the debounced auto-save. Both run every iteration so the 80ms tick
            // drives the debounce even when no key was pressed.
            self.reconcile_settings_text_field(&mut ui_state, now);
            self.maybe_autosave_settings_text(&mut ui_state, now);

            // Apply page refresh — re-fetch the current URL using the same load pipeline.
            if let Some(url) = reload_url {
                load_state = self.start_load(url);
            }

            // Apply the requested navigation — a Tab+Enter activation, a mouse click on a
            // link, or a form submission.
            match navigation_request {
                Some(NavigationRequest::LoadUrl(link_url)) => {
                    load_state = self.start_link_load(
                        link_url,
                        &working_dir,
                        &mut ui_state,
                        &mut cache,
                        &mut scroll,
                        max_offset,
                        now,
                    );
                }
                Some(NavigationRequest::Submit(submit_button)) => {
                    ui_state.exit_interactive_navigation();
                    load_state = self.start_submit_load(submit_button);
                }
                None => {}
            }
        }
    }

    /// Carries out a history-list key action outside the borrow on `load_state`: delete
    /// removes the selected entry, activate opens it and returns the load to track.
    #[allow(clippy::too_many_arguments)]
    async fn apply_history_key_action(
        &mut self,
        action: InputAction,
        ui_state: &mut UiState,
        working_dir: &std::path::Path,
        cache: &mut Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> Option<LoadState> {
        match action {
            InputAction::HistoryDeleteSelected => {
                self.delete_selected_history(ui_state, now).await;
                None
            }
            InputAction::HistoryActivateSelected => self.activate_selected_history(
                ui_state,
                working_dir,
                cache,
                scroll,
                max_offset,
                now,
            ),
            _ => None,
        }
    }

    /// Applies a settings-panel mutation to the focused row: a checkbox toggle or a radio
    /// cycle. The change takes effect on the running session at once and is persisted through
    /// the controller. A row a `PUMA_*` environment variable fixes refuses the change and
    /// explains why, leaving both the live value and the rendered row unchanged.
    fn apply_settings_mutation(
        &mut self,
        mutation: SettingsMutation,
        ui_state: &mut UiState,
        now: Instant,
    ) {
        let env_overridden = match ui_state.focused_settings_row() {
            Some(row) => row.env_overridden,
            None => return,
        };
        if env_overridden {
            ui_state.set_transient_message(SETTINGS_ENV_LOCKED.to_string(), now);
            return;
        }
        match mutation {
            SettingsMutation::Toggle => self.apply_settings_toggle(ui_state, now),
            SettingsMutation::CyclePrev => {
                self.apply_settings_cycle(ui_state, CycleDirection::Prev, now)
            }
            SettingsMutation::CycleNext => {
                self.apply_settings_cycle(ui_state, CycleDirection::Next, now)
            }
        }
    }

    /// Toggles the focused checkbox: flips its live setting, updates the rendered row, and
    /// persists the new value under its config key. A no-op when the focused row is not a
    /// checkbox, so a toggle key on a radio or text row does nothing.
    fn apply_settings_toggle(&mut self, ui_state: &mut UiState, now: Instant) {
        let Some((id, checked)) = ui_state.toggle_focused_checkbox() else {
            return;
        };
        self.apply_checkbox_setting(id, checked, ui_state);
        let Some(key) = checkbox_config_key(id) else {
            return;
        };
        let encoded = if checked { "true" } else { "false" };
        if self.controller.persist_setting(key, encoded).is_err() {
            ui_state.set_transient_message(SETTINGS_SAVE_FAILED.to_string(), now);
        }
    }

    /// Writes a checkbox's new value into the matching live setting. The search-enabled flag is
    /// mirrored into the UI state so the palette filter drops or restores `/search` at once.
    fn apply_checkbox_setting(&mut self, id: SettingId, checked: bool, ui_state: &mut UiState) {
        match id {
            SettingId::CopyOnSelect => self.settings.copy_on_select = checked,
            SettingId::ForceOsc52 => self.settings.force_osc52 = checked,
            SettingId::SearchEnabled => {
                self.settings.search_enabled = checked;
                ui_state.set_search_enabled(checked);
            }
            SettingId::UnwrapTracking => self.settings.unwrap_tracking = checked,
            SettingId::CookiesFirstParty
            | SettingId::CookiesThirdParty
            | SettingId::SearchBaseUrl
            | SettingId::SearchQueryParameter => {}
        }
    }

    /// Cycles the focused radio group's selection, applying the chosen cookie policy to the
    /// running session and persisting it. The controller prunes the jar when the first-party
    /// default tightens to reject. A no-op when the focused row is not a cookie radio.
    fn apply_settings_cycle(
        &mut self,
        ui_state: &mut UiState,
        direction: CycleDirection,
        now: Instant,
    ) {
        let Some((id, policy)) = ui_state.cycle_focused_radio(direction) else {
            return;
        };
        let Some(scope) = cookie_scope(id) else {
            return;
        };
        if self
            .controller
            .set_global_cookie_policy(scope, policy)
            .is_err()
        {
            ui_state.set_transient_message(SETTINGS_SAVE_FAILED.to_string(), now);
        }
    }

    /// Keeps the active text-field draft in step with the focused row. Focusing a text field
    /// seeds its draft from the shown value; leaving a text field, or moving straight to another
    /// one, commits the field being left before the new one is seeded. Anything outside a text
    /// row (a control row or a closed panel) leaves no draft behind.
    fn reconcile_settings_text_field(&mut self, ui_state: &mut UiState, now: Instant) {
        if !ui_state.is_in_settings_mode() {
            ui_state.clear_settings_text_edit();
            return;
        }
        let focused = ui_state.focused_settings_text_id();
        let editing = ui_state.settings_text_edit_id();
        match (focused, editing) {
            (Some(focused_id), Some(editing_id)) if focused_id == editing_id => {}
            (Some(focused_id), Some(_)) => {
                self.commit_settings_text_field(ui_state, now);
                ui_state.begin_settings_text_edit(focused_id, now);
            }
            (Some(focused_id), None) => ui_state.begin_settings_text_edit(focused_id, now),
            (None, Some(_)) => self.commit_settings_text_field(ui_state, now),
            (None, None) => {}
        }
    }

    /// Saves the field being left when its draft is dirty, then drops the draft. A rejected save
    /// leaves the stored value untouched, so dropping the draft reverts the row to the last saved
    /// value rather than keeping an invalid edit that focus has already moved away from.
    fn commit_settings_text_field(&mut self, ui_state: &mut UiState, now: Instant) {
        if let Some(save) = ui_state.settings_text_pending_save() {
            self.persist_settings_text(save, ui_state, now);
        }
        ui_state.clear_settings_text_edit();
    }

    /// Runs the debounced auto-save: when the focused field has been idle long enough, its draft
    /// is validated, applied, and persisted. The draft stays in place so editing continues.
    fn maybe_autosave_settings_text(&mut self, ui_state: &mut UiState, now: Instant) {
        let Some(save) = ui_state.settings_text_due_save(now) else {
            return;
        };
        self.persist_settings_text(save, ui_state, now);
    }

    /// Validates and persists a settings text field through the controller. The search engine is
    /// validated by [`NavigationController::set_search_engine`], which rejects a `file://` or
    /// otherwise malformed base URL, so an invalid value is never applied or stored. On success
    /// the stored values are reflected back onto the rows and the field is marked clean; on
    /// rejection the field keeps its draft and shows an inline note.
    fn persist_settings_text(
        &mut self,
        save: SettingsTextSave,
        ui_state: &mut UiState,
        now: Instant,
    ) {
        let (base_url, query_parameter) = match save.id {
            SettingId::SearchBaseUrl => (
                save.value,
                self.controller
                    .search_engine()
                    .query_parameter()
                    .to_string(),
            ),
            SettingId::SearchQueryParameter => (
                self.controller.search_engine().base_url().to_string(),
                save.value,
            ),
            SettingId::CookiesFirstParty
            | SettingId::CookiesThirdParty
            | SettingId::CopyOnSelect
            | SettingId::ForceOsc52
            | SettingId::SearchEnabled
            | SettingId::UnwrapTracking => return,
        };
        if self
            .controller
            .set_search_engine(base_url, query_parameter)
            .is_err()
        {
            ui_state.mark_settings_text_save_failed(SETTINGS_TEXT_INVALID.to_string());
            return;
        }
        let engine = self.controller.search_engine();
        let saved_base_url = engine.base_url().to_string();
        let saved_query_parameter = engine.query_parameter().to_string();
        ui_state.mark_settings_text_saved(&saved_base_url, &saved_query_parameter);
        ui_state.set_transient_message(SETTINGS_TEXT_SAVED.to_string(), now);
    }

    /// Routes a submitted command-bar buffer. A buffer whose trimmed text starts with `/`
    /// is a slash command; anything else is a URL and takes the unchanged address path.
    #[allow(clippy::too_many_arguments)]
    fn submit_command_input(
        &mut self,
        input: String,
        working_dir: &std::path::Path,
        ui_state: &mut UiState,
        cache: &mut Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> CommandOutcome {
        if !input.trim_start().starts_with('/') {
            return self.submit_address(
                &input,
                working_dir,
                ui_state,
                cache,
                scroll,
                max_offset,
                now,
            );
        }
        self.dispatch_command(
            &input,
            working_dir,
            ui_state,
            cache,
            scroll,
            max_offset,
            now,
        )
    }

    /// Routes a typed address through the navigation classifier: a fragment on the current
    /// page jumps within it, and anything else loads through the same scheme validation as
    /// before. An unresolvable address shows an error echoing only the typed input, which
    /// is local, never remote content.
    #[allow(clippy::too_many_arguments)]
    fn submit_address(
        &mut self,
        input: &str,
        working_dir: &std::path::Path,
        ui_state: &mut UiState,
        cache: &mut Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> CommandOutcome {
        match browser_core::classify_navigation(
            self.controller.current_url(),
            input,
            working_dir,
            self.tracking_unwrap_mode(),
        ) {
            Err(_) => {
                self.view_state = ViewState::Error(format!("Not a valid address: {input}"));
                *cache = None;
                CommandOutcome::None
            }
            Ok(target) => CommandOutcome::Load(
                self.apply_navigation_target(target, ui_state, cache, scroll, max_offset, now),
            ),
        }
    }

    /// Carries out a classified navigation: a same-page fragment jumps within the current
    /// buffer with no request, and a fetch starts the load while remembering the fragment
    /// to honor once the new page renders.
    #[allow(clippy::too_many_arguments)]
    fn apply_navigation_target(
        &mut self,
        target: NavigationTarget,
        ui_state: &mut UiState,
        cache: &mut Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> LoadState {
        match target {
            NavigationTarget::SamePageAnchor { fragment } => {
                self.jump_to_anchor(
                    fragment.as_deref(),
                    cache,
                    scroll,
                    max_offset,
                    ui_state,
                    now,
                );
                LoadState::Idle
            }
            NavigationTarget::Fetch { url, fragment } => {
                ui_state.set_pending_fragment(fragment);
                self.start_load(url)
            }
        }
    }

    /// Moves the viewport to the fragment's anchor in the current buffer, or reports that
    /// the anchor was not found without moving. The reported name is control-stripped so a
    /// crafted fragment cannot place an escape sequence in the status line.
    #[allow(clippy::too_many_arguments)]
    fn jump_to_anchor(
        &self,
        fragment: Option<&str>,
        cache: &Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        ui_state: &mut UiState,
        now: Instant,
    ) {
        let Some(cached) = cache.as_ref() else {
            return;
        };
        if let Some(row) = resolve_anchor_row(fragment, cached.buffer.anchors()) {
            ui_state.push_anchor_return(scroll.offset());
            scroll.scroll_to(row, max_offset);
            return;
        }
        let name = sanitize_fragment_for_display(&decode_fragment(fragment));
        ui_state.set_transient_message(format!("anchor not found: {name}"), now);
    }

    /// Applies a fragment remembered from a completed load, positioning the viewport on its
    /// anchor once the new page has rendered. A fragment that matches nothing leaves the
    /// viewport at the top. Consumes the pending fragment so it applies exactly once.
    fn apply_pending_fragment(
        &self,
        ui_state: &mut UiState,
        cache: &Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
    ) {
        if !ui_state.has_pending_fragment() {
            return;
        }
        let Some(cached) = cache.as_ref() else {
            return;
        };
        let fragment = ui_state.take_pending_fragment();
        if let Some(row) = resolve_anchor_row(fragment.as_deref(), cached.buffer.anchors()) {
            scroll.scroll_to(row, max_offset);
        }
    }

    /// Parses a slash-command buffer and runs the matching handler. An empty token (the
    /// buffer was just `/`) re-opens command mode without loading anything; an unknown
    /// token shows a transient message and never falls through to a URL load.
    #[allow(clippy::too_many_arguments)]
    fn dispatch_command(
        &mut self,
        input: &str,
        working_dir: &std::path::Path,
        ui_state: &mut UiState,
        cache: &mut Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> CommandOutcome {
        let (token, remainder) = parse_command_input(input);
        if token.is_empty() {
            ui_state.enter_command_mode('/');
            return CommandOutcome::None;
        }
        let Some(spec) = command::resolve(token) else {
            ui_state.set_transient_message(format!("unknown command: /{token}"), now);
            return CommandOutcome::None;
        };
        match spec.kind {
            CommandKind::Open => self.run_open(
                remainder,
                working_dir,
                cache,
                ui_state,
                scroll,
                max_offset,
                now,
            ),
            CommandKind::Search => self.run_search(remainder, ui_state, now),
            CommandKind::Reload => self.run_reload(ui_state, now),
            CommandKind::Back => self.run_back(ui_state, cache, scroll, max_offset, now),
            CommandKind::History => CommandOutcome::History(parse_history_request(remainder)),
            CommandKind::Cookies => {
                self.run_cookies(parse_cookies_request(remainder), ui_state, now);
                CommandOutcome::None
            }
            CommandKind::Quit => CommandOutcome::Quit,
            CommandKind::Help => {
                ui_state.enter_command_mode('/');
                CommandOutcome::None
            }
            CommandKind::Settings => {
                let model = build_settings_model(
                    &self.settings,
                    self.controller.cookie_policy(),
                    self.controller.search_engine(),
                );
                ui_state.enter_settings_mode(model);
                CommandOutcome::None
            }
        }
    }

    /// `/open <url>`: loads the argument through the address path, or shows a usage
    /// message when no URL was given.
    #[allow(clippy::too_many_arguments)]
    fn run_open(
        &mut self,
        remainder: &str,
        working_dir: &std::path::Path,
        cache: &mut Option<CachedPage>,
        ui_state: &mut UiState,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> CommandOutcome {
        if remainder.is_empty() {
            ui_state.set_transient_message("usage: /open <url>".to_string(), now);
            return CommandOutcome::None;
        }
        self.submit_address(
            remainder,
            working_dir,
            ui_state,
            cache,
            scroll,
            max_offset,
            now,
        )
    }

    /// `/search <query>`: turns the query into a results URL and loads it through the
    /// shared load path. Rejected with a fixed message when search is disabled, and shows a
    /// usage message when no query was given. The query and any build error are never echoed
    /// into terminal output; only the fixed local messages below can appear there.
    fn run_search(
        &mut self,
        remainder: &str,
        ui_state: &mut UiState,
        now: Instant,
    ) -> CommandOutcome {
        if !self.settings.search_enabled {
            ui_state.set_transient_message("search is disabled".to_string(), now);
            return CommandOutcome::None;
        }
        let query = remainder.trim();
        if query.is_empty() {
            ui_state.set_transient_message("usage: /search <query>".to_string(), now);
            return CommandOutcome::None;
        }
        match self.controller.search_engine().result_url(query) {
            Ok(url) => CommandOutcome::Load(self.start_load(url)),
            Err(_) => {
                ui_state.set_transient_message("could not build search URL".to_string(), now);
                CommandOutcome::None
            }
        }
    }

    /// `/reload`: re-fetches the current URL through the shared load path, or reports that
    /// there is nothing to reload on a blank page.
    fn run_reload(&mut self, ui_state: &mut UiState, now: Instant) -> CommandOutcome {
        let Some(url) = self.controller.current_url().cloned() else {
            ui_state.set_transient_message("nothing to reload".to_string(), now);
            return CommandOutcome::None;
        };
        CommandOutcome::Load(self.start_load(url))
    }

    /// `/back`: restores the previous page from history, or reports that there is no page
    /// to go back to.
    fn run_back(
        &mut self,
        ui_state: &mut UiState,
        cache: &mut Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> CommandOutcome {
        if !self.controller.can_go_back() && !ui_state.has_anchor_return() {
            ui_state.set_transient_message("no page to go back to".to_string(), now);
            return CommandOutcome::None;
        }
        navigate_back(
            &mut self.controller,
            ui_state,
            cache,
            scroll,
            &mut self.view_state,
            max_offset,
        );
        CommandOutcome::None
    }

    /// Runs a parsed `/cookies` request against core. The summary and the accepted/rejected
    /// listings open the read-only inspection popup; `clear` empties the session jar;
    /// `allow-session`/`reject` set a per-site exception. Every outcome is a short status
    /// message or a sanitized popup; no cookie value and no error internals are ever shown.
    fn run_cookies(&mut self, request: CookiesRequest, ui_state: &mut UiState, now: Instant) {
        match request {
            CookiesRequest::Summary => self.show_cookie_summary(ui_state, now),
            CookiesRequest::Accepted => self.show_cookie_decision(CookieFilter::Accepted, ui_state),
            CookiesRequest::Rejected => self.show_cookie_decision(CookieFilter::Rejected, ui_state),
            CookiesRequest::Clear => {
                self.controller.clear_cookies();
                ui_state.set_transient_message("cookies cleared".to_string(), now);
            }
            CookiesRequest::AllowSession(site) => {
                self.set_cookie_policy(&site, CookiePolicy::Session, ui_state, now)
            }
            CookiesRequest::Reject(site) => {
                self.set_cookie_policy(&site, CookiePolicy::Reject, ui_state, now)
            }
            CookiesRequest::Usage => {
                ui_state.set_transient_message(COOKIES_USAGE.to_string(), now);
            }
        }
    }

    /// Opens the `/cookies` summary popup, or reports an empty session when nothing has been
    /// offered yet so the user is not shown a bare zero-count box.
    fn show_cookie_summary(&mut self, ui_state: &mut UiState, now: Instant) {
        let records = self.controller.cookie_records();
        if records.is_empty() {
            ui_state.set_transient_message("no cookies recorded this session".to_string(), now);
            return;
        }
        let lines = cookie_view::summary_lines(records);
        ui_state.enter_cookies_mode(lines);
    }

    /// Opens the accepted or rejected cookie listing in the inspection popup. The listing
    /// always carries a header line, so it opens even when the filter matches nothing.
    fn show_cookie_decision(&mut self, filter: CookieFilter, ui_state: &mut UiState) {
        let lines = cookie_view::decision_lines(self.controller.cookie_records(), filter);
        ui_state.enter_cookies_mode(lines);
    }

    /// Sets a per-site cookie policy exception, confirming it or mapping a store failure to a
    /// short status message. The site is control-stripped before it appears in the
    /// confirmation, and a failure surfaces only the safe user message, never internals.
    fn set_cookie_policy(
        &mut self,
        site: &str,
        policy: CookiePolicy,
        ui_state: &mut UiState,
        now: Instant,
    ) {
        let message = match self.controller.set_site_cookie_policy(site, policy) {
            Ok(()) => format!("{}: {}", strip_control(site), policy_confirmation(policy)),
            Err(error) => TerminalError::from(error).user_message(),
        };
        ui_state.set_transient_message(message, now);
    }

    /// Recomputes the address-bar suggestions for the current command buffer.
    ///
    /// Suggestions appear only while an address (non-slash) is being typed: a slash buffer
    /// drives the palette instead, and an empty or non-command buffer clears the list. The
    /// index read is synchronous, so this runs inline after each buffer edit.
    fn refresh_address_suggestions(&self, ui_state: &mut UiState) {
        if !ui_state.is_in_command_mode() || ui_state.is_palette_active() {
            ui_state.clear_address_suggestions();
            return;
        }
        let buffer = ui_state.command_buffer().trim();
        if buffer.is_empty() {
            ui_state.clear_address_suggestions();
            return;
        }
        let urls = self
            .controller
            .suggest(buffer, ADDRESS_SUGGESTION_LIMIT)
            .into_iter()
            .map(|entry| entry.url().to_string())
            .collect();
        ui_state.set_address_suggestions(urls);
    }

    /// Runs a parsed `/history` request: opening the list, searching, or clearing. Every
    /// outcome is a short, safe status message; a store failure never surfaces raw detail.
    async fn handle_history_request(
        &mut self,
        request: HistoryRequest,
        ui_state: &mut UiState,
        now: Instant,
    ) {
        match request {
            HistoryRequest::List => self.open_history_list(ui_state, now).await,
            HistoryRequest::Search(query) => self.open_history_search(&query, ui_state, now).await,
            HistoryRequest::ClearAll => self.clear_all_history(ui_state, now).await,
            HistoryRequest::ClearSite(host) => {
                self.clear_history_for_site(&host, ui_state, now).await
            }
        }
    }

    /// Loads recent history into the list view, or reports an empty history.
    async fn open_history_list(&mut self, ui_state: &mut UiState, now: Instant) {
        match self.controller.recent_history(HISTORY_QUERY_LIMIT).await {
            Ok(entries) => self.show_history_entries(entries, ui_state, now),
            Err(_) => ui_state.set_transient_message("could not read history".to_string(), now),
        }
    }

    /// Searches history for `query` and shows the matches in the list view.
    async fn open_history_search(&mut self, query: &str, ui_state: &mut UiState, now: Instant) {
        match self
            .controller
            .search_history(query, HISTORY_QUERY_LIMIT)
            .await
        {
            Ok(entries) => self.show_history_entries(entries, ui_state, now),
            Err(_) => ui_state.set_transient_message("could not search history".to_string(), now),
        }
    }

    /// Opens the history list on `entries`, or shows a short notice when there are none.
    fn show_history_entries(
        &self,
        entries: Vec<browser_core::HistoryEntry>,
        ui_state: &mut UiState,
        now: Instant,
    ) {
        if entries.is_empty() {
            ui_state.set_transient_message("no matching history".to_string(), now);
            return;
        }
        ui_state.enter_history_mode(entries);
        ui_state.set_transient_message(HISTORY_CONTROLS_HINT.to_string(), now);
    }

    /// Clears all history, reporting the outcome as a short status message.
    async fn clear_all_history(&mut self, ui_state: &mut UiState, now: Instant) {
        match self.controller.clear_history().await {
            Ok(()) => ui_state.set_transient_message("history cleared".to_string(), now),
            Err(_) => ui_state.set_transient_message("could not clear history".to_string(), now),
        }
    }

    /// Clears history for a single host, reporting the outcome as a short status message.
    async fn clear_history_for_site(&mut self, host: &str, ui_state: &mut UiState, now: Instant) {
        match self.controller.clear_history_site(host).await {
            Ok(()) => ui_state.set_transient_message("site history cleared".to_string(), now),
            Err(_) => ui_state.set_transient_message("could not clear history".to_string(), now),
        }
    }

    /// Removes the highlighted history entry through the store, then drops it from the list.
    ///
    /// A store failure leaves the list unchanged and shows a short message; the raw error is
    /// never surfaced.
    async fn delete_selected_history(&mut self, ui_state: &mut UiState, now: Instant) {
        let Some(id) = ui_state
            .selected_history_entry()
            .map(|entry| HistoryEntryId::new(entry.id()))
        else {
            return;
        };
        match self.controller.remove_history_entry(id).await {
            Ok(()) => ui_state.remove_selected_history_entry(),
            Err(_) => ui_state.set_transient_message("could not delete entry".to_string(), now),
        }
    }

    /// Opens the highlighted history entry by routing its URL through the address path, so
    /// classification and recording match a typed navigation. Returns the load to track, or
    /// `None` when nothing is selected or the URL does not resolve.
    #[allow(clippy::too_many_arguments)]
    fn activate_selected_history(
        &mut self,
        ui_state: &mut UiState,
        working_dir: &std::path::Path,
        cache: &mut Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> Option<LoadState> {
        let url = ui_state.selected_history_entry()?.url().to_string();
        ui_state.exit_history_mode();
        match self.submit_address(&url, working_dir, ui_state, cache, scroll, max_offset, now) {
            CommandOutcome::Load(state) => Some(state),
            _ => None,
        }
    }

    /// Marks a link URL visited, leaves link navigation, and starts loading it through
    /// the shared load path after scheme validation. On an unresolvable URL it shows a
    /// generic error rather than echoing the remote URL, so a malicious href can never
    /// place raw bytes in terminal output.
    #[allow(clippy::too_many_arguments)]
    fn start_link_load(
        &mut self,
        link_url: String,
        working_dir: &std::path::Path,
        ui_state: &mut UiState,
        cache: &mut Option<CachedPage>,
        scroll: &mut ScrollState,
        max_offset: u16,
        now: Instant,
    ) -> LoadState {
        ui_state.mark_visited(&link_url);
        ui_state.exit_interactive_navigation();
        match browser_core::classify_navigation(
            self.controller.current_url(),
            &link_url,
            working_dir,
            self.tracking_unwrap_mode(),
        ) {
            Err(_) => {
                self.view_state = ViewState::Error("Cannot open this link".to_string());
                *cache = None;
                LoadState::Idle
            }
            Ok(target) => {
                self.apply_navigation_target(target, ui_state, cache, scroll, max_offset, now)
            }
        }
    }

    /// Lays the page out again only when there is a page and the content width changed.
    fn refresh_page_cache(
        &self,
        cache: &mut Option<CachedPage>,
        width: u16,
    ) -> Result<(), TerminalError> {
        if !matches!(self.view_state, ViewState::Page) {
            return Ok(());
        }
        if cache.as_ref().is_some_and(|cached| cached.width == width) {
            return Ok(());
        }
        let buffer = render_page(&self.controller, width)?;
        *cache = Some(CachedPage { width, buffer });
        Ok(())
    }

    /// Spawns the fetch/parse pipeline for `url` on a background task, moving the
    /// controller into the task and returning the active load state that carries it back
    /// on completion. Shared by command submission, page refresh, and link activation so
    /// every navigation uses one load path.
    fn start_load(&mut self, url: BrowserUrl) -> LoadState {
        let (progress_tx, progress_rx) = watch::channel(0usize);
        let loading_url = url.to_string();
        let mut taken = std::mem::take(&mut self.controller);
        let handle = tokio::spawn(async move {
            let result = taken
                .load_with_progress(url, progress_tx, NavigationSource::AddressBar)
                .await;
            (taken, result)
        });
        LoadState::Active {
            handle,
            progress_rx,
            spinner_frame: 0,
            loading_url,
        }
    }

    /// Spawns the submission pipeline for the form activated at `submit_button` on a
    /// background task, mirroring [`start_load`](Self::start_load)'s relationship to
    /// `load_with_progress`. The status label falls back to the current URL while the
    /// request is in flight, since the destination is not known to the caller yet.
    fn start_submit_load(&mut self, submit_button: NodeId) -> LoadState {
        let (progress_tx, progress_rx) = watch::channel(0usize);
        let loading_url = self
            .controller
            .current_url()
            .map(ToString::to_string)
            .unwrap_or_default();
        let mut taken = std::mem::take(&mut self.controller);
        let handle = tokio::spawn(async move {
            let result = taken.submit_with_progress(submit_button, progress_tx).await;
            (taken, result)
        });
        LoadState::Active {
            handle,
            progress_rx,
            spinner_frame: 0,
            loading_url,
        }
    }

    fn status_label(&self) -> String {
        match &self.view_state {
            ViewState::Blank => "blank".to_string(),
            ViewState::Error(_) => "error".to_string(),
            ViewState::Page => self.page_label(),
        }
    }

    fn page_label(&self) -> String {
        if let Some(title) = self.controller.current_title() {
            return title.as_str().to_string();
        }
        match self.controller.current_url() {
            Some(url) => url.to_string(),
            None => "page".to_string(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        &self,
        terminal: &mut AppTerminal,
        page: Option<&CellBuffer>,
        scroll_offset: u16,
        ui_state: &UiState,
        label: &str,
        scroll_percent: u16,
        script_count: usize,
        page_byte_count: usize,
        cookie_counts: (usize, usize),
        loading: Option<(usize, &str, usize)>,
        selection_range: Option<(CellPosition, CellPosition)>,
        field_values: Option<&FormFieldValues>,
    ) -> Result<(), TerminalError> {
        terminal
            .draw(|frame| {
                draw_frame(
                    frame,
                    &self.view_state,
                    label,
                    page,
                    scroll_offset,
                    ui_state,
                    scroll_percent,
                    script_count,
                    page_byte_count,
                    cookie_counts,
                    loading,
                    selection_range,
                    field_values,
                )
            })
            .map_err(|_| TerminalError::RenderFailed)?;
        Ok(())
    }
}

/// Parses a `/history` argument into a request. An empty argument lists recent history; a
/// leading `clear` token clears all history, or a single host when one follows; anything
/// else is a search query. The whole remainder is the query so a multi-word search works.
fn parse_history_request(remainder: &str) -> HistoryRequest {
    let trimmed = remainder.trim();
    if trimmed.is_empty() {
        return HistoryRequest::List;
    }
    let (first, rest) = match trimmed.split_once(char::is_whitespace) {
        Some((first, rest)) => (first, rest.trim()),
        None => (trimmed, ""),
    };
    if first != "clear" {
        return HistoryRequest::Search(trimmed.to_string());
    }
    if rest.is_empty() {
        return HistoryRequest::ClearAll;
    }
    HistoryRequest::ClearSite(rest.to_string())
}

/// The confirmation phrase for a per-site policy exception. `Session` and `Reject` are the
/// only policies `/cookies` sets; `Allow` and `Ask` still get a factual phrase so the match
/// stays exhaustive without a catch-all that would hide a new policy.
fn policy_confirmation(policy: CookiePolicy) -> &'static str {
    match policy {
        CookiePolicy::Session => "session cookies allowed",
        CookiePolicy::Reject => "cookies rejected",
        CookiePolicy::Allow => "cookies allowed",
        CookiePolicy::Ask => "cookies set to ask",
    }
}

/// Whether `action` changed the command buffer text and so should trigger a fresh
/// address-suggestion query. Selection moves and dismissals are excluded so they keep the
/// current list and highlight rather than resetting it.
fn action_refreshes_suggestions(action: InputAction) -> bool {
    matches!(
        action,
        InputAction::EnterCommand(_)
            | InputAction::CommandAppend(_)
            | InputAction::CommandDeleteBack
    )
}

fn render_page(controller: &NavigationController, width: u16) -> Result<CellBuffer, TerminalError> {
    if width == 0 {
        return Ok(CellBuffer::new(0, 0));
    }
    let buffer = controller.render(width)?;
    Ok(buffer)
}

/// The buffer row a fragment names, or `None` when the fragment matches no anchor.
///
/// An empty fragment, or `top` compared case-insensitively, is the top of the page. Any
/// other fragment is percent-decoded and matched against anchor names first by exact
/// equality, then case-insensitively. Anchor spans are in ascending row order, so the
/// first match is the earliest anchor of that name on the page.
fn resolve_anchor_row(fragment: Option<&str>, anchors: &[AnchorSpan]) -> Option<u16> {
    let decoded = decode_fragment(fragment);
    if decoded.is_empty() || decoded.eq_ignore_ascii_case("top") {
        return Some(0);
    }
    if let Some(anchor) = anchors.iter().find(|anchor| anchor.name == decoded) {
        return Some(anchor.row);
    }
    anchors
        .iter()
        .find(|anchor| anchor.name.eq_ignore_ascii_case(&decoded))
        .map(|anchor| anchor.row)
}

/// Percent-decode a fragment into the form anchor names are compared against, matching the
/// decoding the HTML layer applied to those names. A missing fragment decodes to empty.
fn decode_fragment(fragment: Option<&str>) -> String {
    let Some(fragment) = fragment else {
        return String::new();
    };
    percent_decode_str(fragment)
        .decode_utf8_lossy()
        .into_owned()
}

/// Strip control characters from a fragment before it appears in a status message, so a
/// crafted `id` or `name` can never carry an escape sequence into terminal output.
fn sanitize_fragment_for_display(fragment: &str) -> String {
    fragment.chars().filter(|c| !c.is_control()).collect()
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(
    frame: &mut Frame,
    view: &ViewState,
    label: &str,
    page: Option<&CellBuffer>,
    scroll_offset: u16,
    ui_state: &UiState,
    scroll_percent: u16,
    script_count: usize,
    page_byte_count: usize,
    cookie_counts: (usize, usize),
    loading: Option<(usize, &str, usize)>,
    selection_range: Option<(CellPosition, CellPosition)>,
    field_values: Option<&FormFieldValues>,
) {
    let terminal_width = frame.area().width;
    let chunks = Layout::vertical([
        Constraint::Min(0),    // content area
        Constraint::Length(1), // separator
        Constraint::Length(1), // command bar
        Constraint::Length(1), // separator
        Constraint::Length(1), // title bar
        Constraint::Length(1), // hints bar
    ])
    .split(frame.area());

    let focused_url = focused_interactive_link_span(ui_state, page);
    let link_context = LinkRenderContext {
        focused_url,
        ui_state,
    };
    if ui_state.is_in_settings_mode() {
        draw_settings(frame, chunks[0], ui_state);
    } else {
        draw_body(
            frame,
            view,
            page,
            chunks[0],
            scroll_offset,
            &link_context,
            selection_range,
        );
        draw_palette_popup(frame, chunks[0], ui_state);
        draw_address_suggestions_popup(frame, chunks[0], ui_state);
        draw_history_popup(frame, chunks[0], ui_state);
        draw_cookies_popup(frame, chunks[0], ui_state);
        draw_citation_preview_popup(frame, chunks[0], ui_state);
        draw_field_text_edit(frame, chunks[0], ui_state, page, scroll_offset);
        draw_field_multi_select(
            frame,
            chunks[0],
            ui_state,
            page,
            scroll_offset,
            field_values,
        );
        draw_submit_confirmation(frame, chunks[0], ui_state);
    }

    draw_separator(frame, chunks[1]);

    if let Some((spinner_frame, loading_url, bytes_received)) = loading {
        let bar =
            compose_command_bar_loading(spinner_frame, loading_url, bytes_received, terminal_width);
        frame.render_widget(Paragraph::new(bar), chunks[2]);
    } else if ui_state.is_in_command_mode() {
        let cmd_text = compose_command_bar_command(ui_state.command_buffer(), terminal_width);
        frame.render_widget(Paragraph::new(cmd_text), chunks[2]);
        let cursor_x = chunks[2].x
            + command_cursor_col(ui_state.command_buffer(), ui_state.cursor_byte_offset());
        let cursor_y = chunks[2].y;
        frame.set_cursor_position((cursor_x, cursor_y));
    } else {
        let cmd_text = compose_command_bar_reading(ui_state.current_hint(), terminal_width);
        let hint_style = Style::default().fg(TerminalColor::DarkGray);
        frame.render_widget(Paragraph::new(cmd_text).style(hint_style), chunks[2]);
    }

    draw_separator(frame, chunks[3]);

    let title_text = compose_title_bar(
        label,
        scroll_percent,
        script_count,
        page_byte_count,
        cookie_counts,
        terminal_width,
    );
    draw_chrome_row(frame, chunks[4], &title_text);

    let hints_text = compose_hints_bar(ui_state.transient_message(), terminal_width);
    frame.render_widget(Paragraph::new(hints_text), chunks[5]);
}

/// Draws the slash-command palette popup over the bottom of the content area, its last
/// row sitting just above the separator and command bar. Renders nothing when the palette
/// is inactive or empty, so a stale list can never linger. The popup shows only local,
/// static command names and descriptions through the normal widget path, never page bytes.
fn draw_palette_popup(frame: &mut Frame, content_area: Rect, ui_state: &UiState) {
    if !ui_state.is_palette_active() {
        return;
    }
    let matches = ui_state.palette_matches();
    if matches.is_empty() || content_area.width == 0 || content_area.height == 0 {
        return;
    }
    let max_rows = MENU_MAX_ROWS.min(content_area.height as usize);
    let menu = compose_palette_menu(
        matches,
        ui_state.palette_selected(),
        content_area.width,
        max_rows,
    );
    if menu.rows.is_empty() {
        return;
    }
    let lines = palette_popup_lines(&menu);
    render_bottom_popup(frame, content_area, lines);
}

/// Draws the address-bar suggestion list over the bottom of the content area while an
/// address is being typed. Renders nothing when not in command mode or when there are no
/// suggestions, so a stale list never lingers. Each row is a stored URL, control-stripped
/// and fitted through the same popup path the palette uses; no page bytes reach it.
fn draw_address_suggestions_popup(frame: &mut Frame, content_area: Rect, ui_state: &UiState) {
    if !ui_state.is_in_command_mode() {
        return;
    }
    let suggestions = ui_state.address_suggestions();
    if suggestions.is_empty() || content_area.width == 0 || content_area.height == 0 {
        return;
    }
    // URLs from the index are already control-free (they come from a validated `BrowserUrl`
    // whose parser percent-encodes control bytes), but the strip makes that guarantee
    // explicit at the render boundary rather than relying on an upstream invariant.
    let labels: Vec<String> = suggestions.iter().map(|url| strip_control(url)).collect();
    let max_rows = LIST_MENU_MAX_ROWS.min(content_area.height as usize);
    let menu = compose_list_menu(
        &labels,
        ui_state.selected_suggestion(),
        content_area.width,
        max_rows,
    );
    if menu.rows.is_empty() {
        return;
    }
    render_bottom_popup(frame, content_area, list_popup_lines(&menu));
}

/// Draws the history list over the bottom of the content area while the list is open.
/// Renders nothing when the list is closed or empty. Each row is a stored URL and title,
/// control-stripped and fitted, so no raw remote bytes reach the terminal.
fn draw_history_popup(frame: &mut Frame, content_area: Rect, ui_state: &UiState) {
    if !ui_state.is_in_history_mode() {
        return;
    }
    let entries = ui_state.history_entries();
    if entries.is_empty() || content_area.width == 0 || content_area.height == 0 {
        return;
    }
    let now = now_unix_seconds();
    let labels: Vec<String> = entries
        .iter()
        .map(|entry| format_history_label(entry, now))
        .collect();
    let max_rows = LIST_MENU_MAX_ROWS.min(content_area.height as usize);
    let menu = compose_list_menu(
        &labels,
        Some(ui_state.history_selected()),
        content_area.width,
        max_rows,
    );
    if menu.rows.is_empty() {
        return;
    }
    render_bottom_popup(frame, content_area, list_popup_lines(&menu));
}

/// Draws the cookie inspection popup over the bottom of the content area while it is open.
/// Renders nothing when the popup is closed or empty. Every row was sanitized and composed
/// by `cookie_view` before it reached the state, so no raw remote bytes reach the terminal.
fn draw_cookies_popup(frame: &mut Frame, content_area: Rect, ui_state: &UiState) {
    if !ui_state.is_in_cookies_mode() {
        return;
    }
    let lines = ui_state.cookie_lines();
    if lines.is_empty() || content_area.width == 0 || content_area.height == 0 {
        return;
    }
    let max_rows = LIST_MENU_MAX_ROWS.min(content_area.height as usize);
    let menu = compose_list_menu(
        lines,
        Some(ui_state.cookie_selected()),
        content_area.width,
        max_rows,
    );
    if menu.rows.is_empty() {
        return;
    }
    render_bottom_popup(frame, content_area, list_popup_lines(&menu));
}

/// Draws the citation preview popup showing the literal `cite` URL of the currently
/// focused or hovered `<q cite>` span. Renders nothing when no citation is previewed or
/// when a higher-precedence overlay is active. The URL is the already-resolved,
/// already-sanitized `citation_url` string; this never issues a network request.
fn draw_citation_preview_popup(frame: &mut Frame, content_area: Rect, ui_state: &UiState) {
    if !ui_state.citation_preview_visible() {
        return;
    }
    let Some(url) = ui_state.citation_preview() else {
        return;
    };
    if content_area.width == 0 || content_area.height == 0 {
        return;
    }
    let line = Line::raw(strip_control(url));
    render_bottom_popup(frame, content_area, vec![line]);
}

/// The mask character drawn in place of a sensitive field's typed characters while it is
/// being edited, matching the character `browser-layout` uses for a live-typed length.
const FIELD_EDIT_MASK_CHARACTER: char = '•';

/// Draws the active field text-edit draft over its field's row, replacing the static
/// placeholder there with the live draft (masked for a sensitive field) and placing the
/// terminal cursor at the draft's cursor position. Draws nothing when no text-edit
/// sub-mode is active or the field's row is not in the current viewport; the draft text
/// itself is a local editor buffer, not remote content, so no sanitizer pass is needed.
fn draw_field_text_edit(
    frame: &mut Frame,
    content_area: Rect,
    ui_state: &UiState,
    page: Option<&CellBuffer>,
    scroll_offset: u16,
) {
    let Some((node_id, sensitive)) = ui_state.field_text_edit_target() else {
        return;
    };
    let Some((draft, cursor_byte_offset)) = ui_state.field_text_edit_cursor() else {
        return;
    };
    let Some(buffer) = page else {
        return;
    };
    let Some(span) = buffer
        .field_spans()
        .iter()
        .find(|span| span.node_id == node_id)
    else {
        return;
    };
    if span.row < scroll_offset {
        return;
    }
    let row_in_view = span.row - scroll_offset;
    if row_in_view >= content_area.height || span.col_start >= content_area.width {
        return;
    }
    let shown: String = if sensitive {
        std::iter::repeat_n(FIELD_EDIT_MASK_CHARACTER, draft.chars().count()).collect()
    } else {
        draft.to_string()
    };
    let available_width = content_area.width - span.col_start;
    let width = (shown.chars().count() as u16).max(1).min(available_width);
    let area = Rect {
        x: content_area.x + span.col_start,
        y: content_area.y + row_in_view,
        width,
        height: 1,
    };
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(shown).style(Style::default().add_modifier(Modifier::REVERSED)),
        area,
    );
    let cursor_columns = (draft[..cursor_byte_offset].chars().count() as u16).min(width);
    frame.set_cursor_position((area.x + cursor_columns, area.y));
}

/// Draws a multi-select's expanded option list under its field's row, one line per
/// option with a checked/unchecked marker and the moved-to option highlighted. Draws
/// nothing when the sub-mode is closed or the field's row is not in the current
/// viewport. Every label is local, already-parsed option text, not remote content
/// re-fetched here.
fn draw_field_multi_select(
    frame: &mut Frame,
    content_area: Rect,
    ui_state: &UiState,
    page: Option<&CellBuffer>,
    scroll_offset: u16,
    field_values: Option<&FormFieldValues>,
) {
    let Some((node_id, options, cursor)) = ui_state.field_multi_select() else {
        return;
    };
    let Some(buffer) = page else {
        return;
    };
    let Some(span) = buffer
        .field_spans()
        .iter()
        .find(|span| span.node_id == node_id)
    else {
        return;
    };
    if span.row < scroll_offset {
        return;
    }
    let anchor_row = span.row - scroll_offset;
    if anchor_row >= content_area.height || span.col_start >= content_area.width {
        return;
    }
    let selected_values = field_values
        .map(|values| values.selected_values(node_id))
        .unwrap_or(&[]);
    let lines: Vec<Line<'static>> = options
        .iter()
        .enumerate()
        .map(|(index, option)| multi_select_option_line(option, selected_values, index == cursor))
        .collect();
    let available_rows = content_area.height - anchor_row - 1;
    let height = (lines.len() as u16).min(available_rows);
    if height == 0 {
        return;
    }
    let area = Rect {
        x: content_area.x + span.col_start,
        y: content_area.y + anchor_row + 1,
        width: content_area.width - span.col_start,
        height,
    };
    frame.render_widget(Clear, area);
    frame.render_widget(Paragraph::new(lines), area);
}

fn multi_select_option_line(
    option: &SelectOption,
    selected_values: &[String],
    is_cursor_row: bool,
) -> Line<'static> {
    let marker = if selected_values.contains(&option.value) {
        "[x]"
    } else {
        "[ ]"
    };
    let text = format!("{marker} {}", strip_control(&option.label));
    let style = if is_cursor_row {
        Style::default().add_modifier(Modifier::REVERSED)
    } else {
        Style::default()
    };
    Line::styled(text, style)
}

/// Draws the `POST` submission confirmation view over the bottom of the content area,
/// with the exact wording spec §11.2 specifies and the highlighted choice marked with
/// `>`. Draws nothing when the view is closed. `destination` is already a resolved,
/// sanitized action URL; it is control-stripped defensively at this render boundary
/// like every other page-derived string.
fn draw_submit_confirmation(frame: &mut Frame, content_area: Rect, ui_state: &UiState) {
    let Some((_, destination, choice)) = ui_state.submit_confirmation() else {
        return;
    };
    if content_area.width == 0 || content_area.height == 0 {
        return;
    }
    let lines = vec![
        Line::raw("Submit this form?"),
        Line::raw(""),
        Line::raw("Method: POST"),
        Line::raw(format!("Destination: {}", strip_control(destination))),
        Line::raw(""),
        Line::raw(confirmation_choice_line(
            "Submit",
            choice == SubmitChoice::Submit,
        )),
        Line::raw(confirmation_choice_line(
            "Cancel",
            choice == SubmitChoice::Cancel,
        )),
    ];
    render_bottom_popup(frame, content_area, lines);
}

fn confirmation_choice_line(label: &str, is_highlighted: bool) -> String {
    if is_highlighted {
        return format!("> {label}");
    }
    format!("  {label}")
}

/// Renders `lines` as a popup anchored to the bottom of `content_area`, clearing the region
/// first so the page behind it never shows through. Shared by the palette, suggestion, and
/// history popups so all three sit in the same place with the same clearing behavior.
fn render_bottom_popup(frame: &mut Frame, content_area: Rect, lines: Vec<Line<'static>>) {
    let popup_height = lines.len() as u16;
    let popup_area = Rect {
        x: content_area.x,
        y: content_area.y + content_area.height - popup_height,
        width: content_area.width,
        height: popup_height,
    };
    frame.render_widget(Clear, popup_area);
    frame.render_widget(Paragraph::new(lines), popup_area);
}

/// Builds the popup's styled lines: one per visible menu row with the selection
/// highlighted. Every string is local registry text, so no page content reaches the
/// terminal here.
fn palette_popup_lines(menu: &PaletteMenu) -> Vec<Line<'static>> {
    menu.rows
        .iter()
        .enumerate()
        .map(|(row_index, row_text)| {
            let style = palette_row_style(row_index == menu.selected_row);
            Line::styled(row_text.clone(), style)
        })
        .collect()
}

/// Builds styled lines for a suggestion or history popup, highlighting the selected row when
/// there is one. The row strings are already width-fitted and control-stripped by the
/// composer, so this only applies the selection style.
fn list_popup_lines(menu: &ListMenu) -> Vec<Line<'static>> {
    menu.rows
        .iter()
        .enumerate()
        .map(|(row_index, row_text)| {
            let style = palette_row_style(menu.selected_row == Some(row_index));
            Line::styled(row_text.clone(), style)
        })
        .collect()
}

/// Style for a palette row: the selected row is reversed to match the chrome bars; other
/// rows render as plain text over the cleared popup area.
fn palette_row_style(is_selected: bool) -> Style {
    if is_selected {
        return Style::default().add_modifier(Modifier::REVERSED);
    }
    Style::default()
}

/// Renders a reversed-style chrome row (title bar or hints bar).
fn draw_chrome_row(frame: &mut Frame, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(text.to_owned()).style(chrome_row_style()),
        area,
    );
}

/// Renders a horizontal separator line of ─ characters.
fn draw_separator(frame: &mut Frame, area: Rect) {
    let line = "─".repeat(area.width as usize);
    frame.render_widget(Paragraph::new(line), area);
}

fn chrome_row_style() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
}

#[allow(clippy::too_many_arguments)]
fn draw_body(
    frame: &mut Frame,
    view: &ViewState,
    page: Option<&CellBuffer>,
    area: Rect,
    scroll_offset: u16,
    link_context: &LinkRenderContext<'_>,
    selection_range: Option<(CellPosition, CellPosition)>,
) {
    let padded = Rect {
        x: area.x.saturating_add(CONTENT_PADDING),
        y: area.y,
        width: area.width.saturating_sub(CONTENT_PADDING * 2),
        height: area.height,
    };
    match view {
        ViewState::Page => draw_page(
            frame,
            page,
            area,
            padded,
            scroll_offset,
            link_context,
            selection_range,
        ),
        ViewState::Blank => draw_message(frame, padded, BLANK_PLACEHOLDER),
        ViewState::Error(message) => draw_message(frame, padded, message),
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_page(
    frame: &mut Frame,
    page: Option<&CellBuffer>,
    clear_area: Rect,
    blit_area: Rect,
    scroll_offset: u16,
    link_context: &LinkRenderContext<'_>,
    selection_range: Option<(CellPosition, CellPosition)>,
) {
    frame.render_widget(Clear, clear_area);
    let Some(cells) = page else {
        return;
    };
    let buffer = frame.buffer_mut();
    for row in 0..blit_area.height {
        blit_row(
            buffer,
            blit_area,
            cells,
            scroll_offset,
            row,
            link_context,
            selection_range,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_row(
    buffer: &mut Buffer,
    area: Rect,
    cells: &CellBuffer,
    scroll_offset: u16,
    row: u16,
    link_context: &LinkRenderContext<'_>,
    selection_range: Option<(CellPosition, CellPosition)>,
) {
    let source_row = scroll_offset.saturating_add(row);
    for column in 0..area.width {
        copy_cell(
            buffer,
            area,
            cells,
            column,
            row,
            source_row,
            link_context,
            selection_range,
        );
    }
}

/// Copies one sanitized source cell into the target buffer, ignoring positions outside
/// either buffer so a wide grapheme near an edge can never index out of bounds. The link
/// context recolours the cell when it falls inside a focused or visited link span.
#[allow(clippy::too_many_arguments)]
fn copy_cell(
    buffer: &mut Buffer,
    area: Rect,
    cells: &CellBuffer,
    column: u16,
    row: u16,
    source_row: u16,
    link_context: &LinkRenderContext<'_>,
    selection_range: Option<(CellPosition, CellPosition)>,
) {
    let Some(cell) = cells.cell_at(column, source_row) else {
        return;
    };
    let position = (area.x.saturating_add(column), area.y.saturating_add(row));
    let Some(target) = buffer.cell_mut(position) else {
        return;
    };
    target.set_symbol(cell.grapheme());
    let mut style = link_style(cell, column, source_row, cells, link_context);
    if cell_is_selected(column, source_row, selection_range) {
        style = style.add_modifier(Modifier::REVERSED);
    }
    target.set_style(style);
}

/// Whether the cell at `column`, `row` falls inside the linear selection span. Interior
/// rows are fully covered; the first row runs from its start column to the row end and the
/// last row from column zero to its end column, matching how the text is extracted.
fn cell_is_selected(
    column: u16,
    row: u16,
    selection_range: Option<(CellPosition, CellPosition)>,
) -> bool {
    let Some((start, end)) = selection_range else {
        return false;
    };
    if row < start.row || row > end.row {
        return false;
    }
    if start.row == end.row {
        return column >= start.column && column <= end.column;
    }
    if row == start.row {
        return column >= start.column;
    }
    if row == end.row {
        return column <= end.column;
    }
    true
}

/// The link colouring applied on top of the cascaded cell style during blit.
///
/// `focused_url` is the URL of the link the reader has Tab-focused, if any; it is a
/// borrow of the loaded buffer's span data. `ui_state` supplies the session visited set.
/// Only URLs are consulted, never rendered, so no remote text reaches the terminal here.
struct LinkRenderContext<'a> {
    focused_url: Option<&'a str>,
    ui_state: &'a UiState,
}

/// The style for one cell, applying the focus and visited overrides when the cell falls
/// inside a link span. A focused link is cyan, a visited link is dim yellow, and an
/// unvisited, unfocused link keeps the bright colour from the cascade.
fn link_style(
    cell: &Cell,
    column: u16,
    row: u16,
    buffer: &CellBuffer,
    context: &LinkRenderContext<'_>,
) -> Style {
    let base = cell_style(cell);
    let Some(span) = buffer
        .links()
        .iter()
        .find(|&span| span_contains(span, row, column))
    else {
        return base;
    };
    if context.focused_url == Some(span.url.as_str()) {
        return base.fg(TerminalColor::Cyan);
    }
    if context.ui_state.is_visited(&span.url) {
        return base.fg(TerminalColor::Yellow);
    }
    base
}

fn cell_style(cell: &Cell) -> Style {
    let mut style = Style::default();
    if let Some(color) = cell.foreground() {
        style = style.fg(map_color(color));
    }
    if let Some(color) = cell.background() {
        style = style.bg(map_color(color));
    }
    if let Some(modifier) = emphasis_modifier(cell.emphasis()) {
        style = style.add_modifier(modifier);
    }
    if cell.underline() {
        style = style.add_modifier(Modifier::UNDERLINED);
    }
    style
}

fn emphasis_modifier(emphasis: Emphasis) -> Option<Modifier> {
    match emphasis {
        Emphasis::None => None,
        Emphasis::Bold => Some(Modifier::BOLD),
        Emphasis::Italic => Some(Modifier::ITALIC),
    }
}

fn map_color(color: Color) -> TerminalColor {
    match color {
        Color::Black => TerminalColor::Black,
        Color::Red => TerminalColor::Red,
        Color::Green => TerminalColor::Green,
        Color::Yellow => TerminalColor::Yellow,
        Color::Blue => TerminalColor::Blue,
        Color::Magenta => TerminalColor::Magenta,
        Color::Cyan => TerminalColor::Cyan,
        Color::White => TerminalColor::Gray,
        Color::BrightBlack => TerminalColor::DarkGray,
        Color::BrightRed => TerminalColor::LightRed,
        Color::BrightGreen => TerminalColor::LightGreen,
        Color::BrightYellow => TerminalColor::LightYellow,
        Color::BrightBlue => TerminalColor::LightBlue,
        Color::BrightMagenta => TerminalColor::LightMagenta,
        Color::BrightCyan => TerminalColor::LightCyan,
        Color::BrightWhite => TerminalColor::White,
    }
}

fn draw_message(frame: &mut Frame, area: Rect, message: &str) {
    let paragraph = Paragraph::new(message).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

/// The panel's top heading.
const SETTINGS_HEADING: &str = "Settings";

/// The controls legend shown at the foot of the panel, covering every row type: moving the
/// focus, toggling a checkbox, cycling a radio, editing a text field, and leaving.
const SETTINGS_CONTROLS_HINT: &str =
    "↑↓ move · Space toggle · ←→ cycle · type to edit · Esc revert/close";

/// The suffix marking a row whose value an environment variable fixes for the session.
const ENV_OVERRIDE_NOTE: &str = " (set by environment)";

/// The marker shown before a settings text row's inline error after a rejected save.
const SETTINGS_ERROR_MARKER: &str = "⚠";

/// Renders the full-screen settings panel into `area`, listing every section and row with its
/// current value. The focused row is reverse-highlighted and environment-fixed rows are dimmed
/// and marked. Every string is a local, browser-owned value composed here, so no remote content
/// or escape sequence reaches the terminal through this view.
fn draw_settings(frame: &mut Frame, area: Rect, ui_state: &UiState) {
    let Some(model) = ui_state.settings_model() else {
        return;
    };
    let padded = Rect {
        x: area.x.saturating_add(CONTENT_PADDING),
        y: area.y,
        width: area.width.saturating_sub(CONTENT_PADDING * 2),
        height: area.height,
    };
    frame.render_widget(Clear, area);
    let focus = ui_state.settings_focus();
    let draft = ui_state.settings_text_draft();
    let error = ui_state.settings_text_error();
    let lines = settings_lines(model, focus, draft, error);
    frame.render_widget(Paragraph::new(lines), padded);
    // Place the terminal cursor in the focused text field's draft, the same way the command
    // bar shows its cursor. Only an active text edit yields a position, so control rows never
    // display a cursor.
    if let Some((buffer, cursor_byte_offset)) = ui_state.settings_text_cursor() {
        if let Some((cursor_x, cursor_y)) =
            settings_text_cursor_position(model, focus, buffer, cursor_byte_offset, padded)
        {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

/// The terminal cursor position for the focused text field's draft, or `None` when the focused
/// row is not a text input. The column is the width of the row's label prefix plus the width of
/// the draft up to the cursor; the row is the screen line the focused row renders on.
fn settings_text_cursor_position(
    model: &SettingsModel,
    focus: usize,
    buffer: &str,
    cursor_byte_offset: usize,
    area: Rect,
) -> Option<(u16, u16)> {
    let row = model.row_at(focus)?;
    let SettingsControl::TextInput { .. } = &row.control else {
        return None;
    };
    let line_index = focused_row_line_index(model, focus)?;
    let prefix = format!("  {}:  ", row.label);
    let prefix_cols = UnicodeWidthStr::width(prefix.as_str());
    let before_cursor = buffer.get(..cursor_byte_offset).unwrap_or(buffer);
    let cursor_cols = UnicodeWidthStr::width(before_cursor);
    let cursor_x = area.x.saturating_add((prefix_cols + cursor_cols) as u16);
    let cursor_y = area.y.saturating_add(line_index as u16);
    Some((cursor_x, cursor_y))
}

/// The screen line index the focused row renders on, matching the line order [`settings_lines`]
/// builds: a heading line, then for each section a blank line, a title line, and one line per
/// row. `None` when `focus` is past the last row.
fn focused_row_line_index(model: &SettingsModel, focus: usize) -> Option<usize> {
    let mut line_index = 1;
    let mut row_index = 0;
    for section in &model.sections {
        line_index += 2;
        for _ in &section.rows {
            if row_index == focus {
                return Some(line_index);
            }
            line_index += 1;
            row_index += 1;
        }
    }
    None
}

/// Builds the panel's styled lines: a heading, then each section's title and rows, then a
/// controls hint. Rows are numbered in a flat sequence so the focused index highlights the
/// right one regardless of how the sections split them.
fn settings_lines(
    model: &SettingsModel,
    focus: usize,
    draft: Option<&str>,
    error: Option<&str>,
) -> Vec<Line<'static>> {
    let heading_style = Style::default().add_modifier(Modifier::BOLD);
    let mut lines: Vec<Line<'static>> =
        vec![Line::styled(SETTINGS_HEADING.to_string(), heading_style)];
    let mut row_index = 0;
    for section in &model.sections {
        lines.push(Line::from(String::new()));
        lines.push(Line::styled(section.title.clone(), heading_style));
        for row in &section.rows {
            let is_focused = row_index == focus;
            // The draft and inline error belong to the focused text field only; every other row
            // shows its stored value with no error.
            let row_draft = if is_focused { draft } else { None };
            let row_error = if is_focused { error } else { None };
            lines.push(settings_row_line(row, is_focused, row_draft, row_error));
            row_index += 1;
        }
    }
    lines.push(Line::from(String::new()));
    let hint_style = Style::default().fg(TerminalColor::DarkGray);
    lines.push(Line::styled(SETTINGS_CONTROLS_HINT.to_string(), hint_style));
    lines
}

/// Composes one row's line: an indented label and control state, the draft value when the row
/// is the focused text field, an environment note when the row is fixed, an inline error after a
/// rejected save, and the focus or environment style.
fn settings_row_line(
    row: &SettingsRow,
    is_focused: bool,
    draft: Option<&str>,
    error: Option<&str>,
) -> Line<'static> {
    let mut text = format!("  {}", compose_settings_row_text(row, draft));
    if row.env_overridden {
        text.push_str(ENV_OVERRIDE_NOTE);
    }
    if let Some(message) = error {
        text.push_str(&format!("  {SETTINGS_ERROR_MARKER} {message}"));
    }
    Line::styled(text, settings_row_style(row, is_focused))
}

/// The style for a settings row: focus reverses it, an environment-fixed row is dimmed, and
/// every other row is plain. Focus wins over the environment dim so the highlight stays visible.
fn settings_row_style(row: &SettingsRow, is_focused: bool) -> Style {
    if is_focused {
        return Style::default().add_modifier(Modifier::REVERSED);
    }
    if row.env_overridden {
        return Style::default().fg(TerminalColor::DarkGray);
    }
    Style::default()
}

/// The label-and-value text for a row, without indentation or styling. Text-input values are
/// control-stripped at this render boundary as a defensive measure, even though a configured
/// URL or token carries no control bytes.
fn compose_settings_row_text(row: &SettingsRow, draft: Option<&str>) -> String {
    match &row.control {
        SettingsControl::Checkbox { checked } => {
            let marker = if *checked { "[x]" } else { "[ ]" };
            format!("{marker} {}", row.label)
        }
        SettingsControl::Radio { options } => {
            format!("{}:  {}", row.label, compose_radio_options(options))
        }
        SettingsControl::TextInput { value } => {
            let shown = draft.unwrap_or(value);
            format!("{}:  {}", row.label, strip_control(shown))
        }
    }
}

/// Joins a radio control's options into one line, marking the selected option with a filled
/// bullet and the rest with an empty one.
fn compose_radio_options(options: &[RadioOption]) -> String {
    options
        .iter()
        .map(|option| {
            let marker = if option.selected { "(•)" } else { "( )" };
            format!("{marker} {}", option.label)
        })
        .collect::<Vec<String>>()
        .join("  ")
}

fn apply_scroll(
    action: InputAction,
    scroll: &mut ScrollState,
    viewport_height: u16,
    max_offset: u16,
) {
    match action {
        InputAction::ScrollLineDown => scroll.line_down(max_offset),
        InputAction::ScrollLineUp => scroll.line_up(),
        InputAction::ScrollPageDown => scroll.page_down(viewport_height, max_offset),
        InputAction::ScrollPageUp => scroll.page_up(viewport_height),
        InputAction::ScrollToTop => scroll.move_to_top(),
        InputAction::ScrollToBottom => scroll.move_to_bottom(max_offset),
        InputAction::ArmQuit
        | InputAction::ArmRefresh
        | InputAction::RefreshArmed
        | InputAction::Disarm
        | InputAction::Quit
        | InputAction::EnterCommand(_)
        | InputAction::CommandAppend(_)
        | InputAction::CommandMoveCursorLeft
        | InputAction::CommandMoveCursorRight
        | InputAction::CommandDeleteBack
        | InputAction::CommandCancel
        | InputAction::CommandSubmit
        | InputAction::PaletteSelectPrev
        | InputAction::PaletteSelectNext
        | InputAction::PaletteComplete
        | InputAction::SuggestionSelectPrev
        | InputAction::SuggestionSelectNext
        | InputAction::SuggestionDismiss
        | InputAction::HistorySelectPrev
        | InputAction::HistorySelectNext
        | InputAction::HistoryActivateSelected
        | InputAction::HistoryDeleteSelected
        | InputAction::HistoryClose
        | InputAction::CookiesSelectPrev
        | InputAction::CookiesSelectNext
        | InputAction::CookiesClose
        | InputAction::SettingsSelectPrev
        | InputAction::SettingsSelectNext
        | InputAction::SettingsToggle
        | InputAction::SettingsCyclePrev
        | InputAction::SettingsCycleNext
        | InputAction::SettingsClose
        | InputAction::SettingsTextInput(_)
        | InputAction::SettingsTextDeleteBack
        | InputAction::SettingsTextMoveCursorLeft
        | InputAction::SettingsTextMoveCursorRight
        | InputAction::SettingsTextCancel
        | InputAction::FocusNextInteractive
        | InputAction::FocusPreviousInteractive
        | InputAction::ActivateFocused
        | InputAction::NavigateBack
        | InputAction::FieldTextInput(_)
        | InputAction::FieldTextDeleteBack
        | InputAction::FieldTextMoveCursorLeft
        | InputAction::FieldTextMoveCursorRight
        | InputAction::FieldTextCancel
        | InputAction::FieldTextCommit
        | InputAction::FieldMultiSelectMoveUp
        | InputAction::FieldMultiSelectMoveDown
        | InputAction::FieldMultiSelectToggle
        | InputAction::FieldMultiSelectCommit
        | InputAction::FieldMultiSelectCancel
        | InputAction::SubmitConfirmToggle
        | InputAction::SubmitConfirmActivate
        | InputAction::SubmitConfirmCancel => {}
    }
}

/// Applies a settings text-field editing action to the focused field's draft. Character and
/// cursor keys edit the draft; `Esc` reverts an unsaved draft or, when there is nothing to
/// revert, closes the panel. Every other action is ignored here. The debounced save, the
/// focus-leave commit, and the seed of a newly focused field run in the owning app, which holds
/// the controller; this only touches UI state, so it can run inside the event borrow.
fn apply_settings_text_action(action: InputAction, ui_state: &mut UiState, now: Instant) {
    match action {
        InputAction::SettingsTextInput(character) => ui_state.settings_text_input(character, now),
        InputAction::SettingsTextDeleteBack => ui_state.settings_text_delete_back(now),
        InputAction::SettingsTextMoveCursorLeft => ui_state.settings_text_move_left(),
        InputAction::SettingsTextMoveCursorRight => ui_state.settings_text_move_right(),
        InputAction::SettingsTextCancel => {
            if matches!(
                ui_state.settings_text_cancel(),
                SettingsEscOutcome::ClosePanel
            ) {
                ui_state.exit_settings_mode();
            }
        }
        _ => {}
    }
}

/// Applies a form field's text-edit sub-mode editing action to its draft. Character and
/// cursor keys edit the draft; commit and cancel need the controller to write the value
/// back or discard it, so they are applied by `handle_navigation_action` instead. Every
/// other action is ignored here, mirroring `apply_settings_text_action`.
fn apply_field_text_action(action: InputAction, ui_state: &mut UiState) {
    match action {
        InputAction::FieldTextInput(character) => ui_state.field_text_input(character),
        InputAction::FieldTextDeleteBack => ui_state.field_text_delete_back(),
        InputAction::FieldTextMoveCursorLeft => ui_state.field_text_move_left(),
        InputAction::FieldTextMoveCursorRight => ui_state.field_text_move_right(),
        _ => {}
    }
}

fn apply_command_action(action: InputAction, ui_state: &mut UiState) {
    match action {
        InputAction::EnterCommand(ch) => ui_state.enter_command_mode(ch),
        InputAction::CommandAppend(ch) => ui_state.command_append_char(ch),
        InputAction::CommandMoveCursorLeft => ui_state.command_move_left(),
        InputAction::CommandMoveCursorRight => ui_state.command_move_right(),
        InputAction::CommandDeleteBack => ui_state.command_delete_or_exit(),
        InputAction::CommandCancel => ui_state.cancel_command_mode(),
        InputAction::PaletteSelectPrev => ui_state.palette_select_prev(),
        InputAction::PaletteSelectNext => ui_state.palette_select_next(),
        InputAction::PaletteComplete => ui_state.palette_complete(),
        InputAction::SuggestionSelectPrev => ui_state.suggestion_select_prev(),
        InputAction::SuggestionSelectNext => ui_state.suggestion_select_next(),
        InputAction::SuggestionDismiss => ui_state.clear_address_suggestions(),
        InputAction::HistorySelectPrev => ui_state.history_select_prev(),
        InputAction::HistorySelectNext => ui_state.history_select_next(),
        InputAction::HistoryClose => ui_state.exit_history_mode(),
        InputAction::CookiesSelectPrev => ui_state.cookie_select_prev(),
        InputAction::CookiesSelectNext => ui_state.cookie_select_next(),
        InputAction::CookiesClose => ui_state.exit_cookies_mode(),
        InputAction::SettingsSelectPrev => ui_state.settings_focus_prev(),
        InputAction::SettingsSelectNext => ui_state.settings_focus_next(),
        InputAction::SettingsClose => ui_state.exit_settings_mode(),
        // The settings mutation actions need the controller and live settings, so they are
        // applied by the owning app after the event borrow ends, not here. The settings
        // text-edit actions are applied by `apply_settings_text_action`, which has the keystroke
        // time the debounce needs.
        InputAction::SettingsToggle
        | InputAction::SettingsCyclePrev
        | InputAction::SettingsCycleNext
        | InputAction::SettingsTextInput(_)
        | InputAction::SettingsTextDeleteBack
        | InputAction::SettingsTextMoveCursorLeft
        | InputAction::SettingsTextMoveCursorRight
        | InputAction::SettingsTextCancel
        | InputAction::ScrollLineDown
        | InputAction::ScrollLineUp
        | InputAction::ScrollPageDown
        | InputAction::ScrollPageUp
        | InputAction::ScrollToTop
        | InputAction::ScrollToBottom
        | InputAction::ArmQuit
        | InputAction::Quit
        | InputAction::ArmRefresh
        | InputAction::RefreshArmed
        | InputAction::Disarm
        | InputAction::CommandSubmit
        | InputAction::HistoryActivateSelected
        | InputAction::HistoryDeleteSelected
        | InputAction::FocusNextInteractive
        | InputAction::FocusPreviousInteractive
        | InputAction::ActivateFocused
        | InputAction::NavigateBack
        | InputAction::FieldTextInput(_)
        | InputAction::FieldTextDeleteBack
        | InputAction::FieldTextMoveCursorLeft
        | InputAction::FieldTextMoveCursorRight
        | InputAction::FieldTextCancel
        | InputAction::FieldTextCommit
        | InputAction::FieldMultiSelectMoveUp
        | InputAction::FieldMultiSelectMoveDown
        | InputAction::FieldMultiSelectToggle
        | InputAction::FieldMultiSelectCommit
        | InputAction::FieldMultiSelectCancel
        | InputAction::SubmitConfirmToggle
        | InputAction::SubmitConfirmActivate
        | InputAction::SubmitConfirmCancel => {}
    }
}

/// One focusable target in the unified Tab order: an author-intended link or citation,
/// or a form control's field span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveTarget<'a> {
    Link(&'a str),
    Field(NodeId, FieldSpanKind),
}

/// The unique interactive targets in the order they first appear in the buffer (by row,
/// then column), merging link spans and field spans. A link is deduplicated by URL and a
/// field by its `NodeId`, each keeping its first occurrence's position, mirroring the
/// former link-only `unique_links`. This is the list Tab navigation steps through.
fn unique_interactive_targets(buffer: &CellBuffer) -> Vec<InteractiveTarget<'_>> {
    let mut positioned: Vec<(u16, u16, InteractiveTarget<'_>)> = Vec::new();
    for span in buffer.links() {
        positioned.push((span.row, span.col_start, InteractiveTarget::Link(&span.url)));
    }
    for span in buffer.field_spans() {
        positioned.push((
            span.row,
            span.col_start,
            InteractiveTarget::Field(span.node_id, span.kind),
        ));
    }
    positioned.sort_by_key(|(row, col_start, _)| (*row, *col_start));
    let mut targets: Vec<InteractiveTarget<'_>> = Vec::new();
    for (_, _, target) in positioned {
        if !targets.contains(&target) {
            targets.push(target);
        }
    }
    targets
}

/// The index within [`unique_interactive_targets`] of the first target whose first span
/// starts at or below `scroll_offset`, so Tab focuses a target the reader can actually
/// see. Falls back to the first target when every target is above the viewport.
fn first_visible_interactive_index(buffer: &CellBuffer, scroll_offset: u16) -> usize {
    for (index, target) in unique_interactive_targets(buffer).iter().enumerate() {
        if let Some(row) = target_row(buffer, target) {
            if row >= scroll_offset {
                return index;
            }
        }
    }
    0
}

/// The row of `target`'s first span in `buffer`, or `None` when it names no span there.
fn target_row(buffer: &CellBuffer, target: &InteractiveTarget<'_>) -> Option<u16> {
    match *target {
        InteractiveTarget::Link(url) => buffer
            .links()
            .iter()
            .find(|span| span.url == url)
            .map(|span| span.row),
        InteractiveTarget::Field(node_id, _) => buffer
            .field_spans()
            .iter()
            .find(|span| span.node_id == node_id)
            .map(|span| span.row),
    }
}

/// Whether the cell at `column`, `row` falls within `span`'s extent on its row.
fn span_contains(span: &LinkSpan, row: u16, column: u16) -> bool {
    span.row == row && span.col_start <= column && column <= span.col_end
}

/// The row of the focused target's first span. `None` when the index or a matching span
/// is absent.
fn focused_interactive_row(buffer: &CellBuffer, focused_index: usize) -> Option<u16> {
    let target = unique_interactive_targets(buffer)
        .into_iter()
        .nth(focused_index)?;
    target_row(buffer, &target)
}

/// Scrolls the viewport the minimum needed to reveal the focused target's first row. Does
/// nothing when nothing is focused or its row cannot be resolved.
fn reveal_focused_interactive(
    ui_state: &UiState,
    buffer: &CellBuffer,
    scroll: &mut ScrollState,
    bounds: ViewportBounds,
) {
    let Some(focused_index) = ui_state.focused_interactive_index else {
        return;
    };
    let Some(row) = focused_interactive_row(buffer, focused_index) else {
        return;
    };
    scroll.reveal_row(row, bounds.height, bounds.max_offset);
}

/// Applies an interactive-navigation, field-edit, submission-confirmation, or history
/// action. Returns the navigation the action asks the event loop to run: a link's URL to
/// load, or a form's activated submit button to submit. Every other action mutates UI or
/// controller state directly and returns `None`.
fn handle_navigation_action(
    action: InputAction,
    controller: &mut NavigationController,
    ui_state: &mut UiState,
    cache: &mut Option<CachedPage>,
    scroll: &mut ScrollState,
    view_state: &mut ViewState,
    bounds: ViewportBounds,
) -> Option<NavigationRequest> {
    match action {
        InputAction::FocusNextInteractive => {
            advance_interactive_focus(
                ui_state,
                cache.as_ref().map(|cached| &cached.buffer),
                scroll,
                bounds,
            );
            None
        }
        InputAction::FocusPreviousInteractive => {
            retreat_interactive_focus(
                ui_state,
                cache.as_ref().map(|cached| &cached.buffer),
                scroll,
                bounds,
            );
            None
        }
        InputAction::ActivateFocused => dispatch_activate_focused(
            controller,
            ui_state,
            cache.as_ref().map(|cached| &cached.buffer),
        ),
        InputAction::NavigateBack => {
            navigate_back(
                controller,
                ui_state,
                cache,
                scroll,
                view_state,
                bounds.max_offset,
            );
            None
        }
        InputAction::FieldTextCommit => {
            commit_field_text_edit(controller, ui_state);
            None
        }
        InputAction::FieldTextCancel => {
            ui_state.take_field_text_edit();
            None
        }
        InputAction::FieldMultiSelectMoveUp => {
            ui_state.field_multi_select_move_up();
            None
        }
        InputAction::FieldMultiSelectMoveDown => {
            ui_state.field_multi_select_move_down();
            None
        }
        InputAction::FieldMultiSelectToggle => {
            apply_field_multi_select_toggle(controller, ui_state);
            None
        }
        InputAction::FieldMultiSelectCommit | InputAction::FieldMultiSelectCancel => {
            ui_state.exit_field_multi_select();
            None
        }
        InputAction::SubmitConfirmToggle => {
            ui_state.submit_confirmation_toggle();
            None
        }
        InputAction::SubmitConfirmCancel => {
            ui_state.exit_submit_confirmation();
            None
        }
        InputAction::SubmitConfirmActivate => activate_submit_confirmation(ui_state),
        InputAction::Disarm => {
            if ui_state.is_in_interactive_navigation() {
                ui_state.exit_interactive_navigation();
            }
            None
        }
        InputAction::ScrollLineDown
        | InputAction::ScrollLineUp
        | InputAction::ScrollPageDown
        | InputAction::ScrollPageUp
        | InputAction::ScrollToTop
        | InputAction::ScrollToBottom
        | InputAction::ArmQuit
        | InputAction::Quit
        | InputAction::ArmRefresh
        | InputAction::RefreshArmed
        | InputAction::EnterCommand(_)
        | InputAction::CommandAppend(_)
        | InputAction::CommandMoveCursorLeft
        | InputAction::CommandMoveCursorRight
        | InputAction::CommandDeleteBack
        | InputAction::CommandCancel
        | InputAction::CommandSubmit
        | InputAction::PaletteSelectPrev
        | InputAction::PaletteSelectNext
        | InputAction::PaletteComplete
        | InputAction::SuggestionSelectPrev
        | InputAction::SuggestionSelectNext
        | InputAction::SuggestionDismiss
        | InputAction::HistorySelectPrev
        | InputAction::HistorySelectNext
        | InputAction::HistoryActivateSelected
        | InputAction::HistoryDeleteSelected
        | InputAction::HistoryClose
        | InputAction::CookiesSelectPrev
        | InputAction::CookiesSelectNext
        | InputAction::CookiesClose
        | InputAction::SettingsSelectPrev
        | InputAction::SettingsSelectNext
        | InputAction::SettingsToggle
        | InputAction::SettingsCyclePrev
        | InputAction::SettingsCycleNext
        | InputAction::SettingsClose
        | InputAction::SettingsTextInput(_)
        | InputAction::SettingsTextDeleteBack
        | InputAction::SettingsTextMoveCursorLeft
        | InputAction::SettingsTextMoveCursorRight
        | InputAction::SettingsTextCancel
        | InputAction::FieldTextInput(_)
        | InputAction::FieldTextDeleteBack
        | InputAction::FieldTextMoveCursorLeft
        | InputAction::FieldTextMoveCursorRight => None,
    }
}

/// Dispatches `ActivateFocused` on the focused target's kind: a link returns its URL to
/// load, and a field control applies its own activation behavior (instant-apply,
/// entering an edit sub-mode, or a submission request) and returns `None` unless it is a
/// submit button. `None` when nothing is focused or the buffer is unavailable.
fn dispatch_activate_focused(
    controller: &mut NavigationController,
    ui_state: &mut UiState,
    buffer: Option<&CellBuffer>,
) -> Option<NavigationRequest> {
    let index = ui_state.focused_interactive_index?;
    let buffer = buffer?;
    let target = unique_interactive_targets(buffer).into_iter().nth(index)?;
    match target {
        InteractiveTarget::Link(url) => Some(NavigationRequest::LoadUrl(url.to_string())),
        InteractiveTarget::Field(node_id, FieldSpanKind::Input) => {
            activate_input_field(controller, ui_state, node_id);
            None
        }
        InteractiveTarget::Field(node_id, FieldSpanKind::Select) => {
            activate_select_field(controller, ui_state, node_id);
            None
        }
        InteractiveTarget::Field(node_id, FieldSpanKind::Textarea) => {
            enter_text_edit(controller, ui_state, node_id, false);
            None
        }
        InteractiveTarget::Field(node_id, FieldSpanKind::Button) => {
            activate_button_field(controller, ui_state, node_id)
        }
    }
}

/// Activates a focused `<input>` control by its kind: a checkbox or radio applies
/// instantly through the controller, and every other kind opens the text-edit sub-mode
/// seeded with its current value.
fn activate_input_field(
    controller: &mut NavigationController,
    ui_state: &mut UiState,
    node_id: NodeId,
) {
    let Some(input) = controller.input_element(node_id) else {
        return;
    };
    if input.kind == InputKind::Checkbox {
        let _ = controller.toggle_checkbox(node_id);
        return;
    }
    if input.kind == InputKind::Radio {
        let _ = controller.select_radio(node_id);
        return;
    }
    let sensitive = input.sensitive;
    enter_text_edit(controller, ui_state, node_id, sensitive);
}

/// Seeds and enters the text-edit sub-mode for `node_id` from its current live value.
fn enter_text_edit(
    controller: &NavigationController,
    ui_state: &mut UiState,
    node_id: NodeId,
    sensitive: bool,
) {
    let current_value = controller
        .field_values()
        .and_then(|values| values.text(node_id))
        .unwrap_or("");
    ui_state.enter_field_text_edit(node_id, current_value, sensitive);
}

/// Activates a focused `<select>` control: a single-select cycles to its next option
/// immediately, wrapping at the ends; a multi-select opens the expansion sub-mode
/// snapshotting its current options.
fn activate_select_field(
    controller: &mut NavigationController,
    ui_state: &mut UiState,
    node_id: NodeId,
) {
    let Some(select) = controller.select_element(node_id) else {
        return;
    };
    if select.multiple {
        ui_state.enter_field_multi_select(node_id, select.options.clone());
        return;
    }
    let next_value = next_select_value(select, controller.field_values());
    let _ = controller.set_select_value(node_id, next_value);
}

/// The value of the option after the currently selected one, wrapping at the end, or the
/// first option when nothing is currently selected. Empty when the select has no
/// options.
fn next_select_value(select: &SelectElement, field_values: Option<&FormFieldValues>) -> String {
    let Some(first_option) = select.options.first() else {
        return String::new();
    };
    let current_value = field_values
        .and_then(|values| values.selected_values(select.id).first())
        .map(String::as_str);
    let current_index = current_value.and_then(|value| {
        select
            .options
            .iter()
            .position(|option| option.value == value)
    });
    let next_index = match current_index {
        Some(index) => (index + 1) % select.options.len(),
        None => 0,
    };
    select
        .options
        .get(next_index)
        .unwrap_or(first_option)
        .value
        .clone()
}

/// Activates a focused button: a reset button restores its enclosing form's controls to
/// their parsed defaults, a plain button has no default action, and a submit button
/// asks for immediate submission (`GET`) or opens the confirmation view (`POST`).
fn activate_button_field(
    controller: &mut NavigationController,
    ui_state: &mut UiState,
    node_id: NodeId,
) -> Option<NavigationRequest> {
    let kind = controller.button_element(node_id)?.kind;
    match kind {
        ButtonKind::Reset => {
            let _ = controller.reset_form(node_id);
            None
        }
        ButtonKind::Button => None,
        ButtonKind::Submit => dispatch_submit_activation(controller, ui_state, node_id),
    }
}

/// Resolves the form enclosing `submit_button` and either asks for an immediate
/// submission (`GET`, matching how a plain link activation loads today) or opens the
/// `POST`-only confirmation view.
fn dispatch_submit_activation(
    controller: &NavigationController,
    ui_state: &mut UiState,
    submit_button: NodeId,
) -> Option<NavigationRequest> {
    let (method, destination) = controller.form_submission_target(submit_button)?;
    if method == FormMethod::Get {
        return Some(NavigationRequest::Submit(submit_button));
    }
    ui_state.enter_submit_confirmation(submit_button, destination);
    None
}

/// Toggles the multi-select option at the sub-mode's current cursor through the
/// controller. A no-op when the sub-mode is closed or the cursor names no option.
fn apply_field_multi_select_toggle(controller: &mut NavigationController, ui_state: &UiState) {
    let Some((node_id, options, cursor)) = ui_state.field_multi_select() else {
        return;
    };
    let Some(option) = options.get(cursor) else {
        return;
    };
    let value = option.value.clone();
    let _ = controller.toggle_multi_select_option(node_id, value);
}

/// Writes the field text-edit draft back through the controller and leaves the sub-mode.
/// A no-op when the sub-mode is closed.
fn commit_field_text_edit(controller: &mut NavigationController, ui_state: &mut UiState) {
    let Some((node_id, sensitive, text)) = ui_state.take_field_text_edit() else {
        return;
    };
    if sensitive {
        let _ = controller.set_sensitive_field_text(node_id, text);
        return;
    }
    let _ = controller.set_field_text(node_id, text);
}

/// Resolves the confirmation view's highlighted choice: `Cancel` closes the view with no
/// further action, `Submit` asks for the pending button's submission.
fn activate_submit_confirmation(ui_state: &mut UiState) -> Option<NavigationRequest> {
    let (submit_button, _, choice) = ui_state.submit_confirmation()?;
    ui_state.exit_submit_confirmation();
    if choice == SubmitChoice::Cancel {
        return None;
    }
    Some(NavigationRequest::Submit(submit_button))
}

/// Moves focus to the next interactive target, entering interactive navigation at the
/// first visible target when nothing is focused yet, then scrolls the viewport to keep
/// the focused target visible.
fn advance_interactive_focus(
    ui_state: &mut UiState,
    buffer: Option<&CellBuffer>,
    scroll: &mut ScrollState,
    bounds: ViewportBounds,
) {
    let Some(buffer) = buffer else {
        return;
    };
    let count = unique_interactive_targets(buffer).len();
    if count == 0 {
        return;
    }
    if ui_state.focused_interactive_index.is_none() {
        ui_state
            .enter_interactive_navigation(first_visible_interactive_index(buffer, scroll.offset()));
    } else {
        ui_state.focus_next_interactive(count);
    }
    reveal_focused_interactive(ui_state, buffer, scroll, bounds);
    update_citation_preview(ui_state, Some(buffer));
}

/// Moves focus to the previous interactive target, entering interactive navigation at
/// the first visible target when nothing is focused yet, then scrolls the viewport to
/// keep the focused target visible.
fn retreat_interactive_focus(
    ui_state: &mut UiState,
    buffer: Option<&CellBuffer>,
    scroll: &mut ScrollState,
    bounds: ViewportBounds,
) {
    let Some(buffer) = buffer else {
        return;
    };
    let count = unique_interactive_targets(buffer).len();
    if count == 0 {
        return;
    }
    if ui_state.focused_interactive_index.is_none() {
        ui_state
            .enter_interactive_navigation(first_visible_interactive_index(buffer, scroll.offset()));
    } else {
        ui_state.focus_previous_interactive(count);
    }
    reveal_focused_interactive(ui_state, buffer, scroll, bounds);
    update_citation_preview(ui_state, Some(buffer));
}

/// The borrowed URL of the currently focused target, if it is a link, for the render
/// pass's link-coloring context. `None` when nothing is focused, the focused target is
/// a field, or the buffer is unavailable.
fn focused_interactive_link_span<'a>(
    ui_state: &UiState,
    buffer: Option<&'a CellBuffer>,
) -> Option<&'a str> {
    let index = ui_state.focused_interactive_index?;
    let buffer = buffer?;
    match unique_interactive_targets(buffer).into_iter().nth(index)? {
        InteractiveTarget::Link(url) => Some(url),
        InteractiveTarget::Field(..) => None,
    }
}

/// The URL of the currently focused target, if it is a link and interactive navigation
/// still points at it in the buffer.
fn focused_interactive_url(ui_state: &UiState, buffer: Option<&CellBuffer>) -> Option<String> {
    let index = ui_state.focused_interactive_index?;
    let buffer = buffer?;
    match unique_interactive_targets(buffer).into_iter().nth(index)? {
        InteractiveTarget::Link(url) => Some(url.to_string()),
        InteractiveTarget::Field(..) => None,
    }
}

/// The kind of the currently focused target's link span, if it is a link. `None` when
/// nothing is focused, the focused target is a field, or it no longer resolves in the
/// buffer.
fn focused_interactive_kind(ui_state: &UiState, buffer: Option<&CellBuffer>) -> Option<LinkKind> {
    let index = ui_state.focused_interactive_index?;
    let buffer = buffer?;
    let InteractiveTarget::Link(url) = unique_interactive_targets(buffer).into_iter().nth(index)?
    else {
        return None;
    };
    buffer
        .links()
        .iter()
        .find(|span| span.url == url)
        .map(|span| span.kind)
}

/// Shows the citation preview popup when the focused target is a citation link, or
/// dismisses it otherwise. Called after every change to interactive focus so the popup
/// always tracks it.
fn update_citation_preview(ui_state: &mut UiState, buffer: Option<&CellBuffer>) {
    if focused_interactive_kind(ui_state, buffer) == Some(LinkKind::Citation) {
        let url =
            focused_interactive_url(ui_state, buffer).expect("citation kind implies a focused url");
        ui_state.set_citation_preview(url);
        return;
    }
    ui_state.clear_citation_preview();
}

/// Undoes the most recent same-page anchor jump, or, when none is outstanding, restores
/// the previous page from history and resets the viewport, clearing any focused
/// interactive target.
///
/// A jump within one page is undone before the page is left, so a table of contents is not
/// a one-way trip. Does nothing when there is neither a jump to undo nor history to
/// restore.
fn navigate_back(
    controller: &mut NavigationController,
    ui_state: &mut UiState,
    cache: &mut Option<CachedPage>,
    scroll: &mut ScrollState,
    view_state: &mut ViewState,
    max_offset: u16,
) {
    if let Some(offset) = ui_state.pop_anchor_return() {
        scroll.scroll_to(offset, max_offset);
        return;
    }
    if !controller.go_back() {
        return;
    }
    *view_state = ViewState::Page;
    *cache = None;
    *scroll = ScrollState::new();
    ui_state.clear_anchor_returns();
    ui_state.exit_interactive_navigation();
}

/// Dispatches a left-button mouse gesture over the page body into a text selection.
///
/// A press begins a selection; a drag extends it, clamped to the buffer so dragging into
/// the chrome or past the content never leaves the grid; a release either keeps the
/// highlight (the gesture moved, so it is a selection) or activates a link (the gesture did
/// not move, so it is a click). Wheel and other events are ignored so scrolling is
/// untouched.
#[allow(clippy::too_many_arguments)]
fn handle_mouse_event(
    mouse: MouseEvent,
    buffer: Option<&CellBuffer>,
    scroll_offset: u16,
    body_area_top_row: u16,
    selection: &mut TextSelection,
    navigate_to_url: &mut Option<String>,
    ui_state: &mut UiState,
    now: Instant,
    copy_on_select: bool,
    force_osc52: bool,
) {
    let Some(buffer) = buffer else {
        return;
    };
    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            let Some(anchor) = document_coordinate(&mouse, scroll_offset, body_area_top_row) else {
                return;
            };
            selection.begin(anchor);
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if !selection.is_dragging() {
                return;
            }
            let cursor =
                clamped_document_coordinate(&mouse, buffer, scroll_offset, body_area_top_row);
            selection.update(cursor);
        }
        MouseEventKind::Up(MouseButton::Left) => {
            if selection.has_moved() {
                if copy_on_select {
                    copy_selection(buffer, selection, ui_state, now, force_osc52);
                }
                return;
            }
            activate_link_under_pointer(
                &mouse,
                buffer,
                scroll_offset,
                body_area_top_row,
                navigate_to_url,
            );
            selection.clear();
        }
        MouseEventKind::Moved => {
            update_citation_preview_for_pointer(
                &mouse,
                buffer,
                scroll_offset,
                body_area_top_row,
                ui_state,
            );
        }
        _ => {}
    }
}

/// Copies the highlighted selection to the clipboard on button release. Extracts the
/// selection text, skips a zero-length or whitespace-only selection, and on a successful
/// clipboard write shows the `copied N chars to clipboard` confirmation for five seconds.
/// The highlight is left in place so the user sees what was copied until the next press.
fn copy_selection(
    buffer: &CellBuffer,
    selection: &TextSelection,
    ui_state: &mut UiState,
    now: Instant,
    force_osc52: bool,
) {
    let Some((start, end)) = selection.range() else {
        return;
    };
    let text = buffer.text_in_range(start, end);
    if text.trim().is_empty() {
        return;
    }
    if !clipboard_write_succeeded(copy_to_clipboard(&text, force_osc52)) {
        return;
    }
    ui_state.set_transient_message(copied_message(&text), now);
}

/// Whether a clipboard write reported success through either the native path or OSC 52.
fn clipboard_write_succeeded(outcome: ClipboardOutcome) -> bool {
    matches!(
        outcome,
        ClipboardOutcome::CopiedNative | ClipboardOutcome::CopiedOsc52
    )
}

/// The copy confirmation for `text`: `copied N chars to clipboard`, where `N` counts
/// grapheme clusters so a multi-byte character or a combined emoji counts as one. Carries
/// only the count; the copied text itself never appears in the message.
fn copied_message(text: &str) -> String {
    let grapheme_count = text.graphemes(true).count();
    format!("copied {grapheme_count} chars to clipboard")
}

/// Maps a mouse position to a document coordinate: the screen row maps to a buffer row
/// through the scroll offset and the screen column is shifted back by the content padding.
/// Returns `None` when the pointer is above the body or inside the left padding.
fn document_coordinate(
    mouse: &MouseEvent,
    scroll_offset: u16,
    body_area_top_row: u16,
) -> Option<CellPosition> {
    let row = mouse
        .row
        .checked_sub(body_area_top_row)
        .and_then(|row| row.checked_add(scroll_offset))?;
    let column = mouse.column.checked_sub(CONTENT_PADDING)?;
    Some(CellPosition { column, row })
}

/// Maps a mouse position to a document coordinate clamped to the buffer's last cell, so a
/// drag into the chrome or past the content still yields a valid in-buffer cursor.
fn clamped_document_coordinate(
    mouse: &MouseEvent,
    buffer: &CellBuffer,
    scroll_offset: u16,
    body_area_top_row: u16,
) -> CellPosition {
    let row = mouse
        .row
        .saturating_sub(body_area_top_row)
        .saturating_add(scroll_offset);
    let column = mouse.column.saturating_sub(CONTENT_PADDING);
    let last_row = buffer.height().saturating_sub(1);
    let last_column = buffer.width().saturating_sub(1);
    CellPosition {
        column: column.min(last_column),
        row: row.min(last_row),
    }
}

/// Sets `navigate_to_url` when the pointer rests on a link span, leaving it untouched
/// otherwise. Used for a clean click, where a release with no movement activates a link.
fn activate_link_under_pointer(
    mouse: &MouseEvent,
    buffer: &CellBuffer,
    scroll_offset: u16,
    body_area_top_row: u16,
    navigate_to_url: &mut Option<String>,
) {
    let Some(pointer) = document_coordinate(mouse, scroll_offset, body_area_top_row) else {
        return;
    };
    let clicked = buffer
        .links()
        .iter()
        .find(|&span| span_contains(span, pointer.row, pointer.column));
    if let Some(span) = clicked {
        *navigate_to_url = Some(span.url.clone());
    }
}

/// Shows the citation preview when the pointer rests on a citation span, or clears it
/// otherwise. Leaves the preview untouched when the pointer resolves outside the document
/// area, mirroring how `activate_link_under_pointer` silently no-ops there.
fn update_citation_preview_for_pointer(
    mouse: &MouseEvent,
    buffer: &CellBuffer,
    scroll_offset: u16,
    body_area_top_row: u16,
    ui_state: &mut UiState,
) {
    let Some(pointer) = document_coordinate(mouse, scroll_offset, body_area_top_row) else {
        return;
    };
    let hovered = buffer.links().iter().find(|span| {
        span_contains(span, pointer.row, pointer.column) && span.kind == LinkKind::Citation
    });
    match hovered {
        Some(span) => ui_state.set_citation_preview(span.url.clone()),
        None => ui_state.clear_citation_preview(),
    }
}

fn install_terminal() -> Result<AppTerminal, TerminalError> {
    enable_raw_mode().map_err(|_| TerminalError::RenderFailed)?;
    let mut stdout = std::io::stdout();
    if execute!(stdout, EnterAlternateScreen, EnableMouseCapture).is_err() {
        let _ = disable_raw_mode();
        return Err(TerminalError::RenderFailed);
    }
    match Terminal::new(CrosstermBackend::new(stdout)) {
        Ok(terminal) => Ok(terminal),
        Err(_error) => {
            leave_alternate_screen_and_raw_mode();
            Err(TerminalError::RenderFailed)
        }
    }
}

fn restore_terminal(terminal: &mut AppTerminal) {
    let _ = disable_raw_mode();
    let _ = execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    );
    let _ = terminal.show_cursor();
}

fn leave_alternate_screen_and_raw_mode() {
    let _ = execute!(std::io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    let _ = disable_raw_mode();
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
