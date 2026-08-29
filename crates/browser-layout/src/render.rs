// @file crates/browser-layout/src/render.rs
// @description Lays a Document's node tree out into a width-correct terminal cell buffer.
// @layer layout
// @created meerita <meerita@icloud.com>

use std::borrow::Cow;

use browser_css::{cascade, computed_run_style, Color, DisplayMode, TextStyle, TextTransform};
use browser_html::{
    Document, InlineRun, InputElement, InputKind, NodeId, SelectOption, SemanticNode,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::cell::{AnchorSpan, Cell, CellBuffer, LinkKind, LinkSpan};
use crate::error::LayoutError;
use crate::field_overlay::{FieldOverlay, FieldRenderValue};
use crate::field_span::{FieldSpan, FieldSpanKind};
use crate::table::render_table;
use crate::width::{emoji_replacement, grapheme_columns, WidthConfig};

/// Columns a block quote's text is indented from the left margin.
const QUOTE_INDENT_COLUMNS: usize = 2;

/// The character repeated across the centered rule drawn for a horizontal separator.
const SEPARATOR_GRAPHEME: &str = "━";

/// The fraction of the content width a horizontal separator's rule spans, as a percentage.
const RULE_WIDTH_PERCENT: usize = 30;

/// The narrowest a separator's rule is drawn, in columns, so it stays visible on a narrow
/// terminal. Capped at the content width when the terminal is narrower than this.
const MIN_RULE_WIDTH: usize = 8;

/// The widest a separator's rule is drawn, in columns, so it does not stretch across an
/// ultrawide terminal.
const MAX_RULE_WIDTH: usize = 40;

/// The blank field drawn for an editable input placeholder.
const INPUT_BLANK: &str = "____";

/// The masked field drawn for a password input, so a value is never revealed.
const INPUT_MASK: &str = "••••";

/// The character repeated to mask a sensitive field's live length, one per typed
/// character, so a password's length is visible but never its value.
const MASK_CHARACTER: &str = "•";

/// The marker drawn after a select placeholder to signal a dropdown control.
const SELECT_MARKER: &str = "▾";

/// The checkbox/radio marker drawn for a checked control, matching the settings panel's
/// checkbox glyph.
const CHECKBOX_CHECKED: &str = "[x]";

/// The checkbox/radio marker drawn for an unchecked control.
const CHECKBOX_UNCHECKED: &str = "[ ]";

/// Lay a document out into a cell buffer sized to `width` columns.
///
/// Each node is styled by the [`cascade`] and turned into laid-out rows: text blocks
/// word-wrap to the width, code and preformatted blocks render verbatim and clip,
/// container nodes recurse into their children, and spacing is applied as blank rows
/// between blocks. The rows are then written into a blank buffer whose height is the
/// total row count. The root nodes inherit from the default style, so the cascade starts
/// from a clean context. `width_config` governs how every grapheme is measured into
/// columns, so wrapping and truncation agree on width throughout.
pub fn render_document(
    document: &Document,
    width: u16,
    width_config: &WidthConfig,
    field_overlay: Option<&FieldOverlay>,
) -> Result<CellBuffer, LayoutError> {
    if width == 0 {
        return Err(LayoutError::ZeroWidth);
    }
    let width_columns = usize::from(width);
    let mut pass = RenderPass::new(width_config, field_overlay);
    let mut rows = render_children(
        document.children(),
        width_columns,
        &TextStyle {
            foreground: Some(Color::White),
            ..TextStyle::default()
        },
        &mut pass,
    );
    mark_trailing_anchors(&mut rows, &mut pass.pending_anchors);
    build_buffer(rows, width, width_config)
}

/// The state carried through the whole block recursion.
///
/// The width configuration and the field overlay are read at every level, and the anchors
/// awaiting placement are filled in at one level and consumed at another, so the three
/// travel together rather than as separate parameters on every signature.
pub(crate) struct RenderPass<'a> {
    width_config: &'a WidthConfig,
    field_overlay: Option<&'a FieldOverlay>,
    /// Fragment targets declared but not yet bound to a row.
    ///
    /// A target names the position the document declared it at. Which row that position
    /// falls on is unknown until the next content row is laid out, so the names wait here
    /// until one appears.
    pending_anchors: Vec<String>,
}

impl<'a> RenderPass<'a> {
    pub(crate) fn new(
        width_config: &'a WidthConfig,
        field_overlay: Option<&'a FieldOverlay>,
    ) -> RenderPass<'a> {
        RenderPass {
            width_config,
            field_overlay,
            pending_anchors: Vec::new(),
        }
    }

    pub(crate) fn width_config(&self) -> &'a WidthConfig {
        self.width_config
    }

    pub(crate) fn field_overlay(&self) -> Option<&'a FieldOverlay> {
        self.field_overlay
    }
}

/// Append each name not already awaiting placement, preserving declaration order.
fn push_unique_anchor_names(destination: &mut Vec<String>, names: &[String]) {
    for name in names {
        if destination.iter().any(|present| present == name) {
            continue;
        }
        destination.push(name.clone());
    }
}

/// Bind the anchors awaiting placement to the first row at or after `from` that holds a
/// cell, then clear them.
///
/// Blank rows hold no cell and carry no anchor, so the scan skips them: a target must name
/// the row its content starts on, not the spacing above it. When no row in range holds a
/// cell the names stay pending and bind to whatever content comes next.
fn mark_pending_anchors(rows: &mut [Vec<Cell>], from: usize, pending: &mut Vec<String>) {
    if pending.is_empty() {
        return;
    }
    let Some(row) = rows.iter_mut().skip(from).find(|row| !row.is_empty()) else {
        return;
    };
    let Some(cell) = row.first_mut() else {
        return;
    };
    cell.push_anchor_names(pending);
    pending.clear();
}

/// Bind anchors still awaiting placement once the document ends to its last content row.
///
/// A target declared after everything the document renders would otherwise resolve to
/// nothing, so a link to the final section reports a miss instead of scrolling to the end.
fn mark_trailing_anchors(rows: &mut [Vec<Cell>], pending: &mut Vec<String>) {
    if pending.is_empty() {
        return;
    }
    let Some(row) = rows.iter_mut().rev().find(|row| !row.is_empty()) else {
        return;
    };
    let Some(cell) = row.first_mut() else {
        return;
    };
    cell.push_anchor_names(pending);
    pending.clear();
}

/// Lay a sequence of sibling nodes out into rows, applying each node's own spacing.
///
/// `inherited` is the computed style of the container these nodes sit in; each node's own
/// style cascades from it, so inherited properties like color flow down into children.
pub(crate) fn render_children(
    children: &[SemanticNode],
    width: usize,
    inherited: &TextStyle,
    pass: &mut RenderPass<'_>,
) -> Vec<Vec<Cell>> {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    for node in children {
        append_node_rows(&mut rows, node, width, inherited, pass);
    }
    rows
}

/// Append a node's spacing and content rows to the running row list.
///
/// A node the cascade computes as hidden (`display: none` or `visibility: hidden`)
/// contributes no rows and its subtree is not walked. A structural node with no content
/// contributes nothing, not even its spacing, so invisible nodes never leave stray blank
/// rows behind.
///
/// A fragment target contributes no rows either, so it never reaches the cascade. Its
/// names wait instead for the first node after it that does produce content, which is
/// where a reader following the link expects to land: a hidden node or an image
/// placeholder renders nothing, so the target belongs to the content that takes its place.
fn append_node_rows(
    rows: &mut Vec<Vec<Cell>>,
    node: &SemanticNode,
    width: usize,
    inherited: &TextStyle,
    pass: &mut RenderPass<'_>,
) {
    if let SemanticNode::AnchorTarget { names } = node {
        push_unique_anchor_names(&mut pass.pending_anchors, names);
        return;
    }
    let style = cascade(inherited, node);
    if is_hidden(&style) {
        return;
    }
    let content = node_rows(node, &style, width, pass);
    if content.is_empty() {
        return;
    }
    append_blank_rows(rows, style.spacing_before);
    let first_content_row = rows.len();
    rows.extend(content);
    mark_pending_anchors(rows, first_content_row, &mut pass.pending_anchors);
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
fn node_rows(
    node: &SemanticNode,
    style: &TextStyle,
    width: usize,
    pass: &mut RenderPass<'_>,
) -> Vec<Vec<Cell>> {
    let width_config = pass.width_config();
    match node {
        SemanticNode::Heading { runs, .. } => wrap_runs(runs, style, width, width_config),
        SemanticNode::Paragraph { runs, .. } => wrap_runs(runs, style, width, width_config),
        SemanticNode::Quote { children, .. } => render_quote(children, style, width, pass),
        SemanticNode::List {
            ordered, children, ..
        } => render_list(*ordered, children, width, style, pass),
        SemanticNode::ListItem { children, .. } => render_children(children, width, style, pass),
        SemanticNode::Table { children } => render_table(children, style, width, pass),
        SemanticNode::CodeBlock { text } | SemanticNode::PreformattedBlock { text } => {
            render_verbatim(text, style, width, width_config)
        }
        SemanticNode::Separator => vec![separator_row(style, width)],
        SemanticNode::Warning { message } => {
            single_row(clip_line(message, style, width, width_config))
        }
        SemanticNode::ImagePlaceholder { .. } => Vec::new(),
        SemanticNode::Figure { children, caption } => {
            render_figure(children, caption, style, width, pass)
        }
        SemanticNode::Details { children, .. } => render_children(children, width, style, pass),
        SemanticNode::Summary { runs, .. } => wrap_runs(runs, style, width, width_config),
        SemanticNode::Landmark { children, .. } => render_children(children, width, style, pass),
        SemanticNode::Form(form) => render_children(&form.children, width, style, pass),
        SemanticNode::Input(input) => {
            render_input(input, style, width, width_config, pass.field_overlay())
        }
        SemanticNode::Textarea(textarea) => {
            let overlay_value = pass
                .field_overlay()
                .and_then(|overlay| overlay.get(textarea.id));
            mark_field(
                single_row(clip_line(
                    &input_placeholder(textarea.label.as_deref(), false, overlay_value),
                    style,
                    width,
                    width_config,
                )),
                textarea.id,
                FieldSpanKind::Textarea,
            )
        }
        SemanticNode::Select(select) => {
            let overlay_value = pass
                .field_overlay()
                .and_then(|overlay| overlay.get(select.id));
            mark_field(
                single_row(clip_line(
                    &select_placeholder(select.label.as_deref(), &select.options, overlay_value),
                    style,
                    width,
                    width_config,
                )),
                select.id,
                FieldSpanKind::Select,
            )
        }
        SemanticNode::Button(button) => mark_field(
            render_button(&button.runs, style, width, width_config),
            button.id,
            FieldSpanKind::Button,
        ),
        SemanticNode::EmbeddedContent { label } => single_row(clip_line(
            &embedded_placeholder(label),
            style,
            width,
            width_config,
        )),
        SemanticNode::TableRow { .. }
        | SemanticNode::TableCell { .. }
        | SemanticNode::AnchorTarget { .. } => Vec::new(),
    }
}

/// Turn a text block's inline runs into styled cells and wrap them to the width.
///
/// Each run's graphemes carry that run's own style, computed from the node's base style
/// (`base`) folded with the run's emphasis and link. Wrapping then treats the whole
/// block as one styled stream, so a word split across run boundaries wraps as a single
/// unit and run boundaries never change where lines break.
fn wrap_runs(
    runs: &[InlineRun],
    base: &TextStyle,
    width: usize,
    width_config: &WidthConfig,
) -> Vec<Vec<Cell>> {
    wrap_cells(runs_to_cells(runs, base), width, width_config)
}

pub(crate) fn runs_to_cells(runs: &[InlineRun], base: &TextStyle) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    // A run whose text is empty produces no cell to mark, so its names move on to the
    // first cell that does appear rather than being dropped.
    let mut pending_anchors: Vec<String> = Vec::new();
    for run in runs {
        let style = computed_run_style(*base, run);
        let text = transform_text(&run.text, style.text_transform);
        let start = cells.len();
        cells.extend(graphemes_to_cells(&text, &style));
        if let Some(url) = &run.link {
            for cell in &mut cells[start..] {
                cell.set_link_url(url.clone());
            }
        }
        if let Some(url) = &run.citation {
            for cell in &mut cells[start..] {
                cell.set_citation_url(url.clone());
            }
        }
        push_unique_anchor_names(&mut pending_anchors, &run.anchors);
        let Some(first) = cells.get_mut(start) else {
            continue;
        };
        first.push_anchor_names(&pending_anchors);
        pending_anchors.clear();
    }
    cells
}

/// Apply a run's `text-transform` to its text before it becomes cells.
///
/// Capitalization uppercases the first character of each whitespace-separated word and
/// leaves the rest untouched, which is a close reading of CSS `capitalize` without full
/// word-boundary detection.
fn transform_text(text: &str, transform: TextTransform) -> Cow<'_, str> {
    match transform {
        TextTransform::None => Cow::Borrowed(text),
        TextTransform::Uppercase => Cow::Owned(text.to_uppercase()),
        TextTransform::Lowercase => Cow::Owned(text.to_lowercase()),
        TextTransform::Capitalize => Cow::Owned(capitalize_words(text)),
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
pub(crate) fn wrap_cells(
    cells: Vec<Cell>,
    width: usize,
    width_config: &WidthConfig,
) -> Vec<Vec<Cell>> {
    let mut wrapper = LineWrapper::new(width, width_config);
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
///
/// Trailing blank rows from the last child are stripped before indenting so the quote's
/// own spacing_after is the only blank row that follows the quoted content.
fn render_quote(
    children: &[SemanticNode],
    style: &TextStyle,
    width: usize,
    pass: &mut RenderPass<'_>,
) -> Vec<Vec<Cell>> {
    let indent = quote_indent(width);
    let content_width = width - indent;
    trim_trailing_blanks(render_children(children, content_width, style, pass))
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
    pass: &mut RenderPass<'_>,
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
            pass,
        );
        ordinal += 1;
    }
    rows
}

/// Lay one list item out: render its block children into the width left after the marker,
/// prefix the first row with the marker, and indent continuation and nested rows to align
/// under the item text.
///
/// Trailing blank rows from the last child are stripped so consecutive list items run
/// tight without gaps between them. The blank that follows the whole list comes from the
/// list node's own spacing_after.
fn append_list_item_rows(
    rows: &mut Vec<Vec<Cell>>,
    item: &SemanticNode,
    marker: &str,
    width: usize,
    inherited: &TextStyle,
    pass: &mut RenderPass<'_>,
) {
    let SemanticNode::ListItem { children, .. } = item else {
        return;
    };
    let style = cascade(inherited, item);
    let marker_cells = graphemes_to_cells(marker, &style);
    let marker_columns = count_columns(&marker_cells, pass.width_config());
    let content_width = list_content_width(width, marker_columns);
    let children_rows =
        trim_trailing_blanks(render_children(children, content_width, &style, pass));
    for (index, row) in children_rows.into_iter().enumerate() {
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
fn render_verbatim(
    text: &str,
    style: &TextStyle,
    width: usize,
    width_config: &WidthConfig,
) -> Vec<Vec<Cell>> {
    text.split('\n')
        .map(|line| clip_line(line, style, width, width_config))
        .collect()
}

/// Turn a single line into cells, stopping before any grapheme that would cross `width`.
fn clip_line(line: &str, style: &TextStyle, width: usize, width_config: &WidthConfig) -> Vec<Cell> {
    let mut cells: Vec<Cell> = Vec::new();
    let mut columns = 0usize;
    for grapheme in line.graphemes(true) {
        let advance = grapheme_columns(grapheme, width_config);
        if columns + advance > width {
            break;
        }
        cells.push(Cell::new(grapheme.to_string(), style));
        columns += advance;
    }
    cells
}

/// Draw a horizontal separator as a rule centered on the content column.
///
/// The rule spans a clamped fraction of the content width (see `rule_width`) and is padded
/// on the left so it sits centered. Only the leading pad and the rule are emitted; rows in
/// this engine are ragged, so no trailing pad is needed.
fn separator_row(style: &TextStyle, width: usize) -> Vec<Cell> {
    let rule_width = rule_width(width);
    let left_pad = (width - rule_width) / 2;
    let mut cells = space_cells(left_pad, style);
    cells.extend((0..rule_width).map(|_| Cell::new(String::from(SEPARATOR_GRAPHEME), style)));
    cells
}

/// The width in columns of a separator's rule for a given content width.
///
/// Thirty percent of the content width, clamped to a floor and ceiling so the rule stays
/// proportional in the common range and bounded at the extremes, then capped at the content
/// width itself so a terminal narrower than the floor cannot produce a rule wider than the
/// row.
fn rule_width(width: usize) -> usize {
    (width * RULE_WIDTH_PERCENT / 100)
        .clamp(MIN_RULE_WIDTH, MAX_RULE_WIDTH)
        .min(width)
}

/// Render a figure: its content rows followed by the caption on its own wrapped lines.
fn render_figure(
    children: &[SemanticNode],
    caption: &Option<Vec<InlineRun>>,
    style: &TextStyle,
    width: usize,
    pass: &mut RenderPass<'_>,
) -> Vec<Vec<Cell>> {
    let mut rows = render_children(children, width, style, pass);
    if let Some(runs) = caption {
        rows.extend(wrap_runs(runs, style, width, pass.width_config()));
    }
    rows
}

/// Render an `<input>` control: a hidden input contributes no row, matching the
/// `ImagePlaceholder` convention that an invisible node renders as nothing; every other
/// kind renders a single marked placeholder row, substituting a live overlay value for
/// its static placeholder when the overlay carries one for this control.
fn render_input(
    input: &InputElement,
    style: &TextStyle,
    width: usize,
    width_config: &WidthConfig,
    field_overlay: Option<&FieldOverlay>,
) -> Vec<Vec<Cell>> {
    if input.kind == InputKind::Hidden {
        return Vec::new();
    }
    let overlay_value = field_overlay.and_then(|overlay| overlay.get(input.id));
    let text = input_display_text(input.label.as_deref(), input.sensitive, overlay_value);
    mark_field(
        single_row(clip_line(&text, style, width, width_config)),
        input.id,
        FieldSpanKind::Input,
    )
}

/// The text shown for an `<input>` control. A live `Checked` overlay renders the
/// checkbox/radio marker beside the label instead of the bracketed placeholder; every
/// other case renders [`input_placeholder`].
fn input_display_text(
    label: Option<&str>,
    sensitive: bool,
    overlay: Option<&FieldRenderValue>,
) -> String {
    if let Some(FieldRenderValue::Checked(checked)) = overlay {
        return checkbox_display(label, *checked);
    }
    input_placeholder(label, sensitive, overlay)
}

/// The checked/unchecked marker for a checkbox or radio input, matching the settings
/// panel's checkbox glyph, followed by the label when one is known.
fn checkbox_display(label: Option<&str>, checked: bool) -> String {
    let marker = if checked {
        CHECKBOX_CHECKED
    } else {
        CHECKBOX_UNCHECKED
    };
    match label {
        Some(label) => format!("{marker} {label}"),
        None => marker.to_string(),
    }
}

/// The bracketed placeholder for an inert input or textarea control.
///
/// A `Text` overlay value replaces the field with the live typed text; a `MaskedLength`
/// overlay value replaces it with that many mask characters, never the value itself. With
/// no matching overlay value, a password input renders a fixed mask and every other
/// control renders a blank field. The label, when known, precedes the field.
fn input_placeholder(
    label: Option<&str>,
    sensitive: bool,
    overlay: Option<&FieldRenderValue>,
) -> String {
    let field = input_field_text(sensitive, overlay);
    match label {
        Some(label) => format!("[{label}: {field}]"),
        None => format!("[{field}]"),
    }
}

fn input_field_text(sensitive: bool, overlay: Option<&FieldRenderValue>) -> String {
    match overlay {
        Some(FieldRenderValue::Text(value)) => value.clone(),
        Some(FieldRenderValue::MaskedLength(length)) => MASK_CHARACTER.repeat(*length),
        Some(FieldRenderValue::Checked(_)) | Some(FieldRenderValue::SelectedLabels(_)) | None => {
            default_field_text(sensitive)
        }
    }
}

fn default_field_text(sensitive: bool) -> String {
    if sensitive {
        INPUT_MASK.to_string()
    } else {
        INPUT_BLANK.to_string()
    }
}

/// The bracketed placeholder for an inert select control, showing a live `SelectedLabels`
/// overlay value when present, or otherwise its selected option, or, when none is marked
/// selected, its first option, matching the HTML default.
fn select_placeholder(
    label: Option<&str>,
    options: &[SelectOption],
    overlay: Option<&FieldRenderValue>,
) -> String {
    let chosen = selected_option_display(options, overlay);
    match label {
        Some(label) => format!("[{label}: {chosen} {SELECT_MARKER}]"),
        None => format!("[{chosen} {SELECT_MARKER}]"),
    }
}

fn selected_option_display(options: &[SelectOption], overlay: Option<&FieldRenderValue>) -> String {
    if let Some(FieldRenderValue::SelectedLabels(labels)) = overlay {
        return labels.join(", ");
    }
    options
        .iter()
        .find(|option| option.selected)
        .or_else(|| options.first())
        .map(|option| option.label.as_str())
        .unwrap_or("")
        .to_string()
}

/// Mark every cell of every row with the given control's identity, so
/// [`extract_field_spans`] can recover its geometry after layout.
fn mark_field(rows: Vec<Vec<Cell>>, node_id: NodeId, kind: FieldSpanKind) -> Vec<Vec<Cell>> {
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|mut cell| {
                    cell.set_field_marker(node_id, kind);
                    cell
                })
                .collect()
        })
        .collect()
}

/// Render a button as its label wrapped in brackets, keeping the label's inline styling.
fn render_button(
    runs: &[InlineRun],
    style: &TextStyle,
    width: usize,
    width_config: &WidthConfig,
) -> Vec<Vec<Cell>> {
    let mut cells = graphemes_to_cells("[ ", style);
    cells.extend(runs_to_cells(runs, style));
    cells.extend(graphemes_to_cells(" ]", style));
    single_row(clip_cells(cells, width, width_config))
}

fn embedded_placeholder(label: &str) -> String {
    format!("[Embedded: {label}]")
}

/// Truncate a styled cell run before the first grapheme that would cross `width`.
fn clip_cells(cells: Vec<Cell>, width: usize, width_config: &WidthConfig) -> Vec<Cell> {
    let mut clipped: Vec<Cell> = Vec::new();
    let mut columns = 0usize;
    for cell in cells {
        let advance = grapheme_columns(cell.grapheme(), width_config);
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

/// Remove blank rows from the end of a row list so a container's own spacing_after is
/// the sole blank row that follows its content. Prevents inner block elements from
/// adding double blanks at container boundaries.
fn trim_trailing_blanks(mut rows: Vec<Vec<Cell>>) -> Vec<Vec<Cell>> {
    while rows.last().map(Vec::is_empty).unwrap_or(false) {
        rows.pop();
    }
    rows
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

pub(crate) fn count_columns(cells: &[Cell], width_config: &WidthConfig) -> usize {
    cells
        .iter()
        .map(|cell| grapheme_columns(cell.grapheme(), width_config))
        .sum()
}

/// Convert the laid-out rows into a filled cell buffer.
///
/// The height is the row count, guarded so a document taller than the addressable row
/// range is refused rather than truncated silently.
fn build_buffer(
    rows: Vec<Vec<Cell>>,
    width: u16,
    width_config: &WidthConfig,
) -> Result<CellBuffer, LayoutError> {
    let links = extract_link_spans(&rows, width_config);
    let anchors = extract_anchor_spans(&rows);
    let field_spans = extract_field_spans(&rows, width_config);
    let height = row_height(rows.len())?;
    let mut buffer = CellBuffer::new(width, height);
    for (row_index, row) in rows.into_iter().enumerate() {
        write_row(&mut buffer, row_index, row, width_config);
    }
    buffer.set_links(links);
    buffer.set_anchors(anchors);
    buffer.set_field_spans(field_spans);
    Ok(buffer)
}

/// Scan the laid-out rows and record one [`AnchorSpan`] per anchor name marked on a cell.
///
/// An anchor is marked only on the first grapheme of its run, so a wrapped run records the
/// row of its first line. Rows are scanned in order, so the spans come out in ascending row
/// order and the first anchor of a shared name wins when a fragment is later resolved.
fn extract_anchor_spans(rows: &[Vec<Cell>]) -> Vec<AnchorSpan> {
    let mut spans: Vec<AnchorSpan> = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let Ok(row_u16) = u16::try_from(row_index) else {
            continue;
        };
        for cell in row {
            for name in cell.anchor_names() {
                spans.push(AnchorSpan {
                    name: name.clone(),
                    row: row_u16,
                });
            }
        }
    }
    spans
}

/// The effective link a cell contributes to focus/activation, and which kind it is.
///
/// A cell's author-intended hyperlink always wins over an enclosing `<q cite>` on the
/// same cell, since the anchor is the explicit navigation target.
fn effective_link(cell: &Cell) -> Option<(&str, LinkKind)> {
    cell.link_url()
        .map(|url| (url, LinkKind::Hyperlink))
        .or_else(|| cell.citation_url().map(|url| (url, LinkKind::Citation)))
}

/// Scan the laid-out rows and record one [`LinkSpan`] per contiguous run of cells that
/// share an effective link URL and kind on a row. A link that wraps across rows yields
/// one span per row, and two adjacent links with different URLs or kinds yield two
/// spans. Columns are measured with the same width config used for layout so
/// hit-testing and rendering agree on positions.
fn extract_link_spans(rows: &[Vec<Cell>], width_config: &WidthConfig) -> Vec<LinkSpan> {
    let mut spans: Vec<LinkSpan> = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let Ok(row_u16) = u16::try_from(row_index) else {
            continue;
        };
        let mut col: u16 = 0;
        let mut open: Option<(u16, String, LinkKind)> = None; // (col_start, url, kind)
        for cell in row {
            let advance =
                u16::try_from(grapheme_columns(cell.grapheme(), width_config)).unwrap_or(1);
            match (&open, effective_link(cell)) {
                (None, Some((url, kind))) => {
                    open = Some((col, url.to_string(), kind));
                }
                (Some((start, previous_url, previous_kind)), Some((url, kind)))
                    if url != previous_url || kind != *previous_kind =>
                {
                    spans.push(LinkSpan {
                        url: previous_url.clone(),
                        kind: *previous_kind,
                        row: row_u16,
                        col_start: *start,
                        col_end: col.saturating_sub(1),
                    });
                    open = Some((col, url.to_string(), kind));
                }
                (Some((start, previous_url, previous_kind)), None) => {
                    spans.push(LinkSpan {
                        url: previous_url.clone(),
                        kind: *previous_kind,
                        row: row_u16,
                        col_start: *start,
                        col_end: col.saturating_sub(1),
                    });
                    open = None;
                }
                _ => {}
            }
            col = col.saturating_add(advance);
        }
        if let Some((start, url, kind)) = open {
            spans.push(LinkSpan {
                url,
                kind,
                row: row_u16,
                col_start: start,
                col_end: col.saturating_sub(1),
            });
        }
    }
    spans
}

/// Scan the laid-out rows and record one [`FieldSpan`] per contiguous run of cells that
/// share a field marker on a row, mirroring [`extract_link_spans`] exactly in shape. A
/// control that wraps across rows yields one span per row.
fn extract_field_spans(rows: &[Vec<Cell>], width_config: &WidthConfig) -> Vec<FieldSpan> {
    let mut spans: Vec<FieldSpan> = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let Ok(row_u16) = u16::try_from(row_index) else {
            continue;
        };
        let mut col: u16 = 0;
        let mut open: Option<(u16, NodeId, FieldSpanKind)> = None; // (col_start, node_id, kind)
        for cell in row {
            let advance =
                u16::try_from(grapheme_columns(cell.grapheme(), width_config)).unwrap_or(1);
            match (&open, cell.field_marker()) {
                (None, Some((node_id, kind))) => {
                    open = Some((col, node_id, kind));
                }
                (Some((start, previous_node_id, previous_kind)), Some((node_id, kind)))
                    if node_id != *previous_node_id || kind != *previous_kind =>
                {
                    spans.push(FieldSpan {
                        node_id: *previous_node_id,
                        kind: *previous_kind,
                        row: row_u16,
                        col_start: *start,
                        col_end: col.saturating_sub(1),
                    });
                    open = Some((col, node_id, kind));
                }
                (Some((start, previous_node_id, previous_kind)), None) => {
                    spans.push(FieldSpan {
                        node_id: *previous_node_id,
                        kind: *previous_kind,
                        row: row_u16,
                        col_start: *start,
                        col_end: col.saturating_sub(1),
                    });
                    open = None;
                }
                _ => {}
            }
            col = col.saturating_add(advance);
        }
        if let Some((start, node_id, kind)) = open {
            spans.push(FieldSpan {
                node_id,
                kind,
                row: row_u16,
                col_start: start,
                col_end: col.saturating_sub(1),
            });
        }
    }
    spans
}

fn row_height(count: usize) -> Result<u16, LayoutError> {
    u16::try_from(count).map_err(|_| LayoutError::DimensionOverflow)
}

/// Write one row of cells into the buffer, advancing the column by each grapheme's width
/// so a wide grapheme leaves the following column blank.
///
/// The column a wide grapheme spans into is written as a blank cell, not merely skipped,
/// so the trailing half never keeps stale content from an earlier write and no partial
/// grapheme is ever left in the buffer.
fn write_row(
    buffer: &mut CellBuffer,
    row_index: usize,
    row: Vec<Cell>,
    width_config: &WidthConfig,
) {
    let Ok(row_position) = u16::try_from(row_index) else {
        return;
    };
    let mut column = 0usize;
    for cell in row {
        let advance = grapheme_columns(cell.grapheme(), width_config);
        write_cell(
            buffer,
            column,
            row_position,
            substitute_emoji(cell, width_config),
        );
        blank_spanned_columns(buffer, column, row_position, advance);
        column += advance;
    }
}

/// Replace an emoji cell with the neutral placeholder in `Replace` mode, leaving every other
/// cell untouched.
///
/// The column advance is measured from the source grapheme before substitution, and the
/// placeholder is measured the same way, so the substituted cell occupies exactly the space
/// the layout reserved for it.
fn substitute_emoji(cell: Cell, width_config: &WidthConfig) -> Cell {
    match emoji_replacement(cell.grapheme(), width_config) {
        Some(placeholder) => cell.with_grapheme(placeholder.to_string()),
        None => cell,
    }
}

/// Blank the columns a grapheme spans beyond its first, so a wide grapheme's trailing
/// column shows a space rather than leftover content.
fn blank_spanned_columns(
    buffer: &mut CellBuffer,
    column: usize,
    row_position: u16,
    advance: usize,
) {
    for offset in 1..advance {
        write_cell(buffer, column + offset, row_position, Cell::blank());
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
struct LineWrapper<'a> {
    width: usize,
    width_config: &'a WidthConfig,
    rows: Vec<Vec<Cell>>,
    current: Vec<Cell>,
    current_columns: usize,
    pending_space: Option<Cell>,
}

impl<'a> LineWrapper<'a> {
    fn new(width: usize, width_config: &'a WidthConfig) -> LineWrapper<'a> {
        LineWrapper {
            width,
            width_config,
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
        let columns = count_columns(&cells, self.width_config);
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

    /// Break a word wider than the whole line across rows, one grapheme cluster at a time.
    ///
    /// Each cell already holds exactly one grapheme cluster, so breaking between cells can
    /// never split a cluster: a base and its combining marks stay in the same cell and
    /// therefore on the same row.
    fn push_broken_word(&mut self, cells: Vec<Cell>) {
        for cell in cells {
            self.push_grapheme_breaking(cell);
        }
    }

    fn push_grapheme_breaking(&mut self, cell: Cell) {
        let columns = grapheme_columns(cell.grapheme(), self.width_config);
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
