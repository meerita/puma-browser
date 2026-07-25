//! @file crates/browser-terminal/src/lib.rs
//! @description Terminal adapter: scrollable read-only viewport over the navigation core.
//! @layer terminal
//! @created meerita <meerita@icloud.com>

mod command_bar;
mod error;
mod hints_bar;
mod input;
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
use browser_layout::{Cell, CellBuffer};
use crossterm::event::{Event, EventStream, KeyCode, KeyEventKind, KeyModifiers};
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
use title_bar::compose_title_bar;
use ui_state::UiState;
use viewport::{max_scroll_offset, scroll_percentage, ScrollState};

/// Rows consumed by the five fixed chrome zones (title + sep + cmd + sep + hints).
const CHROME_ROWS: u16 = 5;

/// Terminal columns of left and right padding on the content area.
const CONTENT_PADDING: u16 = 2;

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
            )?;

            // Accumulators for state transitions; populated inside the select! arms and
            // applied after the borrow on load_state is released.
            let mut completed_load: Option<(NavigationController, Result<(), CoreError>)> = None;
            let mut command_to_submit: Option<String> = None;
            let mut reload_url: Option<BrowserUrl> = None;

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
                        let Event::Key(key) = event else { continue; };
                        if key.kind == KeyEventKind::Release { continue; }
                        let in_command_mode = ui_state.is_in_command_mode();
                        let action = map_key_event(
                            key,
                            ui_state.quit_armed,
                            ui_state.refresh_armed,
                            in_command_mode,
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
                        ui_state.quit_armed = quit_armed_after(action);
                        ui_state.refresh_armed = refresh_armed_after(action);
                        if matches!(action, InputAction::ArmQuit) {
                            ui_state.set_transient_hint("Press Esc again to quit", now);
                        } else if matches!(action, InputAction::ArmRefresh) {
                            ui_state.set_transient_hint("Press r again to refresh", now);
                        }
                    }
                    _ = tick.tick() => {}
                }
            }

            // Apply completed load — borrow on load_state has ended.
            if let Some((ctrl, result)) = completed_load {
                self.controller = ctrl;
                load_state = LoadState::Idle;
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
                        let (progress_tx, progress_rx) = watch::channel(0usize);
                        let loading_url = url.to_string();
                        let mut taken = std::mem::take(&mut self.controller);
                        let handle = tokio::spawn(async move {
                            let result = taken.load_with_progress(url, progress_tx).await;
                            (taken, result)
                        });
                        load_state = LoadState::Active {
                            handle,
                            progress_rx,
                            spinner_frame: 0,
                            loading_url,
                        };
                    }
                }
            }

            // Apply page refresh — re-fetch the current URL using the same load pipeline.
            if let Some(url) = reload_url {
                let (progress_tx, progress_rx) = watch::channel(0usize);
                let loading_url = url.to_string();
                let mut taken = std::mem::take(&mut self.controller);
                let handle = tokio::spawn(async move {
                    let result = taken.load_with_progress(url, progress_tx).await;
                    (taken, result)
                });
                load_state = LoadState::Active {
                    handle,
                    progress_rx,
                    spinner_frame: 0,
                    loading_url,
                };
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

    draw_body(frame, view, page, chunks[0], scroll_offset);

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
        frame.render_widget(Paragraph::new(cmd_text), chunks[2]);
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

fn draw_body(
    frame: &mut Frame,
    view: &ViewState,
    page: Option<&CellBuffer>,
    area: Rect,
    scroll_offset: u16,
) {
    let padded = Rect {
        x: area.x.saturating_add(CONTENT_PADDING),
        y: area.y,
        width: area.width.saturating_sub(CONTENT_PADDING * 2),
        height: area.height,
    };
    match view {
        ViewState::Page => draw_page(frame, page, area, padded, scroll_offset),
        ViewState::Blank => draw_message(frame, padded, BLANK_PLACEHOLDER),
        ViewState::Error(message) => draw_message(frame, padded, message),
    }
}

fn draw_page(
    frame: &mut Frame,
    page: Option<&CellBuffer>,
    clear_area: Rect,
    blit_area: Rect,
    scroll_offset: u16,
) {
    frame.render_widget(Clear, clear_area);
    let Some(cells) = page else {
        return;
    };
    let buffer = frame.buffer_mut();
    for row in 0..blit_area.height {
        blit_row(buffer, blit_area, cells, scroll_offset, row);
    }
}

fn blit_row(buffer: &mut Buffer, area: Rect, cells: &CellBuffer, scroll_offset: u16, row: u16) {
    let source_row = scroll_offset.saturating_add(row);
    for column in 0..area.width {
        copy_cell(buffer, area, cells, column, row, source_row);
    }
}

/// Copies one sanitized source cell into the target buffer, ignoring positions outside
/// either buffer so a wide grapheme near an edge can never index out of bounds.
fn copy_cell(
    buffer: &mut Buffer,
    area: Rect,
    cells: &CellBuffer,
    column: u16,
    row: u16,
    source_row: u16,
) {
    let Some(cell) = cells.cell_at(column, source_row) else {
        return;
    };
    let position = (area.x.saturating_add(column), area.y.saturating_add(row));
    let Some(target) = buffer.cell_mut(position) else {
        return;
    };
    target.set_symbol(cell.grapheme());
    target.set_style(cell_style(cell));
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
        | InputAction::CommandSubmit => {}
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
        | InputAction::CommandSubmit => {}
    }
}

fn install_terminal() -> Result<AppTerminal, TerminalError> {
    enable_raw_mode().map_err(|_| TerminalError::RenderFailed)?;
    let mut stdout = std::io::stdout();
    if execute!(stdout, EnterAlternateScreen).is_err() {
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
    let _ = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let _ = terminal.show_cursor();
}

fn leave_alternate_screen_and_raw_mode() {
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    let _ = disable_raw_mode();
}
