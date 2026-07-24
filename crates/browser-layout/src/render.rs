// @file crates/browser-layout/src/render.rs
// @description Lays a Document's node tree out into a width-correct terminal cell buffer.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::{cascade, computed_run_style, DisplayMode, TextStyle, TextTransform};
use browser_html::{Document, InlineRun, SemanticNode};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::cell::{Cell, CellBuffer};
use crate::error::LayoutError;
use crate::table::render_table;

/// Columns a block quote's text is indented from the left margin.
const QUOTE_INDENT_COLUMNS: usize = 2;

/// The character repeated across a full row to draw a horizontal separator.
const SEPARATOR_GRAPHEME: &str = "─";

/// The blank field drawn for an editable input placeholder.
const INPUT_BLANK: &str = "____";

/// The masked field drawn for a password input, so a value is never revealed.
const INPUT_MASK: &str = "••••";

/// The marker drawn after a select placeholder to signal a dropdown control.
const SELECT_MARKER: &str = "▾";

/// Lay a document out into a cell buffer sized to `width` columns.
///
/// Each node is styled by the [`cascade`] and turned into laid-out rows: text blocks
/// word-wrap to the width, code and preformatted blocks render verbatim and clip,
/// container nodes recurse into their children, and spacing is applied as blank rows
/// between blocks. The rows are then written into a blank buffer whose height is the
/// total row count. The root nodes inherit from the default style, so the cascade starts
/// from a clean context.
pub fn render_document(document: &Document, width: u16) -> Result<CellBuffer, LayoutError> {
    if width == 0 {
        return Err(LayoutError::ZeroWidth);
    }
    let width_columns = usize::from(width);
    let rows = render_children(document.children(), width_columns, &TextStyle::default());
    build_buffer(rows, width)
}

/// Lay a sequence of sibling nodes out into rows, applying each node's own spacing.
///
/// `inherited` is the computed style of the container these nodes sit in; each node's own
/// style cascades from it, so inherited properties like color flow down into children.
fn render_children(
    children: &[SemanticNode],
    width: usize,
    inherited: &TextStyle,
) -> Vec<Vec<Cell>> {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for node in children {
        append_node_rows(&mut rows, node, width, inherited);
    }
    rows
}

/// Append a node's spacing and content rows to the running row list.
///
/// A node the cascade computes as hidden (`display: none` or `visibility: hidden`)
/// contributes no rows and its subtree is not walked. A structural node with no content
/// contributes nothing, not even its spacing, so invisible nodes never leave stray blank
/// rows behind.
fn append_node_rows(
    rows: &mut Vec<Vec<Cell>>,
    node: &SemanticNode,
    width: usize,
    inherited: &TextStyle,
) {
    let style = cascade(inherited, node);
    if is_hidden(&style) {
        return;
    }
    let content = node_rows(node, &style, width);
    if content.is_empty() {
        return;
    }
    append_blank_rows(rows, style.spacing_before);
    rows.extend(content);
    append_blank_rows(rows, style.spacing_after);
}

/// Whether a computed style removes the node from the rendered output.
///
/// `display: none` sets the display mode to hidden; `visibility: hidden` clears the
/// visible flag. Either keeps the node and its subtree from contributing any rows.
fn is_hidden(style: &TextStyle) -> bool {
    style.display_mode == DisplayMode::Hidden || !style.visible
}

fn append_blank_rows(rows: &mut Vec<Vec<Cell>>, count: u16) {
    for _ in 0..count {
        rows.push(Vec::new());
    }
}

/// Produce the content rows for a single node, before spacing is applied.
///
/// Container nodes recurse into their children through [`render_children`]; text blocks
/// lay out each inline run with its own emphasis and link styling and word-wrap the
/// result.
fn node_rows(node: &SemanticNode, style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    match node {
        SemanticNode::Heading { runs, .. } => wrap_runs(runs, style, width),
        SemanticNode::Paragraph { runs, .. } => wrap_runs(runs, style, width),
        SemanticNode::Quote { children, .. } => render_quote(children, style, width),
        SemanticNode::List {
            ordered, children, ..
        } => render_list(*ordered, children, width, style),
        SemanticNode::ListItem { children, .. } => render_children(children, width, style),
        SemanticNode::Table { children } => render_table(children, style, width),
        SemanticNode::CodeBlock { text } | SemanticNode::PreformattedBlock { text } => {
            render_verbatim(text, style, width)
        }
        SemanticNode::Separator => vec![separator_row(style, width)],
        SemanticNode::Warning { message } => single_row(clip_line(message, style, width)),
        SemanticNode::ImagePlaceholder { alt, title, .. } => {
            single_row(clip_line(&image_label(alt, title.as_deref()), style, width))
        }
        SemanticNode::Figure { children, caption } => {
            render_figure(children, caption, style, width)
        }
        SemanticNode::Details { children, .. } => render_children(children, width, style),
        SemanticNode::Summary { runs, .. } => wrap_runs(runs, style, width),
        SemanticNode::Landmark { children, .. } => render_children(children, width, style),
        SemanticNode::Form { children } => render_children(children, width, style),
        SemanticNode::Input {
            label, sensitive, ..
        } => single_row(clip_line(
            &input_placeholder(label.as_deref(), *sensitive),
            style,
            width,
        )),
        SemanticNode::Select { label, options } => single_row(clip_line(
            &select_placeholder(label.as_deref(), options),
            style,
            width,
        )),
        SemanticNode::Button { runs, .. } => render_button(runs, style, width),
        SemanticNode::EmbeddedContent { label } => {
            single_row(clip_line(&embedded_placeholder(label), style, width))
        }
        SemanticNode::TableRow { .. } | SemanticNode::TableCell { .. } => Vec::new(),
    }
}

/// Turn a text block's inline runs into styled cells and wrap them to the width.
///
/// Each run's graphemes carry that run's own style, computed from the node's base style
/// (`base`) folded with the run's emphasis and link. Wrapping then treats the whole
/// block as one styled stream, so a word split across run boundaries wraps as a single
/// unit and run boundaries never change where lines break.
fn wrap_runs(runs: &[InlineRun], base: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    wrap_cells(runs_to_cells(runs, base), width)
}

pub(crate) fn runs_to_cells(runs: &[InlineRun], base: &TextStyle) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    for run in runs {
        let style = computed_run_style(*base, run);
        let text = transform_text(&run.text, style.text_transform);
        cells.extend(graphemes_to_cells(&text, &style));
    }
    cells
}

/// Apply a run's `text-transform` to its text before it becomes cells.
///
/// Capitalization uppercases the first character of each whitespace-separated word and
/// leaves the rest untouched, which is a close reading of CSS `capitalize` without full
/// word-boundary detection.
fn transform_text(text: &str, transform: TextTransform) -> String {
    match transform {
        TextTransform::None => text.to_string(),
        TextTransform::Uppercase => text.to_uppercase(),
        TextTransform::Lowercase => text.to_lowercase(),
        TextTransform::Capitalize => capitalize_words(text),
    }
}

fn capitalize_words(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut at_word_start = true;
    for character in text.chars() {
        if character.is_whitespace() {
            at_word_start = true;
            output.push(character);
            continue;
        }
        push_cased(&mut output, character, at_word_start);
        at_word_start = false;
    }
    output
}

fn push_cased(output: &mut String, character: char, uppercase: bool) {
    if uppercase {
        output.extend(character.to_uppercase());
        return;
    }
    output.push(character);
}

/// Wrap a block's styled cells into rows no wider than `width` display columns.
pub(crate) fn wrap_cells(cells: Vec<Cell>, width: usize) -> Vec<Vec<Cell>> {
    let mut wrapper = LineWrapper::new(width);
    for token in tokenize_line(cells) {
        wrapper.push_token(token);
    }
    wrapper.finish()
}

/// Split a styled cell stream into words and the single spaces that separate them.
///
/// A word is a maximal span of non-space cells, which may hold graphemes from several
/// runs when no whitespace falls between them. Block text is already whitespace-collapsed,
/// so each separator is exactly one space.
fn tokenize_line(cells: Vec<Cell>) -> Vec<LineToken> {
    let mut tokens: Vec<LineToken> = Vec::new();
    let mut word: Vec<Cell> = Vec::new();
    for cell in cells {
        if cell.grapheme() != " " {
            word.push(cell);
            continue;
        }
        flush_word(&mut word, &mut tokens);
        tokens.push(LineToken::Space(cell));
    }
    flush_word(&mut word, &mut tokens);
    tokens
}

fn flush_word(word: &mut Vec<Cell>, tokens: &mut Vec<LineToken>) {
    if word.is_empty() {
        return;
    }
    tokens.push(LineToken::Word(std::mem::take(word)));
}

/// Render a block quote: lay its children out into the indented content width, then push
/// each resulting row right by the quote indent.
fn render_quote(children: &[SemanticNode], style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    let indent = quote_indent(width);
    let content_width = width - indent;
    render_children(children, content_width, style)
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

/// Render a list: give each item a marker (a bullet for an unordered list, a running
/// `N. ` number for an ordered list), then lay the item's block children out and indent
/// them under the item text. Ordered numbering is 1-based within this list; a nested
/// list is rendered by its own call, so its numbering restarts from one.
fn render_list(
    ordered: bool,
    items: &[SemanticNode],
    width: usize,
    inherited: &TextStyle,
) -> Vec<Vec<Cell>> {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut ordinal = 1usize;
    for item in items {
        if !is_list_item(item) {
            continue;
        }
        append_list_item_rows(
            &mut rows,
            item,
            &list_marker(ordered, ordinal),
            width,
            inherited,
        );
        ordinal += 1;
    }
    rows
}

/// Lay one list item out: render its block children into the width left after the marker,
/// prefix the first row with the marker, and indent continuation and nested rows to align
/// under the item text.
fn append_list_item_rows(
    rows: &mut Vec<Vec<Cell>>,
    item: &SemanticNode,
    marker: &str,
    width: usize,
    inherited: &TextStyle,
) {
    let SemanticNode::ListItem { children, .. } = item else {
        return;
    };
    let style = cascade(inherited, item);
    let marker_cells = graphemes_to_cells(marker, &style);
    let marker_columns = count_columns(&marker_cells);
    let content_width = list_content_width(width, marker_columns);
    for (index, row) in render_children(children, content_width, &style)
        .into_iter()
        .enumerate()
    {
        rows.push(decorate_list_row(
            index,
            row,
            &marker_cells,
            marker_columns,
            &style,
        ));
    }
}

fn is_list_item(node: &SemanticNode) -> bool {
    matches!(node, SemanticNode::ListItem { .. })
}

/// The marker drawn before a list item: a bullet and a space for an unordered list, or
/// the item's 1-based position followed by `. ` for an ordered list.
fn list_marker(ordered: bool, ordinal: usize) -> String {
    if ordered {
        return format!("{ordinal}. ");
    }
    String::from("• ")
}

fn list_content_width(width: usize, marker_columns: usize) -> usize {
    if width > marker_columns {
        return width - marker_columns;
    }
    width
}

fn decorate_list_row(
    index: usize,
    row: Vec<Cell>,
    marker_cells: &[Cell],
    marker_columns: usize,
    style: &TextStyle,
) -> Vec<Cell> {
    let mut decorated = if index == 0 {
        marker_cells.to_vec()
    } else {
        space_cells(marker_columns, style)
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

/// The bracketed placeholder for an image.
///
/// The alt text names the image; a distinct title is appended in parentheses so both are
/// visible. An image with neither alt nor a useful title falls back to a generic label.
fn image_label(alt: &str, title: Option<&str>) -> String {
    let base = if alt.is_empty() { "image" } else { alt };
    match title {
        Some(title) if !title.is_empty() && title != base => format!("[{base} ({title})]"),
        _ => format!("[{base}]"),
    }
}

/// Render a figure: its content rows followed by the caption on its own wrapped lines.
fn render_figure(
    children: &[SemanticNode],
    caption: &Option<Vec<InlineRun>>,
    style: &TextStyle,
    width: usize,
) -> Vec<Vec<Cell>> {
    let mut rows = render_children(children, width, style);
    if let Some(runs) = caption {
        rows.extend(wrap_runs(runs, style, width));
    }
    rows
}

/// The bracketed placeholder for an inert input control.
///
/// A password input renders a fixed mask, never its value; every other input renders a
/// blank field. The label, when known, precedes the field.
fn input_placeholder(label: Option<&str>, sensitive: bool) -> String {
    let field = if sensitive { INPUT_MASK } else { INPUT_BLANK };
    match label {
        Some(label) => format!("[{label}: {field}]"),
        None => format!("[{field}]"),
    }
}

/// The bracketed placeholder for an inert select control, showing its first option.
fn select_placeholder(label: Option<&str>, options: &[String]) -> String {
    let selected = options.first().map(String::as_str).unwrap_or("");
    match label {
        Some(label) => format!("[{label}: {selected} {SELECT_MARKER}]"),
        None => format!("[{selected} {SELECT_MARKER}]"),
    }
}

/// Render a button as its label wrapped in brackets, keeping the label's inline styling.
fn render_button(runs: &[InlineRun], style: &TextStyle, width: usize) -> Vec<Vec<Cell>> {
    let mut cells = graphemes_to_cells("[ ", style);
    cells.extend(runs_to_cells(runs, style));
    cells.extend(graphemes_to_cells(" ]", style));
    single_row(clip_cells(cells, width))
}

fn embedded_placeholder(label: &str) -> String {
    format!("[Embedded: {label}]")
}

/// Truncate a styled cell run before the first grapheme that would cross `width`.
fn clip_cells(cells: Vec<Cell>, width: usize) -> Vec<Cell> {
    let mut clipped: Vec<Cell> = Vec::new();
    let mut columns = 0usize;
    for cell in cells {
        let advance = grapheme_columns(cell.grapheme());
        if columns + advance > width {
            break;
        }
        columns += advance;
        clipped.push(cell);
    }
    clipped
}

fn single_row(row: Vec<Cell>) -> Vec<Vec<Cell>> {
    vec![row]
}

fn indent_row(row: Vec<Cell>, indent: usize, style: &TextStyle) -> Vec<Cell> {
    let mut indented = space_cells(indent, style);
    indented.extend(row);
    indented
}

pub(crate) fn space_cells(count: usize, style: &TextStyle) -> Vec<Cell> {
    (0..count)
        .map(|_| Cell::new(String::from(" "), style))
        .collect()
}

pub(crate) fn graphemes_to_cells(text: &str, style: &TextStyle) -> Vec<Cell> {
    text.graphemes(true)
        .map(|grapheme| Cell::new(grapheme.to_string(), style))
        .collect()
}

pub(crate) fn count_columns(cells: &[Cell]) -> usize {
    cells
        .iter()
        .map(|cell| grapheme_columns(cell.grapheme()))
        .sum()
}

/// The number of terminal columns a single grapheme cluster advances.
///
/// A combining mark contributes zero, so a base-plus-mark cluster stays one column; a
/// CJK or other wide grapheme advances two.
pub(crate) fn grapheme_columns(grapheme: &str) -> usize {
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

/// A unit of a text block's wrappable content: a word or the space that separates two.
enum LineToken {
    Word(Vec<Cell>),
    Space(Cell),
}

/// Greedy word-wrapper: accumulates words into the current row and flushes it when the
/// next word (plus its separating space) would overflow the target width. A word carries
/// its graphemes' own styles, so wrapping never restyles content; the separating space is
/// the source space cell, held back until the following word joins the same row.
struct LineWrapper {
    width: usize,
    rows: Vec<Vec<Cell>>,
    current: Vec<Cell>,
    current_columns: usize,
    pending_space: Option<Cell>,
}

impl LineWrapper {
    fn new(width: usize) -> LineWrapper {
        LineWrapper {
            width,
            rows: Vec::new(),
            current: Vec::new(),
            current_columns: 0,
            pending_space: None,
        }
    }

    fn push_token(&mut self, token: LineToken) {
        match token {
            LineToken::Word(cells) => self.push_word(cells),
            LineToken::Space(cell) => self.pending_space = Some(cell),
        }
    }

    fn push_word(&mut self, cells: Vec<Cell>) {
        let columns = count_columns(&cells);
        if columns > self.width {
            self.pending_space = None;
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
        self.pending_space = None;
    }

    fn append_with_space(&mut self, cells: Vec<Cell>, columns: usize) {
        if let Some(space) = self.pending_space.take() {
            self.current.push(space);
            self.current_columns += 1;
        }
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
        self.pending_space = None;
    }

    fn finish(mut self) -> Vec<Vec<Cell>> {
        if !self.current.is_empty() {
            self.rows.push(self.current);
        }
        self.rows
    }
}
