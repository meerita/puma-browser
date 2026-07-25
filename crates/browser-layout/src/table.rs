// @file crates/browser-layout/src/table.rs
// @description Lays a table out as aligned native columns, or a stacked record view when it does not fit.
// @layer layout
// @created meerita <meerita@icloud.com>

use browser_css::{cascade, Emphasis, TextStyle};
use browser_html::{InlineRun, SemanticNode};

use crate::cell::Cell;
use crate::render::{
    count_columns, graphemes_to_cells, render_children, runs_to_cells, space_cells, wrap_cells,
};
use crate::width::{grapheme_columns, WidthConfig};

/// Blank columns drawn between two native columns so their content stays separated.
const COLUMN_GAP_COLUMNS: usize = 2;

/// Widest a single native column may grow before the table falls back to record view.
///
/// One very wide cell would otherwise force the whole table narrow or off the right edge;
/// past this width the record view reads better than squeezed columns.
const MAX_COLUMN_WIDTH_COLUMNS: usize = 40;

/// Columns each record-view field line is indented under its record heading.
const RECORD_INDENT_COLUMNS: usize = 2;

/// Lay a table out into rows, choosing aligned columns when it fits and a stacked record
/// view when it does not.
///
/// Column widths derive from the widest cell per column. The table renders as native
/// columns only when every column stays within [`MAX_COLUMN_WIDTH_COLUMNS`] and the summed
/// widths plus gaps fit the available `width`; otherwise each row renders as a record so no
/// content is clipped away silently.
///
/// When any cell contains block-level children (nested tables, landmarks, forms), the table
/// is treated as a layout container rather than a data table: each cell's children are
/// rendered as ordinary block content. This handles pages that use `<table>` for page
/// structure rather than for presenting tabular data.
pub(crate) fn render_table(
    rows: &[SemanticNode],
    base: &TextStyle,
    width: usize,
    width_config: &WidthConfig,
) -> Vec<Vec<Cell>> {
    if is_layout_table(rows) {
        return render_layout_table(rows, base, width, width_config);
    }
    let grid = build_grid(rows, base, width_config);
    if grid.is_empty() {
        return Vec::new();
    }
    let widths = column_widths(&grid);
    if table_fits(&widths, width) {
        return render_columns(&grid, &widths, base, width_config);
    }
    render_records(&grid, base, width, width_config)
}

/// A table is a layout table when any cell holds block-level children — nested tables,
/// landmarks, or forms — that cannot be meaningfully flattened into a single text line.
fn is_layout_table(rows: &[SemanticNode]) -> bool {
    rows.iter().any(row_has_block_cell)
}

fn row_has_block_cell(node: &SemanticNode) -> bool {
    let SemanticNode::TableRow { children } = node else {
        return false;
    };
    children.iter().any(cell_has_block_children)
}

fn cell_has_block_children(node: &SemanticNode) -> bool {
    let SemanticNode::TableCell { children, .. } = node else {
        return false;
    };
    children.iter().any(is_block_node)
}

fn is_block_node(node: &SemanticNode) -> bool {
    matches!(
        node,
        SemanticNode::Table { .. }
            | SemanticNode::Landmark { .. }
            | SemanticNode::Form { .. }
            | SemanticNode::Details { .. }
            | SemanticNode::List { .. }
    )
}

/// Render a layout table by treating each cell as an ordinary block container.
///
/// Each row's cells are rendered in order using the normal block rendering path, so nested
/// tables, landmarks, and other block content are fully laid out rather than flattened or
/// dropped.
fn render_layout_table(
    rows: &[SemanticNode],
    base: &TextStyle,
    width: usize,
    width_config: &WidthConfig,
) -> Vec<Vec<Cell>> {
    let mut output: Vec<Vec<Cell>> = Vec::new();
    for row in rows {
        let SemanticNode::TableRow { children } = row else {
            continue;
        };
        for cell in children {
            let SemanticNode::TableCell { children, .. } = cell else {
                continue;
            };
            let cell_style = cascade(base, cell);
            output.extend(render_children(children, width, &cell_style, width_config));
        }
    }
    output
}

/// A table cell reduced to a single styled line of content and its display width.
///
/// `content` already folds the header emphasis into every grapheme, so both layout paths
/// render the same cells without re-styling.
struct GridCell {
    header: bool,
    content: Vec<Cell>,
    columns: usize,
}

struct GridRow {
    cells: Vec<GridCell>,
}

fn build_grid(rows: &[SemanticNode], base: &TextStyle, width_config: &WidthConfig) -> Vec<GridRow> {
    rows.iter()
        .filter_map(|row| grid_row(row, base, width_config))
        .collect()
}

fn grid_row(node: &SemanticNode, base: &TextStyle, width_config: &WidthConfig) -> Option<GridRow> {
    let SemanticNode::TableRow { children } = node else {
        return None;
    };
    let cells = children
        .iter()
        .filter_map(|cell| grid_cell(cell, base, width_config))
        .collect();
    Some(GridRow { cells })
}

fn grid_cell(
    node: &SemanticNode,
    base: &TextStyle,
    width_config: &WidthConfig,
) -> Option<GridCell> {
    let SemanticNode::TableCell {
        header, children, ..
    } = node
    else {
        return None;
    };
    let runs = collect_cell_runs(children);
    let content = runs_to_cells(&runs, &cell_base_style(node, *header, base));
    let columns = count_columns(&content, width_config);
    Some(GridCell {
        header: *header,
        content,
        columns,
    })
}

/// The base style for a cell's content: the cell cascaded from the table's style, with
/// header cells forced bold so a header row stays distinguishable without relying on
/// color.
fn cell_base_style(node: &SemanticNode, header: bool, base: &TextStyle) -> TextStyle {
    let style = cascade(base, node);
    if !header {
        return style;
    }
    TextStyle {
        emphasis: Emphasis::Bold,
        ..style
    }
}

/// Flatten a cell's block content into one line of inline runs for single-line rendering.
///
/// Cells usually hold a single paragraph; when a cell holds several blocks their runs are
/// joined with a separating space so the cell still reads as one line.
fn collect_cell_runs(children: &[SemanticNode]) -> Vec<InlineRun> {
    let mut runs = Vec::new();
    for child in children {
        collect_node_runs(child, &mut runs);
    }
    runs
}

fn collect_node_runs(node: &SemanticNode, runs: &mut Vec<InlineRun>) {
    match node {
        SemanticNode::Paragraph { runs: block, .. } | SemanticNode::Heading { runs: block, .. } => {
            append_block_runs(runs, block)
        }
        SemanticNode::List { children, .. }
        | SemanticNode::ListItem { children, .. }
        | SemanticNode::Quote { children, .. } => append_children_runs(runs, children),
        _ => {}
    }
}

fn append_children_runs(runs: &mut Vec<InlineRun>, children: &[SemanticNode]) {
    for child in children {
        collect_node_runs(child, runs);
    }
}

fn append_block_runs(runs: &mut Vec<InlineRun>, block: &[InlineRun]) {
    if block.is_empty() {
        return;
    }
    if !runs.is_empty() {
        runs.push(InlineRun::plain(" ".to_string()));
    }
    runs.extend(block.iter().cloned());
}

fn column_count(grid: &[GridRow]) -> usize {
    grid.iter().map(|row| row.cells.len()).max().unwrap_or(0)
}

/// Widest content in each column, so a column is exactly as wide as its widest cell.
fn column_widths(grid: &[GridRow]) -> Vec<usize> {
    let mut widths = vec![0usize; column_count(grid)];
    for row in grid {
        widen_for_row(&mut widths, row);
    }
    widths
}

fn widen_for_row(widths: &mut [usize], row: &GridRow) {
    for (index, cell) in row.cells.iter().enumerate() {
        if cell.columns > widths[index] {
            widths[index] = cell.columns;
        }
    }
}

/// A table renders as columns only when no column is wider than the per-column cap and the
/// whole table fits the available width; either failure sends it to the record view.
fn table_fits(widths: &[usize], width: usize) -> bool {
    let within_cap = widths
        .iter()
        .all(|column| *column <= MAX_COLUMN_WIDTH_COLUMNS);
    within_cap && total_table_width(widths) <= width
}

fn total_table_width(widths: &[usize]) -> usize {
    let content: usize = widths.iter().sum();
    let gaps = COLUMN_GAP_COLUMNS * widths.len().saturating_sub(1);
    content + gaps
}

fn render_columns(
    grid: &[GridRow],
    widths: &[usize],
    base: &TextStyle,
    width_config: &WidthConfig,
) -> Vec<Vec<Cell>> {
    grid.iter()
        .map(|row| render_column_row(row, widths, base, width_config))
        .collect()
}

fn render_column_row(
    row: &GridRow,
    widths: &[usize],
    base: &TextStyle,
    width_config: &WidthConfig,
) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    let last_column = widths.len().saturating_sub(1);
    for (index, width) in widths.iter().enumerate() {
        append_column_cell(
            &mut cells,
            row,
            index,
            *width,
            last_column,
            base,
            width_config,
        );
    }
    cells
}

fn append_column_cell(
    cells: &mut Vec<Cell>,
    row: &GridRow,
    index: usize,
    width: usize,
    last_column: usize,
    base: &TextStyle,
    width_config: &WidthConfig,
) {
    if index > 0 {
        cells.extend(space_cells(COLUMN_GAP_COLUMNS, base));
    }
    let content = row.cells.get(index).map(|cell| cell.content.clone());
    let fill = index != last_column;
    cells.extend(fit_cell(
        content.unwrap_or_default(),
        width,
        fill,
        base,
        width_config,
    ));
}

/// Fit a cell's content to its column: clip anything past the column and, for every column
/// but the last, pad with spaces so the following column starts at a fixed offset.
fn fit_cell(
    content: Vec<Cell>,
    width: usize,
    fill: bool,
    base: &TextStyle,
    width_config: &WidthConfig,
) -> Vec<Cell> {
    let mut fitted: Vec<Cell> = Vec::new();
    let mut columns = 0usize;
    for cell in content {
        let advance = grapheme_columns(cell.grapheme(), width_config);
        if columns + advance > width {
            break;
        }
        fitted.push(cell);
        columns += advance;
    }
    if fill {
        fitted.extend(space_cells(width - columns, base));
    }
    fitted
}

/// Render each data row as a record: the first cell as a heading line, then one indented
/// `Label: value` line per remaining cell.
fn render_records(
    grid: &[GridRow],
    base: &TextStyle,
    width: usize,
    width_config: &WidthConfig,
) -> Vec<Vec<Cell>> {
    let labels = header_labels(grid);
    let first_data = if labels.is_some() { 1 } else { 0 };
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for (position, row) in grid.iter().skip(first_data).enumerate() {
        append_record(
            &mut rows,
            row,
            labels.as_deref(),
            base,
            width,
            position,
            width_config,
        );
    }
    rows
}

/// The header row's cell texts, used as record-view field labels, when the first row is a
/// header row.
fn header_labels(grid: &[GridRow]) -> Option<Vec<String>> {
    let first = grid.first()?;
    if !row_is_header(first) {
        return None;
    }
    Some(first.cells.iter().map(cell_text).collect())
}

fn row_is_header(row: &GridRow) -> bool {
    !row.cells.is_empty() && row.cells.iter().all(|cell| cell.header)
}

fn cell_text(cell: &GridCell) -> String {
    cell.content.iter().map(|cell| cell.grapheme()).collect()
}

fn append_record(
    rows: &mut Vec<Vec<Cell>>,
    row: &GridRow,
    labels: Option<&[String]>,
    base: &TextStyle,
    width: usize,
    position: usize,
    width_config: &WidthConfig,
) {
    if position > 0 {
        rows.push(Vec::new());
    }
    let mut cells = row.cells.iter().enumerate();
    if let Some((_, heading)) = cells.next() {
        append_wrapped_field(
            rows,
            Vec::new(),
            &heading.content,
            width,
            base,
            width_config,
        );
    }
    for (index, cell) in cells {
        let prefix = field_prefix(index, labels, base);
        append_wrapped_field(rows, prefix, &cell.content, width, base, width_config);
    }
}

/// The prefix for a record field.
///
/// When the table has a header row, the prefix is `  Label: ` so each field is clearly
/// labelled. Without headers, a plain indent is used instead of a numeric column index,
/// so rows that would otherwise show `  1: title` show `  title` — cleaner for layout
/// tables where column numbers carry no meaning for the reader.
fn field_prefix(index: usize, labels: Option<&[String]>, base: &TextStyle) -> Vec<Cell> {
    match labels.and_then(|labels| labels.get(index)) {
        Some(label) if !label.is_empty() => {
            let text = format!("{}{label}: ", " ".repeat(RECORD_INDENT_COLUMNS));
            graphemes_to_cells(&text, base)
        }
        _ => space_cells(RECORD_INDENT_COLUMNS, base),
    }
}

fn append_wrapped_field(
    rows: &mut Vec<Vec<Cell>>,
    prefix: Vec<Cell>,
    content: &[Cell],
    width: usize,
    base: &TextStyle,
    width_config: &WidthConfig,
) {
    if content.is_empty() {
        return;
    }
    let prefix_columns = count_columns(&prefix, width_config);
    let content_width = record_content_width(width, prefix_columns);
    let wrapped = wrap_cells(content.to_vec(), content_width, width_config);
    if wrapped.is_empty() {
        return;
    }
    push_wrapped_lines(rows, &prefix, prefix_columns, wrapped, base);
}

fn push_wrapped_lines(
    rows: &mut Vec<Vec<Cell>>,
    prefix: &[Cell],
    prefix_columns: usize,
    wrapped: Vec<Vec<Cell>>,
    base: &TextStyle,
) {
    for (index, line) in wrapped.into_iter().enumerate() {
        let mut out = record_line_start(index, prefix, prefix_columns, base);
        out.extend(line);
        rows.push(out);
    }
}

/// The first wrapped line carries the label prefix; continuation lines align under the
/// value with an equal-width blank prefix.
fn record_line_start(
    index: usize,
    prefix: &[Cell],
    prefix_columns: usize,
    base: &TextStyle,
) -> Vec<Cell> {
    if index == 0 {
        return prefix.to_vec();
    }
    space_cells(prefix_columns, base)
}

fn record_content_width(width: usize, prefix_columns: usize) -> usize {
    if width > prefix_columns {
        return width - prefix_columns;
    }
    1
}
