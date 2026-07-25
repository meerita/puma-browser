// @file crates/browser-layout/src/cell.rs
// @description Terminal cell and cell-buffer types that form the layout stage output.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::{Color, Emphasis, TextStyle};

/// The terminal-row extent of a single hyperlink span in the laid-out buffer.
///
/// A link that wraps across multiple lines produces one `LinkSpan` per row. `col_end`
/// is the inclusive last column of the span on that row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpan {
    pub url: String,
    pub row: u16,
    pub col_start: u16,
    pub col_end: u16,
}

/// A single terminal cell: one grapheme cluster and the display attributes used to
/// render it.
///
/// The grapheme is stored as an owned `String` because a cluster may combine several
/// Unicode scalar values (base character plus combining marks). Attributes are the
/// reduced set the terminal can express directly; richer styling can be added when the
/// full cascade and layout algorithm land.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    grapheme: String,
    foreground: Option<Color>,
    background: Option<Color>,
    emphasis: Emphasis,
    underline: bool,
    link_url: Option<String>,
}

impl Cell {
    /// Builds a cell for one grapheme cluster, taking its display attributes from the
    /// computed style of the node or run the grapheme belongs to.
    pub fn new(grapheme: String, style: &TextStyle) -> Cell {
        Cell {
            grapheme,
            foreground: style.foreground,
            background: style.background,
            emphasis: style.emphasis,
            underline: style.underline,
            link_url: None,
        }
    }

    /// The blank cell used to fill an empty buffer: a single space with no attributes.
    pub fn blank() -> Cell {
        Cell {
            grapheme: String::from(" "),
            foreground: None,
            background: None,
            emphasis: Emphasis::None,
            underline: false,
            link_url: None,
        }
    }

    /// A copy of this cell showing a different grapheme, keeping every display attribute.
    ///
    /// Used to substitute a neutral placeholder for an emoji cluster without disturbing the
    /// cell's colour, emphasis, or underline.
    pub(crate) fn with_grapheme(&self, grapheme: String) -> Cell {
        Cell {
            grapheme,
            ..self.clone()
        }
    }

    /// Records the URL of the link this cell belongs to, used only by layout to extract
    /// link spans. The URL is never rendered from here; it is not part of the public API.
    pub(crate) fn set_link_url(&mut self, url: String) {
        self.link_url = Some(url);
    }

    /// The URL of the link this cell belongs to, or `None` when the cell is not part of a
    /// link. Crate-private so remote-sourced URLs never leak through the public cell API.
    pub(crate) fn link_url(&self) -> Option<&str> {
        self.link_url.as_deref()
    }

    pub fn grapheme(&self) -> &str {
        &self.grapheme
    }

    pub fn foreground(&self) -> Option<Color> {
        self.foreground
    }

    pub fn background(&self) -> Option<Color> {
        self.background
    }

    pub fn emphasis(&self) -> Emphasis {
        self.emphasis
    }

    /// Whether the grapheme is underlined, used to mark a link so it stays distinguishable
    /// without relying on color.
    pub fn underline(&self) -> bool {
        self.underline
    }
}

/// A fixed-size grid of terminal cells, addressed by column and row.
///
/// This is the output contract of the layout stage: the layout engine writes into the
/// buffer and the terminal adapter reads cells from it. The buffer never emits escape
/// sequences itself.
#[derive(Debug, Clone)]
pub struct CellBuffer {
    width: u16,
    height: u16,
    cells: Vec<Cell>,
    links: Vec<LinkSpan>,
}

impl CellBuffer {
    /// Allocates a blank buffer of the requested dimensions, filled with blank cells.
    pub fn new(width: u16, height: u16) -> CellBuffer {
        let cell_count = usize::from(width) * usize::from(height);
        let cells = vec![Cell::blank(); cell_count];
        CellBuffer {
            width,
            height,
            cells,
            links: Vec::new(),
        }
    }

    /// The link spans recorded for this buffer, in row-major order.
    pub fn links(&self) -> &[LinkSpan] {
        &self.links
    }

    pub(crate) fn set_links(&mut self, links: Vec<LinkSpan>) {
        self.links = links;
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    /// Overwrites the cell at `column`, `row`.
    ///
    /// A position outside the buffer is ignored rather than an error, so a layout pass
    /// that writes a wide grapheme near the right edge can never index out of bounds.
    pub(crate) fn set_cell(&mut self, column: u16, row: u16, cell: Cell) {
        if column >= self.width {
            return;
        }
        if row >= self.height {
            return;
        }
        let index = usize::from(row) * usize::from(self.width) + usize::from(column);
        if let Some(slot) = self.cells.get_mut(index) {
            *slot = cell;
        }
    }

    /// Returns the cell at `column`, `row`, or `None` when the position lies outside the
    /// buffer's dimensions.
    pub fn cell_at(&self, column: u16, row: u16) -> Option<&Cell> {
        if column >= self.width {
            return None;
        }
        if row >= self.height {
            return None;
        }
        let index = usize::from(row) * usize::from(self.width) + usize::from(column);
        self.cells.get(index)
    }
}
