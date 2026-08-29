// @file crates/browser-layout/src/cell.rs
// @description Terminal cell and cell-buffer types that form the layout stage output.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::{Color, Emphasis, TextStyle};
use browser_html::NodeId;

use crate::field_span::{FieldSpan, FieldSpanKind};

/// Whether a link span comes from an author-intended hyperlink (`<a href>`) or a
/// citation (`<q cite>`).
///
/// A `<q cite>` nested inside an `<a href>` always resolves to `Hyperlink` for that
/// cell's span, since the enclosing anchor is the author's explicit navigation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkKind {
    Hyperlink,
    Citation,
}

/// The terminal-row extent of a single hyperlink or citation span in the laid-out
/// buffer.
///
/// A link that wraps across multiple lines produces one `LinkSpan` per row. `col_end`
/// is the inclusive last column of the span on that row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpan {
    pub url: String,
    pub kind: LinkKind,
    pub row: u16,
    pub col_start: u16,
    pub col_end: u16,
}

/// The row a fragment anchor target sits on in the laid-out buffer.
///
/// An anchor names a point, not a range, so it carries only its name and the row where the
/// run it belongs to begins. A run carrying several names produces one `AnchorSpan` per
/// name, all on that row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchorSpan {
    pub name: String,
    pub row: u16,
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
    citation_url: Option<String>,
    anchor_names: Vec<String>,
    field_marker: Option<(NodeId, FieldSpanKind)>,
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
            citation_url: None,
            anchor_names: Vec::new(),
            field_marker: None,
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
            citation_url: None,
            anchor_names: Vec::new(),
            field_marker: None,
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

    /// Records the citation URL (`<q cite>`) this cell belongs to, used only by layout to
    /// extract link spans. The URL is never rendered from here; it is not part of the
    /// public API.
    pub(crate) fn set_citation_url(&mut self, url: String) {
        self.citation_url = Some(url);
    }

    /// The citation URL this cell belongs to, or `None` when the cell is not part of a
    /// `<q cite>`. Crate-private so remote-sourced URLs never leak through the public
    /// cell API.
    pub(crate) fn citation_url(&self) -> Option<&str> {
        self.citation_url.as_deref()
    }

    /// Records the anchor names whose target begins at this cell, used only by layout to
    /// extract anchor spans. Set on the first grapheme so an anchor marks one row.
    ///
    /// Names already present are not added again, and existing names are kept, because a
    /// cell can be named both by the run it belongs to and by a target declared on the
    /// block that starts here. Overwriting would drop one of the two.
    pub(crate) fn push_anchor_names(&mut self, names: &[String]) {
        for name in names {
            if self.anchor_names.iter().any(|present| present == name) {
                continue;
            }
            self.anchor_names.push(name.clone());
        }
    }

    /// The anchor names whose run begins at this cell, empty when no anchor starts here.
    /// Crate-private so remote-sourced names never leak through the public cell API.
    pub(crate) fn anchor_names(&self) -> &[String] {
        &self.anchor_names
    }

    /// Records the interactive form control this cell belongs to, used only by layout to
    /// extract field spans. Not part of the public API: a control's identity reaches the
    /// terminal only through the extracted [`FieldSpan`]s, never through the cell itself.
    pub(crate) fn set_field_marker(&mut self, node_id: NodeId, kind: FieldSpanKind) {
        self.field_marker = Some((node_id, kind));
    }

    /// The interactive form control this cell belongs to, or `None` when the cell is not
    /// part of one. Crate-private for the same reason as [`Self::link_url`].
    pub(crate) fn field_marker(&self) -> Option<(NodeId, FieldSpanKind)> {
        self.field_marker
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

/// A single cell position in the buffer, addressed by column and row.
///
/// Used to pass the two ends of a linear text selection to `text_in_range` without
/// coupling the layout stage to any terminal coordinate type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellPosition {
    pub column: u16,
    pub row: u16,
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
    anchors: Vec<AnchorSpan>,
    field_spans: Vec<FieldSpan>,
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
            anchors: Vec::new(),
            field_spans: Vec::new(),
        }
    }

    /// The link spans recorded for this buffer, in row-major order.
    pub fn links(&self) -> &[LinkSpan] {
        &self.links
    }

    pub(crate) fn set_links(&mut self, links: Vec<LinkSpan>) {
        self.links = links;
    }

    /// The interactive form-control spans recorded for this buffer, in row-major order.
    pub fn field_spans(&self) -> &[FieldSpan] {
        &self.field_spans
    }

    pub(crate) fn set_field_spans(&mut self, field_spans: Vec<FieldSpan>) {
        self.field_spans = field_spans;
    }

    /// The anchor spans recorded for this buffer, in ascending row order.
    pub fn anchors(&self) -> &[AnchorSpan] {
        &self.anchors
    }

    pub(crate) fn set_anchors(&mut self, anchors: Vec<AnchorSpan>) {
        self.anchors = anchors;
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

    /// Extracts the text of a linear selection between two document coordinates.
    ///
    /// The two ends are given in any order; they are normalized into reading order
    /// (earlier row first, then smaller column) and both ends are inclusive. On a single
    /// row the selection spans the two columns; across rows it runs from the anchor
    /// column to the end of its row, covers interior rows in full, and ends at the cursor
    /// column on the final row. Each row's trailing blank cells are trimmed and rows are
    /// joined with `'\n'`. Coordinates outside the buffer contribute nothing, and an
    /// empty buffer or an empty range yields an empty string.
    pub fn text_in_range(&self, anchor: CellPosition, cursor: CellPosition) -> String {
        if self.width == 0 || self.height == 0 {
            return String::new();
        }
        let (start, end) = Self::reading_order(anchor, cursor);
        if start.row >= self.height {
            return String::new();
        }
        let final_row = end.row.min(self.height - 1);
        let mut rows_text: Vec<String> = Vec::new();
        for row in start.row..=final_row {
            let (first_column, last_column) = self.selected_columns(row, start, end);
            rows_text.push(self.row_text(row, first_column, last_column));
        }
        rows_text.join("\n")
    }

    /// Orders two positions into `(start, end)` so that `start` is the earlier cell in
    /// row-major reading order: earlier row first, then smaller column on the same row.
    fn reading_order(anchor: CellPosition, cursor: CellPosition) -> (CellPosition, CellPosition) {
        let anchor_is_first =
            anchor.row < cursor.row || (anchor.row == cursor.row && anchor.column <= cursor.column);
        if anchor_is_first {
            return (anchor, cursor);
        }
        (cursor, anchor)
    }

    /// The inclusive column bounds selected on `row` for a selection from `start` to
    /// `end`, clamped to the last column of the buffer.
    fn selected_columns(&self, row: u16, start: CellPosition, end: CellPosition) -> (u16, u16) {
        let last_column = self.width - 1;
        if start.row == end.row {
            return (start.column, end.column.min(last_column));
        }
        if row == start.row {
            return (start.column, last_column);
        }
        if row == end.row {
            return (0, end.column.min(last_column));
        }
        (0, last_column)
    }

    /// The text of `row` from `first_column` to `last_column` inclusive, with trailing
    /// blank cells trimmed. Columns outside the buffer contribute nothing.
    fn row_text(&self, row: u16, first_column: u16, last_column: u16) -> String {
        let mut graphemes: Vec<&str> = Vec::new();
        for column in first_column..=last_column {
            let Some(cell) = self.cell_at(column, row) else {
                continue;
            };
            graphemes.push(cell.grapheme());
        }
        while graphemes.last() == Some(&" ") {
            graphemes.pop();
        }
        graphemes.concat()
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

#[cfg(test)]
#[path = "cell_tests.rs"]
mod tests;
