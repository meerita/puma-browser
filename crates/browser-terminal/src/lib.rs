//! @file crates/browser-terminal/src/lib.rs
//! @description Terminal adapter: scrollable read-only viewport over the navigation core.
//! @layer terminal
//! @created meerita <meerita@icloud.com>

// The copy entry point is invoked from the mouse-release path added later on
// this branch; until that lands the module has no in-crate caller.
#[allow(dead_code)]
mod clipboard;
mod command_bar;
mod error;
mod hints_bar;
mod input;
mod selection;
mod title_bar;
mod ui_state;
mod view_state;
mod viewport;

pub use error::TerminalError;
pub use view_state::ViewState;

use std::io::Stdout;
use std::path::PathBuf;
use std::time::Instant;

use browser_core::{BrowserUrl, CoreError, NavigationController};
use browser_css::{Color, Emphasis};
use browser_layout::{Cell, CellBuffer, CellPosition, LinkSpan};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
    KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures_util::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color as TerminalColor, Modifier, Style};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::{interval, Duration};

use command_bar::{
    command_cursor_col, compose_command_bar_command, compose_command_bar_loading,
    compose_command_bar_reading,
};
use hints_bar::compose_hints_bar;
use input::{map_key_event, quit_armed_after, refresh_armed_after, InputAction};
use selection::TextSelection;
use title_bar::compose_title_bar;
use ui_state::UiState;
use viewport::{max_scroll_offset, scroll_percentage, ScrollState};

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
}

/// A cell buffer cached alongside the content width it was laid out for, so the page
/// is re-rendered only when the content area width changes.
struct CachedPage {
    width: u16,
    buffer: CellBuffer,
}

impl TerminalApp {
    pub fn new(controller: NavigationController, view_state: ViewState) -> Self {
        Self {
            controller,
            view_state,
        }
    }

    /// Borrows the navigation core this adapter drives.
    pub fn controller(&self) -> &NavigationController {
        &self.controller
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
        let mut scroll = ScrollState::new();
        let mut selection = TextSelection::new();
        let mut ui_state = UiState::new();
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
                loading_ref,
                selection_range,
            )?;

            // Accumulators for state transitions; populated inside the select! arms and
            // applied after the borrow on load_state is released.
            let mut completed_load: Option<(NavigationController, Result<(), CoreError>)> = None;
            let mut command_to_submit: Option<String> = None;
            let mut reload_url: Option<BrowserUrl> = None;
            let mut navigate_to_url: Option<String> = None;

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
                                let in_link_navigation = ui_state.is_in_link_navigation();
                                let action = map_key_event(
                                    key,
                                    ui_state.quit_armed,
                                    ui_state.refresh_armed,
                                    in_command_mode,
                                    in_link_navigation,
                                );
                                if matches!(action, InputAction::Quit) {
                                    return Ok(());
                                }
                                ui_state.clear_transient();
                                apply_scroll(action, &mut scroll, viewport_height, max_offset);
                                if matches!(action, InputAction::CommandSubmit) {
                                    command_to_submit = Some(ui_state.take_command_buffer());
                                } else {
                                    apply_command_action(action, &mut ui_state);
                                }
                                if matches!(action, InputAction::RefreshArmed) {
                                    reload_url = self.controller.current_url().cloned();
                                }
                                let scroll_offset = scroll.offset();
                                if let Some(link_url) = handle_navigation_action(
                                    action,
                                    &mut self.controller,
                                    &mut ui_state,
                                    &mut cache,
                                    &mut scroll,
                                    &mut self.view_state,
                                    scroll_offset,
                                ) {
                                    navigate_to_url = Some(link_url);
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
                                handle_mouse_event(
                                    mouse,
                                    cache.as_ref().map(|cached| &cached.buffer),
                                    scroll.offset(),
                                    BODY_AREA_TOP_ROW,
                                    &mut selection,
                                    &mut navigate_to_url,
                                );
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
                    }
                }
            }

            // Apply command submission.
            if let Some(input) = command_to_submit {
                match browser_core::resolve_address(&input, &working_dir) {
                    Err(_) => {
                        self.view_state = ViewState::Error(format!("Not a valid address: {input}"));
                        cache = None;
                    }
                    Ok(url) => {
                        load_state = self.start_load(url);
                    }
                }
            }

            // Apply page refresh — re-fetch the current URL using the same load pipeline.
            if let Some(url) = reload_url {
                load_state = self.start_load(url);
            }

            // Apply link navigation — a Tab+Enter activation or a mouse click on a link.
            if let Some(link_url) = navigate_to_url {
                load_state =
                    self.start_link_load(link_url, &working_dir, &mut ui_state, &mut cache);
            }
        }
    }

    /// Marks a link URL visited, leaves link navigation, and starts loading it through
    /// the shared load path after scheme validation. On an unresolvable URL it shows a
    /// generic error rather than echoing the remote URL, so a malicious href can never
    /// place raw bytes in terminal output.
    fn start_link_load(
        &mut self,
        link_url: String,
        working_dir: &std::path::Path,
        ui_state: &mut UiState,
        cache: &mut Option<CachedPage>,
    ) -> LoadState {
        ui_state.mark_visited(&link_url);
        ui_state.exit_link_navigation();
        match browser_core::resolve_address(&link_url, working_dir) {
            Err(_) => {
                self.view_state = ViewState::Error("Cannot open this link".to_string());
                *cache = None;
                LoadState::Idle
            }
            Ok(url) => self.start_load(url),
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
            let result = taken.load_with_progress(url, progress_tx).await;
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
        loading: Option<(usize, &str, usize)>,
        selection_range: Option<(CellPosition, CellPosition)>,
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
                    loading,
                    selection_range,
                )
            })
            .map_err(|_| TerminalError::RenderFailed)?;
        Ok(())
    }
}

fn render_page(controller: &NavigationController, width: u16) -> Result<CellBuffer, TerminalError> {
    if width == 0 {
        return Ok(CellBuffer::new(0, 0));
    }
    let buffer = controller.render(width)?;
    Ok(buffer)
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
    loading: Option<(usize, &str, usize)>,
    selection_range: Option<(CellPosition, CellPosition)>,
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

    let focused_url = ui_state
        .focused_link_index
        .and_then(|index| page.and_then(|buffer| unique_links(buffer).get(index).copied()));
    let link_context = LinkRenderContext {
        focused_url,
        ui_state,
    };
    draw_body(
        frame,
        view,
        page,
        chunks[0],
        scroll_offset,
        &link_context,
        selection_range,
    );

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
        terminal_width,
    );
    draw_chrome_row(frame, chunks[4], &title_text);

    let hints_text = compose_hints_bar(None, terminal_width);
    frame.render_widget(Paragraph::new(hints_text), chunks[5]);
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
        | InputAction::FocusNextLink
        | InputAction::FocusPreviousLink
        | InputAction::ActivateFocusedLink
        | InputAction::NavigateBack => {}
    }
}

fn apply_command_action(action: InputAction, ui_state: &mut UiState) {
    match action {
        InputAction::EnterCommand(ch) => ui_state.enter_command_mode(ch),
        InputAction::CommandAppend(ch) => ui_state.command_append_char(ch),
        InputAction::CommandMoveCursorLeft => ui_state.command_move_left(),
        InputAction::CommandMoveCursorRight => ui_state.command_move_right(),
        InputAction::CommandDeleteBack => ui_state.command_delete_before_cursor(),
        InputAction::CommandCancel => ui_state.cancel_command_mode(),
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
        | InputAction::Disarm
        | InputAction::CommandSubmit
        | InputAction::FocusNextLink
        | InputAction::FocusPreviousLink
        | InputAction::ActivateFocusedLink
        | InputAction::NavigateBack => {}
    }
}

/// The unique link URLs in the order they first appear in the buffer (by row, then
/// column). Each URL appears once regardless of how many spans it wraps across, so this
/// is the list Tab navigation steps through.
fn unique_links(buffer: &CellBuffer) -> Vec<&str> {
    let mut seen: Vec<&str> = Vec::new();
    for span in buffer.links() {
        if !seen.contains(&span.url.as_str()) {
            seen.push(span.url.as_str());
        }
    }
    seen
}

/// The index within [`unique_links`] of the first link whose first span starts at or
/// below `scroll_offset`, so Tab focuses a link the reader can actually see. Falls back
/// to the first link when every link is above the viewport.
fn first_visible_link_index(buffer: &CellBuffer, scroll_offset: u16) -> usize {
    for (index, url) in unique_links(buffer).iter().enumerate() {
        let first_span = buffer.links().iter().find(|span| span.url.as_str() == *url);
        if let Some(span) = first_span {
            if span.row >= scroll_offset {
                return index;
            }
        }
    }
    0
}

/// Whether the cell at `column`, `row` falls within `span`'s extent on its row.
fn span_contains(span: &LinkSpan, row: u16, column: u16) -> bool {
    span.row == row && span.col_start <= column && column <= span.col_end
}

/// Applies a link-navigation or history action, returning the raw link URL to load when
/// the action activates the focused link. Focus and back actions mutate UI and controller
/// state directly and return `None`.
fn handle_navigation_action(
    action: InputAction,
    controller: &mut NavigationController,
    ui_state: &mut UiState,
    cache: &mut Option<CachedPage>,
    scroll: &mut ScrollState,
    view_state: &mut ViewState,
    scroll_offset: u16,
) -> Option<String> {
    match action {
        InputAction::FocusNextLink => {
            advance_link_focus(
                ui_state,
                cache.as_ref().map(|cached| &cached.buffer),
                scroll_offset,
            );
            None
        }
        InputAction::FocusPreviousLink => {
            retreat_link_focus(
                ui_state,
                cache.as_ref().map(|cached| &cached.buffer),
                scroll_offset,
            );
            None
        }
        InputAction::ActivateFocusedLink => {
            focused_link_url(ui_state, cache.as_ref().map(|cached| &cached.buffer))
        }
        InputAction::NavigateBack => {
            navigate_back(controller, ui_state, cache, scroll, view_state);
            None
        }
        InputAction::Disarm => {
            if ui_state.is_in_link_navigation() {
                ui_state.exit_link_navigation();
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
        | InputAction::CommandSubmit => None,
    }
}

/// Moves focus to the next link, entering link navigation at the first visible link when
/// nothing is focused yet.
fn advance_link_focus(ui_state: &mut UiState, buffer: Option<&CellBuffer>, scroll_offset: u16) {
    let Some(buffer) = buffer else {
        return;
    };
    let count = unique_links(buffer).len();
    if count == 0 {
        return;
    }
    if ui_state.focused_link_index.is_none() {
        ui_state.enter_link_navigation(first_visible_link_index(buffer, scroll_offset));
        return;
    }
    ui_state.focus_next_link(count);
}

/// Moves focus to the previous link, entering link navigation at the first visible link
/// when nothing is focused yet.
fn retreat_link_focus(ui_state: &mut UiState, buffer: Option<&CellBuffer>, scroll_offset: u16) {
    let Some(buffer) = buffer else {
        return;
    };
    let count = unique_links(buffer).len();
    if count == 0 {
        return;
    }
    if ui_state.focused_link_index.is_none() {
        ui_state.enter_link_navigation(first_visible_link_index(buffer, scroll_offset));
        return;
    }
    ui_state.focus_previous_link(count);
}

/// The URL of the currently focused link, if link navigation is active and the focused
/// index still points at a link in the buffer.
fn focused_link_url(ui_state: &UiState, buffer: Option<&CellBuffer>) -> Option<String> {
    let index = ui_state.focused_link_index?;
    let buffer = buffer?;
    unique_links(buffer).get(index).map(|url| url.to_string())
}

/// Restores the previous page from history and resets the viewport, clearing any focused
/// link. Does nothing when there is no history to restore.
fn navigate_back(
    controller: &mut NavigationController,
    ui_state: &mut UiState,
    cache: &mut Option<CachedPage>,
    scroll: &mut ScrollState,
    view_state: &mut ViewState,
) {
    if !controller.go_back() {
        return;
    }
    *view_state = ViewState::Page;
    *cache = None;
    *scroll = ScrollState::new();
    ui_state.exit_link_navigation();
}

/// Dispatches a left-button mouse gesture over the page body into a text selection.
///
/// A press begins a selection; a drag extends it, clamped to the buffer so dragging into
/// the chrome or past the content never leaves the grid; a release either keeps the
/// highlight (the gesture moved, so it is a selection) or activates a link (the gesture did
/// not move, so it is a click). Wheel and other events are ignored so scrolling is
/// untouched.
fn handle_mouse_event(
    mouse: MouseEvent,
    buffer: Option<&CellBuffer>,
    scroll_offset: u16,
    body_area_top_row: u16,
    selection: &mut TextSelection,
    navigate_to_url: &mut Option<String>,
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
        _ => {}
    }
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
