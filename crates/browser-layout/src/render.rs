// @file crates/browser-layout/src/render.rs
// @description Lays a Document's node tree out into a width-correct terminal cell buffer.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::computed_style;
use browser_css::TextStyle;
use browser_html::{Document, InlineRun, SemanticNode};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::cell::{Cell, CellBuffer};
use crate::error::LayoutError;

/// Columns reserved to the left of a list item's text for its bullet and one space.
const LIST_MARKER_COLUMNS: usize = 2;

/// Columns a block quote's text is indented from the left margin.
const QUOTE_INDENT_COLUMNS: usize = 2;

/// The character repeated across a full row to draw a horizontal separator.
const SEPARATOR_GRAPHEME: &str = "─";

/// Lay a document out into a cell buffer sized to `width` columns.
///
/// Each node is styled by [`computed_style`] and turned into laid-out rows: text blocks
/// word-wrap to the width, code and preformatted blocks render verbatim and clip,
/// container nodes recurse into their children, and spacing is applied as blank rows
/// between blocks. The rows are then written into a blank buffer whose height is the
/// total row count.
pub fn render_document(document: &Document, width: u16) -> Result<CellBuffer, LayoutError> {
    if width == 0 {
        return Err(LayoutError::ZeroWidth);
    }
    let width_columns = usize::from(width);
    let rows = render_children(document.children(), width_columns);
    build_buffer(rows, width)
}

/// Lay a sequence of sibling nodes out into rows, applying each node's own spacing.
fn render_children(children: &[SemanticNode], width: usize) -> Vec<Vec<Cell>> {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for node in children {
        append_node_rows(&mut rows, node, width);
    }
    rows
}

/// Append a node's spacing and content rows to the running row list.
///
/// A structural node with no content contributes nothing, not even its spacing, so
/// invisible nodes never leave stray blank rows behind.
fn append_node_rows(rows: &mut Vec<Vec<Cell>>, node: &SemanticNode, width: usize) {
    let style = computed_style(node);
    let content = node_rows(node, &style, width);
    if content.is_empty() {
        return;
    }
    append_blank_rows(rows, style.spacing_before);
    rows.extend(content);
    append_blank_rows(rows, style.spacing_after);
}

fn append_blank_rows(rows: &mut Vec<Vec<Cell>>, count: u16) {
    for _ in 0..count {
        rows.push(Vec::new());
    }
}

/// Produce the content rows for a single node, before spacing is applied.
///
/// Container nodes recurse into their children through [`render_children`]; text blocks
/// flatten their inline runs into a single string and word-wrap it. Per-run emphasis and
/// links are a later phase, so a run's `emphasis` and `link` are ignored here.
fn node_rows(node: &SemanticNode, style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    match node {
        SemanticNode::Heading { runs, .. } => wrap_runs(runs, style, width),
        SemanticNode::Paragraph { runs } => wrap_runs(runs, style, width),
        SemanticNode::Quote { children } => render_quote(children, style, width),
        SemanticNode::List { children, .. } => render_children(children, width),
        SemanticNode::ListItem { children } => render_list_item(children, style, width),
        SemanticNode::CodeBlock { text } | SemanticNode::PreformattedBlock { text } => {
            render_verbatim(text, style, width)
        }
        SemanticNode::Separator => vec![separator_row(style, width)],
        SemanticNode::Warning { message } => single_row(clip_line(message, style, width)),
        SemanticNode::ImagePlaceholder { alt, .. } => {
            single_row(clip_line(&image_label(alt), style, width))
        }
        _ => Vec::new(),
    }
}

/// Concatenate a text block's inline runs and wrap the result to the width.
fn wrap_runs(runs: &[InlineRun], style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    let text: String = runs.iter().map(|run| run.text.as_str()).collect();
    wrap_styled_text(&text, style, width)
}

/// Wrap a run of text into rows no wider than `width` display columns.
fn wrap_styled_text(text: &str, style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    let mut wrapper = LineWrapper::new(width, style);
    for word in text.split_whitespace() {
        wrapper.push_word(word);
    }
    wrapper.finish()
}

/// Render a block quote: lay its children out into the indented content width, then push
/// each resulting row right by the quote indent.
fn render_quote(children: &[SemanticNode], style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    let indent = quote_indent(width);
    let content_width = width - indent;
    render_children(children, content_width)
        .into_iter()
        .map(|row| indent_row(row, indent, style))
        .collect()
}

fn quote_indent(width: usize) -> usize {
    if width > QUOTE_INDENT_COLUMNS {
        return QUOTE_INDENT_COLUMNS;
    }
    0
}

/// Render a list item: lay its children out into the content width, then prefix the
/// first row with a bullet and indent the continuation rows to align under it.
fn render_list_item(children: &[SemanticNode], style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    let content_width = list_content_width(width);
    render_children(children, content_width)
        .into_iter()
        .enumerate()
        .map(|(index, row)| decorate_list_row(index, row, style))
        .collect()
}

fn list_content_width(width: usize) -> usize {
    if width > LIST_MARKER_COLUMNS {
        return width - LIST_MARKER_COLUMNS;
    }
    width
}

fn decorate_list_row(index: usize, row: Vec<Cell>, style: &TextStyle) -> Vec<Cell> {
    let mut decorated = if index == 0 {
        bullet_prefix(style)
    } else {
        space_cells(LIST_MARKER_COLUMNS, style)
    };
    decorated.extend(row);
    decorated
}

/// Render code or preformatted text verbatim: one row per source line, clipped at the
/// width rather than wrapped, so the original line structure is preserved.
fn render_verbatim(text: &str, style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    text.split('\n')
        .map(|line| clip_line(line, style, width))
        .collect()
}

/// Turn a single line into cells, stopping before any grapheme that would cross `width`.
fn clip_line(line: &str, style: &TextStyle, width: usize) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    let mut columns = 0usize;
    for grapheme in line.graphemes(true) {
        let advance = grapheme_columns(grapheme);
        if columns + advance > width {
            break;
        }
        cells.push(Cell::new(grapheme.to_string(), style));
        columns += advance;
    }
    cells
}

fn separator_row(style: &TextStyle, width: usize) -> Vec<Cell> {
    (0..width)
        .map(|_| Cell::new(String::from(SEPARATOR_GRAPHEME), style))
        .collect()
}

fn image_label(alt: &str) -> String {
    format!("[{alt}]")
}

fn single_row(row: Vec<Cell>) -> Vec<Vec<Cell>> {
    vec![row]
}

fn bullet_prefix(style: &TextStyle) -> Vec<Cell> {
    vec![
        Cell::new(String::from("•"), style),
        Cell::new(String::from(" "), style),
    ]
}

fn indent_row(row: Vec<Cell>, indent: usize, style: &TextStyle) -> Vec<Cell> {
    let mut indented = space_cells(indent, style);
    indented.extend(row);
    indented
}

fn space_cells(count: usize, style: &TextStyle) -> Vec<Cell> {
    (0..count)
        .map(|_| Cell::new(String::from(" "), style))
        .collect()
}

fn graphemes_to_cells(text: &str, style: &TextStyle) -> Vec<Cell> {
    text.graphemes(true)
        .map(|grapheme| Cell::new(grapheme.to_string(), style))
        .collect()
}

fn count_columns(cells: &[Cell]) -> usize {
    cells
        .iter()
        .map(|cell| grapheme_columns(cell.grapheme()))
        .sum()
}

/// The number of terminal columns a single grapheme cluster advances.
///
/// A combining mark contributes zero, so a base-plus-mark cluster stays one column; a
/// CJK or other wide grapheme advances two.
fn grapheme_columns(grapheme: &str) -> usize {
    UnicodeWidthStr::width(grapheme)
}

/// Convert the laid-out rows into a filled cell buffer.
///
/// The height is the row count, guarded so a document taller than the addressable row
/// range is refused rather than truncated silently.
fn build_buffer(rows: Vec<Vec<Cell>>, width: u16) -> Result<CellBuffer, LayoutError> {
    let height = row_height(rows.len())?;
    let mut buffer = CellBuffer::new(width, height);
    for (row_index, row) in rows.into_iter().enumerate() {
        write_row(&mut buffer, row_index, row);
    }
    Ok(buffer)
}

fn row_height(count: usize) -> Result<u16, LayoutError> {
    u16::try_from(count).map_err(|_| LayoutError::DimensionOverflow)
}

/// Write one row of cells into the buffer, advancing the column by each grapheme's width
/// so a wide grapheme leaves the following column blank.
fn write_row(buffer: &mut CellBuffer, row_index: usize, row: Vec<Cell>) {
    let Ok(row_position) = u16::try_from(row_index) else {
        return;
    };
    let mut column = 0usize;
    for cell in row {
        let advance = grapheme_columns(cell.grapheme());
        write_cell(buffer, column, row_position, cell);
        column += advance;
    }
}

fn write_cell(buffer: &mut CellBuffer, column: usize, row_position: u16, cell: Cell) {
    let Ok(column_position) = u16::try_from(column) else {
        return;
    };
    buffer.set_cell(column_position, row_position, cell);
}

/// Greedy word-wrapper: accumulates words into the current row and flushes it when the
/// next word (plus its separating space) would overflow the target width.
struct LineWrapper<'style> {
    width: usize,
    style: &'style TextStyle,
    rows: Vec<Vec<Cell>>,
    current: Vec<Cell>,
    current_columns: usize,
}

impl<'style> LineWrapper<'style> {
    fn new(width: usize, style: &'style TextStyle) -> Self {
        LineWrapper {
            width,
            style,
            rows: Vec::new(),
            current: Vec::new(),
            current_columns: 0,
        }
    }

    fn push_word(&mut self, word: &str) {
        let cells = graphemes_to_cells(word, self.style);
        let columns = count_columns(&cells);
        if columns > self.width {
            self.push_broken_word(cells);
            return;
        }
        if self.current.is_empty() {
            self.start_row(cells, columns);
            return;
        }
        if self.exceeds_width_with_space(columns) {
            self.flush();
            self.start_row(cells, columns);
            return;
        }
        self.append_with_space(cells, columns);
    }

    /// Break a word wider than the whole line across rows, one grapheme at a time.
    fn push_broken_word(&mut self, cells: Vec<Cell>) {
        for cell in cells {
            self.push_grapheme_breaking(cell);
        }
    }

    fn push_grapheme_breaking(&mut self, cell: Cell) {
        let columns = grapheme_columns(cell.grapheme());
        if self.would_overflow(columns) {
            self.flush();
        }
        self.current.push(cell);
        self.current_columns += columns;
    }

    fn start_row(&mut self, cells: Vec<Cell>, columns: usize) {
        self.current = cells;
        self.current_columns = columns;
    }

    fn append_with_space(&mut self, cells: Vec<Cell>, columns: usize) {
        self.current.push(Cell::new(String::from(" "), self.style));
        self.current_columns += 1;
        self.current.extend(cells);
        self.current_columns += columns;
    }

    fn exceeds_width_with_space(&self, columns: usize) -> bool {
        self.current_columns + 1 + columns > self.width
    }

    fn would_overflow(&self, columns: usize) -> bool {
        !self.current.is_empty() && self.current_columns + columns > self.width
    }

    fn flush(&mut self) {
        let row = std::mem::take(&mut self.current);
        self.rows.push(row);
        self.current_columns = 0;
    }

    fn finish(mut self) -> Vec<Vec<Cell>> {
        if !self.current.is_empty() {
            self.rows.push(self.current);
        }
        self.rows
    }
}
