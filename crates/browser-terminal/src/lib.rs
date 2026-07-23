//! @file crates/browser-terminal/src/lib.rs
//! @description Terminal adapter: scrollable read-only viewport over the navigation core.
//! @layer terminal
//! @created meerita <meerita@icloud.com>

mod error;
mod initial_view;
mod input;
mod status_line;
mod viewport;

pub use error::TerminalError;
pub use initial_view::InitialView;

use std::io::Stdout;

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
use ratatui::layout::Rect;
use ratatui::style::{Color as TerminalColor, Modifier, Style};
use ratatui::widgets::{Clear, Paragraph, Wrap};
use ratatui::{Frame, Terminal};

use input::{map_key_event, quit_armed_after, InputAction};
use status_line::compose_status_line;
use viewport::{max_scroll_offset, scroll_percentage, ScrollState};

/// The number of bottom rows reserved for the status line.
const STATUS_LINE_ROWS: u16 = 1;

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

/// A cell buffer cached alongside the width it was laid out for, so the page is
/// re-rendered only when the terminal width changes.
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
        let mut quit_armed = false;
        let mut cache: Option<CachedPage> = None;
        loop {
            let size = terminal.size().map_err(|_| TerminalError::RenderFailed)?;
            let viewport_height = size.height.saturating_sub(STATUS_LINE_ROWS);
            self.refresh_page_cache(&mut cache, size.width)?;
            let content_rows = cache.as_ref().map_or(0, |cached| cached.buffer.height());
            let max_offset = max_scroll_offset(content_rows, viewport_height);
            scroll.clamp(max_offset);
            let status_text = self.status_text(scroll.offset(), max_offset, quit_armed);
            let page = cache.as_ref().map(|cached| &cached.buffer);
            self.draw(terminal, page, scroll.offset(), &status_text)?;
            if let LoopControl::Quit =
                step_event(&mut scroll, &mut quit_armed, viewport_height, max_offset)?
            {
                return Ok(());
            }
        }
    }

    /// Lays the page out again only when there is a page to show and the width changed.
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

    fn status_text(&self, offset: u16, max_offset: u16, quit_armed: bool) -> String {
        let label = self.status_label();
        let percent = scroll_percentage(offset, max_offset);
        compose_status_line(&label, percent, self.controller.script_count(), quit_armed)
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

    fn draw(
        &self,
        terminal: &mut AppTerminal,
        page: Option<&CellBuffer>,
        scroll_offset: u16,
        status_text: &str,
    ) -> Result<(), TerminalError> {
        terminal
            .draw(|frame| draw_frame(frame, &self.initial_view, page, scroll_offset, status_text))
            .map_err(|_| TerminalError::RenderFailed)?;
        Ok(())
    }
}

/// Reads one event and applies it, reporting whether the loop should quit.
fn step_event(
    scroll: &mut ScrollState,
    quit_armed: &mut bool,
    viewport_height: u16,
    max_offset: u16,
) -> Result<LoopControl, TerminalError> {
    let event = event::read().map_err(|_| TerminalError::RenderFailed)?;
    let Event::Key(key) = event else {
        return Ok(LoopControl::Continue);
    };
    if key.kind == KeyEventKind::Release {
        return Ok(LoopControl::Continue);
    }
    let action = map_key_event(key, *quit_armed);
    if matches!(action, InputAction::Quit) {
        return Ok(LoopControl::Quit);
    }
    apply_scroll(action, scroll, viewport_height, max_offset);
    *quit_armed = quit_armed_after(action);
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
        InputAction::ArmQuit | InputAction::Disarm | InputAction::Quit => {}
    }
}

fn render_page(controller: &NavigationController, width: u16) -> Result<CellBuffer, TerminalError> {
    if width == 0 {
        return Ok(CellBuffer::new(0, 0));
    }
    let buffer = controller.render(width)?;
    Ok(buffer)
}

fn draw_frame(
    frame: &mut Frame,
    view: &InitialView,
    page: Option<&CellBuffer>,
    scroll_offset: u16,
    status_text: &str,
) {
    let (body_area, status_area) = split_status_line(frame.area());
    draw_body(frame, view, page, body_area, scroll_offset);
    draw_status_line(frame, status_area, status_text);
}

/// Splits the full area into the page body and the reserved status row.
fn split_status_line(area: Rect) -> (Rect, Rect) {
    if area.height == 0 {
        return (area, area);
    }
    let status_area = Rect {
        x: area.x,
        y: area.y + area.height - STATUS_LINE_ROWS,
        width: area.width,
        height: STATUS_LINE_ROWS,
    };
    let body_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height - STATUS_LINE_ROWS,
    };
    (body_area, status_area)
}

fn draw_body(
    frame: &mut Frame,
    view: &InitialView,
    page: Option<&CellBuffer>,
    area: Rect,
    scroll_offset: u16,
) {
    match view {
        InitialView::Page => draw_page(frame, page, area, scroll_offset),
        InitialView::Blank => draw_message(frame, area, BLANK_PLACEHOLDER),
        InitialView::Error(message) => draw_message(frame, area, message),
    }
}

fn draw_page(frame: &mut Frame, page: Option<&CellBuffer>, area: Rect, scroll_offset: u16) {
    frame.render_widget(Clear, area);
    let Some(cells) = page else {
        return;
    };
    let buffer = frame.buffer_mut();
    for row in 0..area.height {
        blit_row(buffer, area, cells, scroll_offset, row);
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

fn draw_status_line(frame: &mut Frame, area: Rect, status_text: &str) {
    let paragraph = Paragraph::new(status_text).style(status_line_style());
    frame.render_widget(paragraph, area);
}

fn status_line_style() -> Style {
    Style::default().add_modifier(Modifier::REVERSED)
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
