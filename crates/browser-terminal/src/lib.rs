//! @file crates/browser-terminal/src/lib.rs
//! @description Terminal adapter: scrollable read-only viewport over the navigation core.
//! @layer terminal
//! @created meerita <meerita@icloud.com>

mod command_bar;
mod error;
mod hints_bar;
mod initial_view;
mod input;
mod title_bar;
mod ui_state;
mod viewport;

pub use error::TerminalError;
pub use initial_view::InitialView;

use std::io::Stdout;
use std::time::{Duration, Instant};

use browser_core::NavigationController;
use browser_css::{Color, Emphasis};
use browser_layout::{Cell, CellBuffer};
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color as TerminalColor, Modifier, Style};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use command_bar::compose_command_bar_reading;
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
    initial_view: InitialView,
}

/// A cell buffer cached alongside the content width it was laid out for, so the page
/// is re-rendered only when the content area width changes.
struct CachedPage {
    width: u16,
    buffer: CellBuffer,
}

/// Whether the event loop should keep running or exit.
enum LoopControl {
    Continue,
    Quit,
}

impl TerminalApp {
    pub fn new(controller: NavigationController, initial_view: InitialView) -> Self {
        Self {
            controller,
            initial_view,
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
    pub fn run(&mut self) -> Result<(), TerminalError> {
        let mut terminal = install_terminal()?;
        let outcome = self.drive(&mut terminal);
        restore_terminal(&mut terminal);
        outcome
    }

    fn drive(&self, terminal: &mut AppTerminal) -> Result<(), TerminalError> {
        let mut scroll = ScrollState::new();
        let mut ui_state = UiState::new();
        let mut cache: Option<CachedPage> = None;
        loop {
            let now = Instant::now();
            ui_state.advance_hint_if_due(now);
            ui_state.clear_transient_if_expired(now);
            let size = terminal.size().map_err(|_| TerminalError::RenderFailed)?;
            let viewport_height = size.height.saturating_sub(CHROME_ROWS);
            let content_width = size.width.saturating_sub(CONTENT_PADDING * 2);
            self.refresh_page_cache(&mut cache, content_width)?;
            let content_rows = cache.as_ref().map_or(0, |cached| cached.buffer.height());
            let max_offset = max_scroll_offset(content_rows, viewport_height);
            scroll.clamp(max_offset);
            let label = self.status_label();
            let scroll_percent_val = scroll_percentage(scroll.offset(), max_offset);
            let page = cache.as_ref().map(|cached| &cached.buffer);
            self.draw(
                terminal,
                page,
                scroll.offset(),
                &ui_state,
                &label,
                scroll_percent_val,
                self.controller.script_count(),
            )?;
            if let LoopControl::Quit =
                step_event(&mut scroll, &mut ui_state, viewport_height, max_offset, now)?
            {
                return Ok(());
            }
        }
    }

    /// Lays the page out again only when there is a page and the content width changed.
    fn refresh_page_cache(
        &self,
        cache: &mut Option<CachedPage>,
        width: u16,
    ) -> Result<(), TerminalError> {
        if !matches!(self.initial_view, InitialView::Page) {
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
        match &self.initial_view {
            InitialView::Blank => "blank".to_string(),
            InitialView::Error(_) => "error".to_string(),
            InitialView::Page => self.page_label(),
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
    ) -> Result<(), TerminalError> {
        terminal
            .draw(|frame| {
                draw_frame(
                    frame,
                    &self.initial_view,
                    label,
                    page,
                    scroll_offset,
                    ui_state,
                    scroll_percent,
                    script_count,
                )
            })
            .map_err(|_| TerminalError::RenderFailed)?;
        Ok(())
    }
}

/// Polls for one event and applies it, reporting whether the loop should quit.
///
/// Returns immediately with `Continue` when no event arrives within the poll window,
/// allowing the caller to check timers and redraw between keypresses.
fn step_event(
    scroll: &mut ScrollState,
    ui_state: &mut UiState,
    viewport_height: u16,
    max_offset: u16,
    now: Instant,
) -> Result<LoopControl, TerminalError> {
    let poll_available =
        event::poll(Duration::from_millis(250)).map_err(|_| TerminalError::RenderFailed)?;
    if !poll_available {
        return Ok(LoopControl::Continue);
    }
    let event = event::read().map_err(|_| TerminalError::RenderFailed)?;
    let Event::Key(key) = event else {
        return Ok(LoopControl::Continue);
    };
    if key.kind == KeyEventKind::Release {
        return Ok(LoopControl::Continue);
    }
    let action = map_key_event(key, ui_state.quit_armed, ui_state.refresh_armed);
    if matches!(action, InputAction::Quit) {
        return Ok(LoopControl::Quit);
    }
    ui_state.clear_transient();
    apply_scroll(action, scroll, viewport_height, max_offset);
    ui_state.quit_armed = quit_armed_after(action);
    ui_state.refresh_armed = refresh_armed_after(action);
    if matches!(action, InputAction::ArmQuit) {
        ui_state.set_transient_hint("Press Esc again to quit", now);
    } else if matches!(action, InputAction::ArmRefresh) {
        ui_state.set_transient_hint("Press r again to refresh", now);
    }
    Ok(LoopControl::Continue)
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
        | InputAction::Quit => {}
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
    view: &InitialView,
    label: &str,
    page: Option<&CellBuffer>,
    scroll_offset: u16,
    ui_state: &UiState,
    scroll_percent: u16,
    script_count: usize,
) {
    let terminal_width = frame.area().width;
    let chunks = Layout::vertical([
        Constraint::Length(1), // title bar
        Constraint::Length(1), // separator
        Constraint::Length(1), // command bar
        Constraint::Length(1), // separator
        Constraint::Min(0),    // content area
        Constraint::Length(1), // hints bar
    ])
    .split(frame.area());

    let title_text = compose_title_bar(label, scroll_percent, script_count, terminal_width);
    draw_chrome_row(frame, chunks[0], &title_text);

    draw_separator(frame, chunks[1]);

    let cmd_text = compose_command_bar_reading(ui_state.current_hint(), terminal_width);
    frame.render_widget(Paragraph::new(cmd_text), chunks[2]);

    draw_separator(frame, chunks[3]);

    draw_body(frame, view, page, chunks[4], scroll_offset);

    let hints_text = compose_hints_bar(None, terminal_width);
    draw_chrome_row(frame, chunks[5], &hints_text);
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
    view: &InitialView,
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
        InitialView::Page => draw_page(frame, page, area, padded, scroll_offset),
        InitialView::Blank => draw_message(frame, padded, BLANK_PLACEHOLDER),
        InitialView::Error(message) => draw_message(frame, padded, message),
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
