// @file crates/browser-html/src/parser.rs
// @description Parses HTML into a recursive Document tree of sanitized, script-free nodes.
// @layer html
// @created meerita <meerita@icloud.com>

use std::borrow::Cow;
use std::cell::{Cell, Ref, RefCell};

use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{parse_document, Attribute, LocalName, Namespace, ParseOpts, QualName};
use percent_encoding::percent_decode_str;
use url::Url;

use crate::button_element::ButtonElement;
use crate::button_kind::ButtonKind;
use crate::document::{Document, DocumentTitle};
use crate::encoding::{decode, detect_encoding, DetectedEncoding};
use crate::error::HtmlError;
use crate::form_element::FormElement;
use crate::form_method::FormMethod;
use crate::inline_run::{InlineEmphasis, InlineRun};
use crate::input_element::InputElement;
use crate::input_kind::InputKind;
use crate::landmark_role::LandmarkRole;
use crate::node_id::NodeId;
use crate::sanitize::{
    collapse_whitespace, strip_control_characters, strip_control_characters_preserving_layout,
};
use crate::select_element::{SelectElement, SelectOption};
use crate::semantic_node::SemanticNode;
use crate::textarea_element::TextareaElement;

/// Upper bound on the number of DOM nodes retained during a parse.
///
/// Untrusted remote content could otherwise expand into an arbitrary number of nodes
/// and exhaust memory, so the parser stops retaining nodes past this count and fails.
const MAX_NODE_COUNT: usize = 50_000;

/// Upper bound on how deeply DOM nodes may nest during a parse.
///
/// Deeply nested markup would otherwise drive unbounded recursion when the tree is
/// walked, so the parser rejects any document that nests past this depth.
const MAX_DOM_DEPTH: usize = 256;

/// Upper bound on the number of rows retained from a single table.
///
/// A table with far more rows than this is a memory hazard from untrusted markup and
/// cannot render usefully, so it is truncated to this many rows and a warning is emitted.
const MAX_TABLE_ROWS: usize = 1_000;

/// Upper bound on the number of columns (cells) retained from a single table row.
///
/// A row far wider than the terminal cannot render as columns and only inflates the node
/// count, so each row is truncated to this many cells and a warning is emitted.
const MAX_TABLE_COLUMNS: usize = 64;

/// Upper bound on the number of options retained from a single `<select>`.
///
/// A pathological `<select>` with far more options than any terminal could usefully
/// present is a memory hazard from untrusted markup, so collection stops once this many
/// options have been gathered.
const MAX_SELECT_OPTIONS: usize = 500;

/// Parse HTML document bytes into a recursive [`Document`] tree.
///
/// The bytes are decoded to Unicode before parsing: the encoding is detected from a
/// byte-order mark, then the `Content-Type` charset hint, then a bounded `<meta charset>`
/// pre-scan, then a UTF-8 fallback, and decoding replaces malformed bytes rather than
/// failing. All decoded text is stripped of control characters before it enters a node
/// or the title, `<script>` elements are counted and never retained, and the node count
/// and nesting depth are bounded so untrusted input cannot exhaust memory.
pub fn parse_html(source: &[u8], charset_hint: Option<&str>) -> Result<Document, HtmlError> {
    parse_html_with_base(source, charset_hint, None)
}

/// Parse HTML document bytes, resolving relative references against `document_url`.
///
/// Identical to [`parse_html`] except that relative link and image references resolve
/// against a base URL when the document declares no usable `<base href>`. `document_url`
/// is the page's own location (the final URL after any redirects); it is passed as a
/// string so no URL type crosses this boundary. A `<base href>` still overrides it, and
/// a relative `<base href>` resolves against it. When neither a base element nor a
/// document URL is available, references are kept exactly as authored.
pub fn parse_html_with_base(
    source: &[u8],
    charset_hint: Option<&str>,
    document_url: Option<&str>,
) -> Result<Document, HtmlError> {
    let (encoding, mark_length) = detect_encoding(source, charset_hint);
    let decoded = decode(source, encoding, mark_length);

    let dom = parse_document(DomBuilder::new(), ParseOpts::default()).one(decoded.as_str());

    if let Some(error) = dom.limit_error() {
        return Err(error);
    }

    let arena = dom.into_nodes();
    let title = extract_title(&arena).map(|raw| DocumentTitle::new(&raw));
    let document_base = document_url.and_then(|value| Url::parse(value).ok());
    let base_url = resolve_base_url(&arena, document_base);

    let mut extractor = TreeExtractor::new(&arena, base_url);
    let mut children = extractor.walk_children(DOCUMENT_HANDLE);
    let script_count = extractor.script_count();

    if script_count > 0 {
        children.push(SemanticNode::Warning {
            message: suppressed_scripts_message(script_count),
        });
    }

    // The detected and active encoding coincide because no override is applied here.
    let detected = DetectedEncoding::new(encoding, encoding);
    Ok(Document::new(children, title, script_count).with_encoding(detected))
}

/// Handle of the document root node, created first by [`DomBuilder::new`].
const DOCUMENT_HANDLE: usize = 0;

/// Handle returned once a resource limit is exceeded, so no further nodes are retained.
const OVERFLOW_HANDLE: usize = 1;

/// A single node in the arena the [`DomBuilder`] fills while `html5ever` drives it.
struct Node {
    name: QualName,
    is_element: bool,
    attributes: Vec<Attribute>,
    text: Option<String>,
    children: Vec<usize>,
    parent: Option<usize>,
    depth: usize,
    template_contents: Option<usize>,
}

impl Node {
    fn container(name: QualName) -> Node {
        Node {
            name,
            is_element: false,
            attributes: Vec::new(),
            text: None,
            children: Vec::new(),
            parent: None,
            depth: 0,
            template_contents: None,
        }
    }

    fn element(name: QualName, attributes: Vec<Attribute>) -> Node {
        Node {
            name,
            is_element: true,
            attributes,
            text: None,
            children: Vec::new(),
            parent: None,
            depth: 0,
            template_contents: None,
        }
    }

    fn text(name: QualName, content: String) -> Node {
        Node {
            name,
            is_element: false,
            attributes: Vec::new(),
            text: Some(content),
            children: Vec::new(),
            parent: None,
            depth: 0,
            template_contents: None,
        }
    }
}

/// A minimal DOM sink that builds an arena of [`Node`]s from `html5ever`'s events.
///
/// The arena is enough to walk the finished tree into a flat block stream. It tracks a
/// node count and nesting depth so a resource limit can be reported once parsing ends;
/// past the limit it stops retaining nodes and hands back [`OVERFLOW_HANDLE`].
struct DomBuilder {
    nodes: RefCell<Vec<Node>>,
    node_count: Cell<usize>,
    depth_exceeded: Cell<bool>,
}

impl DomBuilder {
    fn new() -> DomBuilder {
        let nodes = vec![
            Node::container(empty_name()),
            Node::element(empty_name(), Vec::new()),
        ];
        DomBuilder {
            nodes: RefCell::new(nodes),
            node_count: Cell::new(0),
            depth_exceeded: Cell::new(false),
        }
    }

    fn over_limit(&self) -> bool {
        self.node_count.get() > MAX_NODE_COUNT || self.depth_exceeded.get()
    }

    fn limit_error(&self) -> Option<HtmlError> {
        if self.depth_exceeded.get() {
            return Some(HtmlError::MaxDepthExceeded);
        }
        if self.node_count.get() > MAX_NODE_COUNT {
            return Some(HtmlError::MaxNodeCountExceeded);
        }
        None
    }

    fn into_nodes(self) -> Vec<Node> {
        self.nodes.into_inner()
    }

    fn push(&self, node: Node) -> usize {
        self.node_count.set(self.node_count.get() + 1);
        if self.over_limit() {
            return OVERFLOW_HANDLE;
        }
        let mut nodes = self.nodes.borrow_mut();
        nodes.push(node);
        nodes.len() - 1
    }

    fn link_child(&self, parent: usize, child: usize) {
        let mut nodes = self.nodes.borrow_mut();
        let parent_depth = nodes[parent].depth;
        let child_depth = parent_depth + 1;
        if child_depth > MAX_DOM_DEPTH {
            self.depth_exceeded.set(true);
        }
        nodes[child].parent = Some(parent);
        nodes[child].depth = child_depth;
        nodes[parent].children.push(child);
    }

    fn append_text(&self, parent: usize, text: &str) {
        if let Some(existing) = self.last_text_child(parent) {
            self.nodes.borrow_mut()[existing]
                .text
                .get_or_insert_with(String::new)
                .push_str(text);
            return;
        }
        let child = self.push(Node::text(empty_name(), text.to_string()));
        self.link_child(parent, child);
    }

    fn last_text_child(&self, parent: usize) -> Option<usize> {
        let nodes = self.nodes.borrow();
        let last = *nodes[parent].children.last()?;
        if nodes[last].text.is_some() {
            return Some(last);
        }
        None
    }

    fn text_before(&self, sibling: usize, text: &str) {
        let Some((parent, index)) = self.sibling_position(sibling) else {
            return;
        };
        if index > 0 {
            let previous = self.nodes.borrow()[parent].children[index - 1];
            if self.nodes.borrow()[previous].text.is_some() {
                self.nodes.borrow_mut()[previous]
                    .text
                    .get_or_insert_with(String::new)
                    .push_str(text);
                return;
            }
        }
        let child = self.push(Node::text(empty_name(), text.to_string()));
        self.insert_child(parent, index, child);
    }

    fn node_before(&self, sibling: usize, child: usize) {
        self.detach(child);
        let Some((parent, index)) = self.sibling_position(sibling) else {
            return;
        };
        self.insert_child(parent, index, child);
    }

    fn insert_child(&self, parent: usize, index: usize, child: usize) {
        let mut nodes = self.nodes.borrow_mut();
        let child_depth = nodes[parent].depth + 1;
        if child_depth > MAX_DOM_DEPTH {
            self.depth_exceeded.set(true);
        }
        nodes[child].parent = Some(parent);
        nodes[child].depth = child_depth;
        let bounded = index.min(nodes[parent].children.len());
        nodes[parent].children.insert(bounded, child);
    }

    fn sibling_position(&self, sibling: usize) -> Option<(usize, usize)> {
        let nodes = self.nodes.borrow();
        let parent = nodes[sibling].parent?;
        let index = nodes[parent]
            .children
            .iter()
            .position(|candidate| *candidate == sibling)?;
        Some((parent, index))
    }

    fn detach(&self, target: usize) {
        let mut nodes = self.nodes.borrow_mut();
        let Some(parent) = nodes[target].parent else {
            return;
        };
        nodes[parent]
            .children
            .retain(|candidate| *candidate != target);
        nodes[target].parent = None;
    }
}

impl TreeSink for DomBuilder {
    type Handle = usize;
    type Output = Self;
    type ElemName<'a> = Ref<'a, QualName>;

    fn finish(self) -> Self {
        self
    }

    fn parse_error(&self, _message: Cow<'static, str>) {}

    fn get_document(&self) -> usize {
        DOCUMENT_HANDLE
    }

    fn elem_name(&self, target: &usize) -> Ref<'_, QualName> {
        Ref::map(self.nodes.borrow(), |nodes| &nodes[*target].name)
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> usize {
        let element = self.push(Node::element(name, attrs));
        if flags.template {
            let contents = self.push(Node::container(empty_name()));
            if element != OVERFLOW_HANDLE {
                self.nodes.borrow_mut()[element].template_contents = Some(contents);
            }
        }
        element
    }

    fn create_comment(&self, _text: StrTendril) -> usize {
        self.push(Node::container(empty_name()))
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> usize {
        self.push(Node::container(empty_name()))
    }

    fn append(&self, parent: &usize, child: NodeOrText<usize>) {
        if self.over_limit() {
            return;
        }
        match child {
            NodeOrText::AppendNode(node) => self.link_child(*parent, node),
            NodeOrText::AppendText(text) => self.append_text(*parent, &text),
        }
    }

    fn append_based_on_parent_node(
        &self,
        element: &usize,
        prev_element: &usize,
        child: NodeOrText<usize>,
    ) {
        let has_parent = self.nodes.borrow()[*element].parent.is_some();
        if has_parent {
            self.append_before_sibling(element, child);
            return;
        }
        self.append(prev_element, child);
    }

    fn append_doctype_to_document(
        &self,
        _name: StrTendril,
        _public_id: StrTendril,
        _system_id: StrTendril,
    ) {
    }

    fn get_template_contents(&self, target: &usize) -> usize {
        self.nodes.borrow()[*target]
            .template_contents
            .unwrap_or(*target)
    }

    fn same_node(&self, x: &usize, y: &usize) -> bool {
        x == y
    }

    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &usize, new_node: NodeOrText<usize>) {
        if self.over_limit() {
            return;
        }
        match new_node {
            NodeOrText::AppendNode(node) => self.node_before(*sibling, node),
            NodeOrText::AppendText(text) => self.text_before(*sibling, &text),
        }
    }

    fn add_attrs_if_missing(&self, target: &usize, attrs: Vec<Attribute>) {
        let mut nodes = self.nodes.borrow_mut();
        for attribute in attrs {
            let already_present = nodes[*target]
                .attributes
                .iter()
                .any(|existing| existing.name == attribute.name);
            if already_present {
                continue;
            }
            nodes[*target].attributes.push(attribute);
        }
    }

    fn remove_from_parent(&self, target: &usize) {
        self.detach(*target);
    }

    fn reparent_children(&self, node: &usize, new_parent: &usize) {
        let moved = std::mem::take(&mut self.nodes.borrow_mut()[*node].children);
        for child in moved {
            self.link_child(*new_parent, child);
        }
    }
}

/// Walks a finished node arena into a recursive tree of [`SemanticNode`]s.
///
/// Each walk method returns the block-level children it produces; container elements
/// recurse into their own children, so the returned tree owns itself. The script count
/// accumulates across the whole walk so it can be reported once parsing ends. The base
/// URL, taken from the document's `<base href>`, resolves relative link and image
/// references as runs and image placeholders are built.
struct TreeExtractor<'a> {
    arena: &'a [Node],
    script_count: usize,
    base_url: Option<Url>,
    /// Anchor names (`id` and `<a name>`) seen but not yet attached to a run.
    ///
    /// An anchor names a point in the document, so it is held here until the next run is
    /// built and absorbs it. The buffer persists across the block boundary so an `id` on a
    /// container reaches the first run of the block nested inside it.
    pending_anchors: Vec<String>,
    /// The next value [`allocate_node_id`](Self::allocate_node_id) hands out.
    ///
    /// Ids are assigned in document order as forms and controls are visited; they are not
    /// required to be stable across reparses, since a page's live field state is rebuilt
    /// fresh on every load.
    next_node_id: u32,
}

impl<'a> TreeExtractor<'a> {
    fn new(arena: &'a [Node], base_url: Option<Url>) -> TreeExtractor<'a> {
        TreeExtractor {
            arena,
            script_count: 0,
            base_url,
            pending_anchors: Vec::new(),
            next_node_id: 0,
        }
    }

    /// Allocate the next [`NodeId`], in document order.
    fn allocate_node_id(&mut self) -> NodeId {
        let id = NodeId::new(self.next_node_id);
        self.next_node_id += 1;
        id
    }

    /// Record this element's anchor names so the next run built absorbs them.
    ///
    /// Every element contributes its `id`; an `<a>` also contributes its `name`. Names are
    /// decoded and control-stripped so they match a decoded fragment, and a name already
    /// pending is not added twice, which keeps the block walk and the inline walk from
    /// recording the same element's `id` twice.
    fn enter_anchor_names(&mut self, element: usize) {
        self.record_anchor_name(self.attribute(element, "id"));
        if local_name(&self.arena[element].name) == "a" {
            self.record_anchor_name(self.attribute(element, "name"));
        }
    }

    fn record_anchor_name(&mut self, raw: Option<String>) {
        let Some(raw) = raw else {
            return;
        };
        let name = decode_anchor_name(&raw);
        if name.is_empty() {
            return;
        }
        if self.pending_anchors.contains(&name) {
            return;
        }
        self.pending_anchors.push(name);
    }

    /// Drain the anchors pending at a block's end into a trailing anchor-only segment.
    ///
    /// An anchor that follows the block's last text has nothing after it to absorb it, so
    /// it is carried out on an empty segment and attached to the block's last run.
    fn take_pending_anchor_segment(&mut self) -> Option<Segment> {
        if self.pending_anchors.is_empty() {
            return None;
        }
        Some(Segment::anchor_only(std::mem::take(
            &mut self.pending_anchors,
        )))
    }

    fn script_count(&self) -> usize {
        self.script_count
    }

    fn walk_children(&mut self, container: usize) -> Vec<SemanticNode> {
        let mut output = Vec::new();
        self.walk_children_into(container, &mut output);
        output
    }

    fn walk_children_into(&mut self, container: usize, output: &mut Vec<SemanticNode>) {
        for child in self.child_handles(container) {
            self.walk_node(child, output);
        }
    }

    fn walk_node(&mut self, node: usize, output: &mut Vec<SemanticNode>) {
        let entry = &self.arena[node];
        if entry.text.is_some() {
            return;
        }
        if !entry.is_element {
            self.walk_children_into(node, output);
            return;
        }
        self.handle_element(node, output);
    }

    fn handle_element(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        self.enter_anchor_names(element);
        let tag = local_name(&self.arena[element].name).to_string();
        if let Some(level) = heading_level(&tag) {
            self.push_heading(element, level, output);
            return;
        }
        self.map_named_element(element, &tag, output);
    }

    fn map_named_element(&mut self, element: usize, tag: &str, output: &mut Vec<SemanticNode>) {
        match tag {
            "script" => self.script_count += 1,
            "style" | "title" => {}
            "p" => self.push_paragraph(element, output),
            "ul" => self.push_list(element, false, output),
            "ol" => self.push_list(element, true, output),
            "blockquote" => self.push_quote(element, output),
            "pre" => self.push_verbatim_block(element, output, |text| {
                SemanticNode::PreformattedBlock { text }
            }),
            "code" => {
                self.push_verbatim_block(element, output, |text| SemanticNode::CodeBlock { text })
            }
            "table" => self.push_table(element, output),
            "hr" => output.push(SemanticNode::Separator),
            "img" => self.push_image(element, output),
            "figure" => self.push_figure(element, output),
            "details" => self.push_details(element, output),
            "summary" => self.push_summary(element, output),
            "form" => self.push_form(element, output),
            "input" => self.push_input(element, output),
            "textarea" => self.push_textarea(element, output),
            "select" => self.push_select(element, output),
            "button" => self.push_button(element, output),
            "iframe" | "object" | "embed" | "video" | "audio" => {
                output.push(SemanticNode::EmbeddedContent {
                    label: embedded_label(tag).to_string(),
                })
            }
            "nav" | "main" | "aside" | "footer" | "header" | "section" => {
                self.push_landmark(element, tag, output)
            }
            _ => self.push_landmark_or_walk(element, output),
        }
    }

    /// Map a container that is not a known block element.
    ///
    /// A generic container carrying an ARIA landmark `role` becomes a `Landmark`; any
    /// other container contributes only its children, so unknown wrappers stay transparent.
    fn push_landmark_or_walk(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let Some(role) = self
            .attribute(element, "role")
            .and_then(|role| LandmarkRole::from_aria_role(&role))
        else {
            self.walk_children_into(element, output);
            return;
        };
        let children = self.block_children(element);
        if children.is_empty() {
            return;
        }
        output.push(SemanticNode::Landmark { role, children });
    }

    fn push_heading(&mut self, element: usize, level: u8, output: &mut Vec<SemanticNode>) {
        let runs = self.block_runs(element);
        if runs.is_empty() {
            return;
        }
        output.push(SemanticNode::Heading {
            level,
            runs,
            inline_style: self.inline_style(element),
        });
    }

    fn push_paragraph(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let runs = self.block_runs(element);
        if runs.is_empty() {
            return;
        }
        output.push(SemanticNode::Paragraph {
            runs,
            inline_style: self.inline_style(element),
        });
    }

    fn push_list(&mut self, element: usize, ordered: bool, output: &mut Vec<SemanticNode>) {
        let children = self.list_items(element);
        if children.is_empty() {
            return;
        }
        output.push(SemanticNode::List {
            ordered,
            children,
            inline_style: self.inline_style(element),
        });
    }

    fn push_quote(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let children = self.block_children(element);
        if children.is_empty() {
            return;
        }
        output.push(SemanticNode::Quote {
            children,
            inline_style: self.inline_style(element),
        });
    }

    /// Map a `<table>` into a `Table` of `TableRow`s of `TableCell`s.
    ///
    /// Rows exceeding [`MAX_TABLE_ROWS`] and cells exceeding [`MAX_TABLE_COLUMNS`] per row
    /// are dropped and a `Warning` describing the truncation follows the table, so
    /// untrusted markup cannot expand a table without bound.
    fn push_table(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let mut rows = Vec::new();
        self.collect_table_rows(element, &mut rows);
        let rows_truncated = rows.len() > MAX_TABLE_ROWS;
        rows.truncate(MAX_TABLE_ROWS);
        let columns_truncated = truncate_row_columns(&mut rows);
        if rows.is_empty() {
            return;
        }
        output.push(SemanticNode::Table { children: rows });
        if rows_truncated || columns_truncated {
            output.push(SemanticNode::Warning {
                message: table_truncation_message(rows_truncated, columns_truncated),
            });
        }
    }

    /// Gather a table's rows in source order, flattening `<thead>`/`<tbody>`/`<tfoot>`
    /// wrappers so their rows join the sequence at the table's own level.
    fn collect_table_rows(&mut self, container: usize, rows: &mut Vec<SemanticNode>) {
        for child in self.child_handles(container) {
            self.collect_row_or_section(child, rows);
        }
    }

    fn collect_row_or_section(&mut self, node: usize, rows: &mut Vec<SemanticNode>) {
        let entry = &self.arena[node];
        if !entry.is_element {
            return;
        }
        let tag = local_name(&entry.name).to_string();
        if tag == "tr" {
            self.push_table_row(node, rows);
            return;
        }
        if is_table_section(&tag) {
            self.collect_table_rows(node, rows);
        }
    }

    fn push_table_row(&mut self, node: usize, rows: &mut Vec<SemanticNode>) {
        let mut cells = Vec::new();
        for child in self.child_handles(node) {
            self.push_table_cell(child, &mut cells);
        }
        if cells.is_empty() {
            return;
        }
        rows.push(SemanticNode::TableRow { children: cells });
    }

    fn push_table_cell(&mut self, node: usize, cells: &mut Vec<SemanticNode>) {
        let entry = &self.arena[node];
        if !entry.is_element {
            return;
        }
        let tag = local_name(&entry.name).to_string();
        let header = tag == "th";
        if !header && tag != "td" {
            return;
        }
        // colspan and rowspan are ignored: each cell occupies exactly one column
        // position, so a spanned cell is rendered as a single cell rather than widened.
        // Honoring spans needs a grid model that a later table milestone will add.
        let children = self.block_children(node);
        cells.push(SemanticNode::TableCell {
            header,
            children,
            inline_style: self.inline_style(node),
        });
    }

    fn list_items(&mut self, element: usize) -> Vec<SemanticNode> {
        let mut items = Vec::new();
        for child in self.child_handles(element) {
            self.push_list_item(child, &mut items);
        }
        items
    }

    fn push_list_item(&mut self, node: usize, items: &mut Vec<SemanticNode>) {
        if !self.is_list_item_element(node) {
            return;
        }
        let children = self.block_children(node);
        if children.is_empty() {
            return;
        }
        items.push(SemanticNode::ListItem {
            children,
            inline_style: self.inline_style(node),
        });
    }

    fn is_list_item_element(&self, node: usize) -> bool {
        let entry = &self.arena[node];
        entry.is_element && local_name(&entry.name) == "li"
    }

    /// Gather the block-level children of a container, preserving source order.
    ///
    /// Block children (paragraphs, nested lists, quotes, and the like) are walked in
    /// place. Runs of bare inline content between them (text, links, emphasis directly
    /// inside the container) are collected into anonymous paragraphs, so a list item that
    /// holds text followed by a nested list keeps both parts in their original order. A
    /// container whose whole content is inline yields a single paragraph.
    fn block_children(&mut self, element: usize) -> Vec<SemanticNode> {
        let mut output = Vec::new();
        let mut inline_children: Vec<usize> = Vec::new();
        for child in self.child_handles(element) {
            if !self.child_is_block_level(child) {
                inline_children.push(child);
                continue;
            }
            self.flush_inline_paragraph(&inline_children, &mut output);
            inline_children.clear();
            self.walk_node(child, &mut output);
        }
        self.flush_inline_paragraph(&inline_children, &mut output);
        output
    }

    /// Whether a child, once walked, contributes block-level content rather than inline
    /// text.
    ///
    /// A child is block-level when its subtree holds an element that maps to a block node
    /// (a paragraph, list, quote, heading, separator, image, or preformatted block). Text
    /// nodes and inline markup (links, emphasis) are not block-level, so they are gathered
    /// into anonymous paragraphs instead of being walked as blocks.
    fn child_is_block_level(&self, node: usize) -> bool {
        let entry = &self.arena[node];
        if entry.is_element && is_block_tag(local_name(&entry.name)) {
            return true;
        }
        entry
            .children
            .iter()
            .any(|child| self.child_is_block_level(*child))
    }

    /// Collapse a run of inline children into one paragraph and push it.
    ///
    /// Whitespace-only content collapses to no runs and contributes no paragraph, matching
    /// how a plain text block drops surrounding whitespace.
    fn flush_inline_paragraph(
        &mut self,
        inline_children: &[usize],
        output: &mut Vec<SemanticNode>,
    ) {
        if inline_children.is_empty() {
            return;
        }
        let context = InlineContext::root();
        let mut segments = Vec::new();
        for child in inline_children {
            self.gather_segments(*child, &context, &mut segments);
        }
        segments.extend(self.take_pending_anchor_segment());
        let runs = collapse_segments(segments);
        if runs.is_empty() {
            return;
        }
        // An anonymous paragraph gathers bare inline content that has no element of its
        // own, so it carries no inline style.
        output.push(SemanticNode::Paragraph {
            runs,
            inline_style: None,
        });
    }

    /// Gather a text block's content into a sequence of styled inline runs.
    ///
    /// A new run starts at every boundary where the emphasis (`<strong>`/`<b>`,
    /// `<em>`/`<i>`, inline `<code>`) or the link target changes. Whitespace is collapsed
    /// across the whole block, exactly as plain block text is collapsed, so wrapping is
    /// unaffected by where runs begin and end. An empty block yields no run so it is
    /// dropped upstream.
    fn block_runs(&mut self, element: usize) -> Vec<InlineRun> {
        let mut segments = Vec::new();
        self.gather_segments(element, &InlineContext::root(), &mut segments);
        segments.extend(self.take_pending_anchor_segment());
        collapse_segments(segments)
    }

    /// Walk a block's descendants in order, recording each text node as a segment tagged
    /// with the emphasis and link in force at that point.
    fn gather_segments(
        &mut self,
        node: usize,
        context: &InlineContext,
        segments: &mut Vec<Segment>,
    ) {
        let entry = &self.arena[node];
        if let Some(text) = &entry.text {
            let anchors = std::mem::take(&mut self.pending_anchors);
            segments.push(Segment::with_anchors(text, context, anchors));
            return;
        }
        if entry.is_element {
            self.gather_element_segments(node, context, segments);
            return;
        }
        self.gather_children_segments(node, context, segments);
    }

    fn gather_element_segments(
        &mut self,
        element: usize,
        context: &InlineContext,
        segments: &mut Vec<Segment>,
    ) {
        self.enter_anchor_names(element);
        let tag = local_name(&self.arena[element].name).to_string();
        if tag == "script" {
            self.script_count += 1;
            return;
        }
        if tag == "style" {
            return;
        }
        if tag == "q" {
            self.gather_quote_segments(element, context, segments);
            return;
        }
        let child_context = self.child_context(element, &tag, context);
        self.gather_children_segments(element, &child_context, segments);
    }

    /// Bracket a `<q>` element's children with depth-selected quote-mark segments,
    /// carrying the element's resolved `cite` (or the surrounding citation, when nested
    /// inside another `<q>`) so the marks themselves are part of the activatable span.
    fn gather_quote_segments(
        &mut self,
        element: usize,
        context: &InlineContext,
        segments: &mut Vec<Segment>,
    ) {
        let mut child = context.clone();
        child.quote_depth = context.quote_depth.saturating_add(1);
        child.citation = self
            .quote_citation(element)
            .or_else(|| context.citation.clone());
        let (open_mark, close_mark) = quote_marks(child.quote_depth);
        segments.push(Segment::mark(open_mark, &child));
        self.gather_children_segments(element, &child, segments);
        segments.push(Segment::mark(close_mark, &child));
    }

    /// The resolved citation target of a `<q>` element's `cite` attribute, or `None` when
    /// it has no usable `cite`, mirroring `anchor_link`'s resolution of `href`.
    fn quote_citation(&self, element: usize) -> Option<String> {
        let reference = sanitize_reference(self.attribute(element, "cite")?);
        if reference.is_empty() {
            return None;
        }
        Some(resolve_reference(&reference, self.base_url.as_ref()))
    }

    fn gather_children_segments(
        &mut self,
        node: usize,
        context: &InlineContext,
        segments: &mut Vec<Segment>,
    ) {
        for child in self.child_handles(node) {
            self.gather_segments(child, context, segments);
        }
    }

    /// Derive the inline context for an element's children by folding the element's own
    /// emphasis or link onto the surrounding context.
    fn child_context(&self, element: usize, tag: &str, context: &InlineContext) -> InlineContext {
        let mut child = context.clone();
        match tag {
            "strong" | "b" => child.emphasis.strong = true,
            "em" | "i" => child.emphasis.emphasis = true,
            "code" => child.emphasis.code = true,
            "a" => child.link = self.anchor_link(element).or_else(|| context.link.clone()),
            _ => {}
        }
        child
    }

    /// The resolved link target of an anchor, or `None` when it has no usable `href`.
    ///
    /// An anchor without an `href` contributes plain text, so its descendants keep the
    /// surrounding link context rather than gaining one.
    fn anchor_link(&self, element: usize) -> Option<String> {
        let reference = sanitize_reference(self.attribute(element, "href")?);
        if reference.is_empty() {
            return None;
        }
        Some(resolve_reference(&reference, self.base_url.as_ref()))
    }

    fn push_image(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let alt = self
            .attribute(element, "alt")
            .map(sanitize_inline)
            .unwrap_or_default();
        let title = self.attribute(element, "title").map(sanitize_inline);
        let source = self
            .attribute(element, "src")
            .map(sanitize_reference)
            .map(|reference| resolve_reference(&reference, self.base_url.as_ref()));
        output.push(SemanticNode::ImagePlaceholder { alt, title, source });
    }

    /// Map a `<figure>` into a `Figure`, lifting a `<figcaption>` out as the caption and
    /// keeping the remaining content as the figure's children in source order.
    fn push_figure(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let mut children = Vec::new();
        let mut caption = None;
        for child in self.child_handles(element) {
            if self.is_element_named(child, "figcaption") {
                caption = self.non_empty_runs(child);
                continue;
            }
            self.walk_node(child, &mut children);
        }
        if children.is_empty() && caption.is_none() {
            return;
        }
        output.push(SemanticNode::Figure { children, caption });
    }

    /// Map a `<details>` into a `Details`. Its `<summary>` child folds into a `Summary`
    /// node through the block walk, so the summary and the body keep their source order.
    fn push_details(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let open = self.attribute(element, "open").is_some();
        let children = self.block_children(element);
        if children.is_empty() {
            return;
        }
        output.push(SemanticNode::Details { open, children });
    }

    fn push_summary(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let runs = self.block_runs(element);
        if runs.is_empty() {
            return;
        }
        output.push(SemanticNode::Summary {
            runs,
            inline_style: self.inline_style(element),
        });
    }

    /// Map a `<form>` into a `Form`, resolving its submission `action` and `method`
    /// before recursing into its children, so the form's own id precedes its controls'
    /// ids in document order.
    fn push_form(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let id = self.allocate_node_id();
        let action = self.form_action(element);
        let method = self.form_method(element);
        let children = self.block_children(element);
        if children.is_empty() {
            return;
        }
        output.push(SemanticNode::Form(FormElement {
            id,
            action,
            method,
            children,
        }));
    }

    /// A form's resolved submission target: an absent or empty `action` attribute
    /// resolves to the document's own URL, exactly like an empty link `href` would.
    fn form_action(&self, element: usize) -> String {
        let action = self
            .attribute(element, "action")
            .map(sanitize_reference)
            .unwrap_or_default();
        resolve_reference(&action, self.base_url.as_ref())
    }

    /// A form's submission method: matched case-insensitively, defaulting to `Get` for
    /// an absent or unrecognized `method` attribute.
    fn form_method(&self, element: usize) -> FormMethod {
        let Some(raw) = self.attribute(element, "method") else {
            return FormMethod::Get;
        };
        if raw.trim().eq_ignore_ascii_case("post") {
            return FormMethod::Post;
        }
        FormMethod::Get
    }

    fn push_landmark(&mut self, element: usize, tag: &str, output: &mut Vec<SemanticNode>) {
        let role = self.landmark_role(element, tag);
        let children = self.block_children(element);
        if children.is_empty() {
            return;
        }
        output.push(SemanticNode::Landmark { role, children });
    }

    /// The landmark role of an element: an explicit ARIA `role` attribute wins over the
    /// role implied by the element name.
    fn landmark_role(&self, element: usize, tag: &str) -> LandmarkRole {
        let aria = self
            .attribute(element, "role")
            .and_then(|role| LandmarkRole::from_aria_role(&role));
        aria.or_else(|| LandmarkRole::from_tag(tag))
            .unwrap_or(LandmarkRole::Region)
    }

    /// Map an `<input>` into `Input`, or into `Button` when its `type` normalizes to a
    /// submit/reset/button control.
    ///
    /// A sensitive (`type="password"`) input never has its `value`/`checked` attribute
    /// read: [`input_value_and_checked`](Self::input_value_and_checked) returns their
    /// defaults for it without inspecting the source at all.
    fn push_input(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let kind = InputKind::from_type_attribute(self.attribute(element, "type").as_deref());
        if let Some(button_kind) = button_kind_for_input(kind) {
            let button = self.build_button(element, button_kind);
            output.push(SemanticNode::Button(button));
            return;
        }
        let sensitive = kind.is_sensitive();
        let id = self.allocate_node_id();
        let name = self.attribute(element, "name");
        let label = self.control_label(element);
        let (value, checked) = self.input_value_and_checked(element, sensitive);
        output.push(SemanticNode::Input(InputElement {
            id,
            kind,
            name,
            value,
            checked,
            label,
            sensitive,
        }));
    }

    /// An input's `value` and `checked` state, or defaults for a sensitive control.
    ///
    /// A sensitive input returns before either source attribute is read, so a password
    /// value is never parsed into a local variable, let alone stored.
    fn input_value_and_checked(&self, element: usize, sensitive: bool) -> (String, bool) {
        if sensitive {
            return (String::new(), false);
        }
        let value = self
            .attribute(element, "value")
            .map(sanitize_inline)
            .unwrap_or_default();
        let checked = self.attribute(element, "checked").is_some();
        (value, checked)
    }

    /// Map a `<textarea>` into `Textarea`. Its value is the element's text content, not
    /// an attribute.
    fn push_textarea(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let id = self.allocate_node_id();
        let name = self.attribute(element, "name");
        let label = self.control_label(element);
        let value = self.plain_text_of(element);
        output.push(SemanticNode::Textarea(TextareaElement {
            id,
            name,
            value,
            label,
        }));
    }

    fn push_select(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let id = self.allocate_node_id();
        let name = self.attribute(element, "name");
        let label = self.control_label(element);
        let multiple = self.attribute(element, "multiple").is_some();
        let mut options = Vec::new();
        self.collect_options(element, &mut options);
        if !multiple {
            keep_last_selected(&mut options);
        }
        output.push(SemanticNode::Select(SelectElement {
            id,
            name,
            label,
            multiple,
            options,
        }));
    }

    /// Gather a select's options in source order, descending through `<optgroup>`
    /// wrappers so grouped options join the flat list. Collection stops once
    /// [`MAX_SELECT_OPTIONS`] options have been gathered, so a pathological `<select>`
    /// cannot grow the tree unbounded.
    fn collect_options(&self, node: usize, options: &mut Vec<SelectOption>) {
        for child in self.child_handles(node) {
            if options.len() >= MAX_SELECT_OPTIONS {
                return;
            }
            if self.is_element_named(child, "option") {
                options.push(self.build_select_option(child));
                continue;
            }
            if !self.arena[child].is_element {
                continue;
            }
            self.collect_options(child, options);
        }
    }

    /// An `<option>`'s submission data: `value` defaults to the option's own text when
    /// the source carries no `value` attribute.
    fn build_select_option(&self, node: usize) -> SelectOption {
        let label = self.plain_text_of(node);
        let value = self
            .attribute(node, "value")
            .map(sanitize_inline)
            .unwrap_or_else(|| label.clone());
        let selected = self.attribute(node, "selected").is_some();
        SelectOption {
            value,
            label,
            selected,
        }
    }

    fn push_button(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let kind = self.button_type_attribute(element);
        let button = self.build_button(element, kind);
        output.push(SemanticNode::Button(button));
    }

    /// A `<button>` element's own submission behavior, read from its `type` attribute
    /// and matched case-insensitively. An absent or unrecognized value defaults to
    /// `Submit`, matching a `<button>`'s default behavior inside a form.
    fn button_type_attribute(&self, element: usize) -> ButtonKind {
        let Some(raw) = self.attribute(element, "type") else {
            return ButtonKind::Submit;
        };
        match raw.trim().to_ascii_lowercase().as_str() {
            "reset" => ButtonKind::Reset,
            "button" => ButtonKind::Button,
            _ => ButtonKind::Submit,
        }
    }

    /// Build a `ButtonElement` for a `<button>` element or a normalized submit/reset/
    /// button `<input>`, assigning a fresh `NodeId`.
    ///
    /// A `<button>`'s label comes from its own inline content. A normalized `<input>`
    /// has no children, so its label is a single plain-text run built from its `value`
    /// attribute, falling back to a fixed default matching default browser rendering.
    fn build_button(&mut self, element: usize, kind: ButtonKind) -> ButtonElement {
        let id = self.allocate_node_id();
        let name = self.attribute(element, "name");
        let value = self.attribute(element, "value").map(sanitize_inline);
        let is_button_element = self.is_element_named(element, "button");
        let runs = if is_button_element {
            self.block_runs(element)
        } else {
            let text = value
                .clone()
                .unwrap_or_else(|| default_button_label(kind).to_string());
            vec![InlineRun::plain(text)]
        };
        ButtonElement {
            id,
            kind,
            name,
            value,
            runs,
            inline_style: self.inline_style(element),
        }
    }

    /// The accessible label of a form control.
    ///
    /// An explicit `aria-label` wins, then a `<label for=...>` matching the control's
    /// `id`, then an ancestor `<label>` that wraps the control. A control with none of
    /// these has no label.
    fn control_label(&self, element: usize) -> Option<String> {
        if let Some(aria) = self.attribute(element, "aria-label") {
            return non_empty_text(sanitize_inline(aria));
        }
        if let Some(text) = self.label_for_control(element) {
            return non_empty_text(text);
        }
        self.wrapping_label_text(element).and_then(non_empty_text)
    }

    /// The text of a `<label for=id>` associated with the control by its `id`.
    fn label_for_control(&self, element: usize) -> Option<String> {
        let id = self.attribute(element, "id")?;
        for (candidate, node) in self.arena.iter().enumerate() {
            if !node.is_element || local_name(&node.name) != "label" {
                continue;
            }
            if !attribute_equals(node, "for", &id) {
                continue;
            }
            return Some(self.plain_text_of(candidate));
        }
        None
    }

    /// The text of the nearest ancestor `<label>` that wraps the control, if any.
    fn wrapping_label_text(&self, element: usize) -> Option<String> {
        let mut ancestor = self.arena[element].parent;
        while let Some(node) = ancestor {
            if self.is_element_named(node, "label") {
                return Some(self.plain_text_of(node));
            }
            ancestor = self.arena[node].parent;
        }
        None
    }

    /// The runs of a text block, or `None` when the block collapses to nothing.
    fn non_empty_runs(&mut self, element: usize) -> Option<Vec<InlineRun>> {
        let runs = self.block_runs(element);
        if runs.is_empty() {
            return None;
        }
        Some(runs)
    }

    fn is_element_named(&self, node: usize, name: &str) -> bool {
        let entry = &self.arena[node];
        entry.is_element && local_name(&entry.name) == name
    }

    fn plain_text_of(&self, node: usize) -> String {
        let mut raw = String::new();
        gather_plain_text(self.arena, node, &mut raw);
        sanitize_inline(raw)
    }

    fn push_verbatim_block(
        &mut self,
        element: usize,
        output: &mut Vec<SemanticNode>,
        build: impl FnOnce(String) -> SemanticNode,
    ) {
        let text = self.verbatim_text(element);
        if text.is_empty() {
            return;
        }
        output.push(build(text));
    }

    fn verbatim_text(&mut self, element: usize) -> String {
        let mut raw = String::new();
        self.gather_text(element, &mut raw);
        strip_control_characters_preserving_layout(&raw)
            .trim_matches('\n')
            .to_string()
    }

    fn gather_text(&mut self, node: usize, buffer: &mut String) {
        let entry = &self.arena[node];
        if let Some(text) = &entry.text {
            buffer.push_str(text);
            return;
        }
        if entry.is_element {
            let tag = local_name(&entry.name);
            if tag == "script" {
                self.script_count += 1;
                return;
            }
            if tag == "style" {
                return;
            }
        }
        for child in self.child_handles(node) {
            self.gather_text(child, buffer);
        }
    }

    fn attribute(&self, element: usize, wanted: &str) -> Option<String> {
        self.arena[element]
            .attributes
            .iter()
            .find(|attribute| local_name(&attribute.name) == wanted)
            .map(|attribute| attribute.value.to_string())
    }

    /// The raw `style` attribute of a style-bearing element, control-stripped and kept
    /// unparsed, or `None` when the element carries no `style`.
    ///
    /// Control characters are removed here so no escape sequence from remote markup can
    /// survive into a node; CSS interpretation stays in the css layer, which parses this
    /// string during the cascade.
    fn inline_style(&self, element: usize) -> Option<String> {
        let raw = self.attribute(element, "style")?;
        Some(strip_control_characters(&raw))
    }

    fn child_handles(&self, node: usize) -> Vec<usize> {
        self.arena[node].children.clone()
    }
}

/// The emphasis, link, quote depth, and citation in force at a point during a block's
/// inline walk.
#[derive(Clone)]
struct InlineContext {
    emphasis: InlineEmphasis,
    link: Option<String>,
    quote_depth: u8,
    citation: Option<String>,
}

impl InlineContext {
    fn root() -> InlineContext {
        InlineContext {
            emphasis: InlineEmphasis::none(),
            link: None,
            quote_depth: 0,
            citation: None,
        }
    }
}

/// A contiguous span of a block's text sharing one emphasis, link, and citation, before
/// whitespace is collapsed across the whole block.
struct Segment {
    text: String,
    emphasis: InlineEmphasis,
    link: Option<String>,
    citation: Option<String>,
    anchors: Vec<String>,
}

impl Segment {
    /// A text segment that carries the anchor names pending when it was produced.
    fn with_anchors(raw: &str, context: &InlineContext, anchors: Vec<String>) -> Segment {
        Segment {
            text: strip_control_characters_preserving_layout(raw),
            emphasis: context.emphasis.clone(),
            link: context.link.clone(),
            citation: context.citation.clone(),
            anchors,
        }
    }

    /// A text-less segment that carries only anchor names, used to flush anchors left
    /// pending at the end of a block onto its last run.
    fn anchor_only(anchors: Vec<String>) -> Segment {
        Segment {
            text: String::new(),
            emphasis: InlineEmphasis::none(),
            link: None,
            citation: None,
            anchors,
        }
    }

    /// A synthetic, anchor-less segment carrying a literal marker (a quote-mark
    /// character), styled with the given context so it renders as part of the span it
    /// brackets, including any citation link that span carries.
    fn mark(text: &'static str, context: &InlineContext) -> Segment {
        Segment {
            text: text.to_string(),
            emphasis: context.emphasis.clone(),
            link: context.link.clone(),
            citation: context.citation.clone(),
            anchors: Vec::new(),
        }
    }
}

/// Collapse a block's segments into inline runs.
///
/// Whitespace is collapsed across the whole block: each run of whitespace becomes a
/// single space and leading and trailing whitespace is dropped, matching how plain block
/// text is collapsed. A new run begins wherever the emphasis or link changes.
fn collapse_segments(segments: Vec<Segment>) -> Vec<InlineRun> {
    let mut builder = RunBuilder::new();
    for segment in &segments {
        builder.push_segment(segment);
    }
    builder.finish()
}

/// The opening and closing quote-mark characters for a `<q>` nested at the given depth.
///
/// Depth 1 (an outermost `<q>`) uses curly double quotes; depth 2 and deeper reuses curly
/// single quotes, matching the CSS UA-stylesheet default's own two-level alternation with
/// no further nesting distinction. `depth` is never `0` here: it is only reached from
/// `gather_quote_segments`, which always increments before calling this function.
fn quote_marks(depth: u8) -> (&'static str, &'static str) {
    if depth % 2 == 1 {
        return ("\u{201C}", "\u{201D}");
    }
    ("\u{2018}", "\u{2019}")
}

/// Builds inline runs from a block's segments, collapsing whitespace as it goes.
struct RunBuilder {
    runs: Vec<InlineRun>,
    current: Option<InlineRun>,
    last_was_space: bool,
    pending_anchors: Vec<String>,
}

impl RunBuilder {
    fn new() -> RunBuilder {
        RunBuilder {
            runs: Vec::new(),
            current: None,
            // Start as though a space preceded the block so leading whitespace is dropped.
            last_was_space: true,
            pending_anchors: Vec::new(),
        }
    }

    fn push_segment(&mut self, segment: &Segment) {
        push_unique_anchors(&mut self.pending_anchors, &segment.anchors);
        for character in segment.text.chars() {
            self.push_character(
                character,
                &segment.emphasis,
                &segment.link,
                &segment.citation,
            );
        }
    }

    fn push_character(
        &mut self,
        character: char,
        emphasis: &InlineEmphasis,
        link: &Option<String>,
        citation: &Option<String>,
    ) {
        if character.is_whitespace() {
            self.push_space(emphasis, link, citation);
            return;
        }
        self.push_visible(character, emphasis, link, citation);
    }

    fn push_visible(
        &mut self,
        character: char,
        emphasis: &InlineEmphasis,
        link: &Option<String>,
        citation: &Option<String>,
    ) {
        self.open_run(emphasis, link, citation);
        self.absorb_pending_anchors();
        if let Some(run) = self.current.as_mut() {
            run.text.push(character);
        }
        self.last_was_space = false;
    }

    /// Attach the anchors pending on the run now open, so the first visible text after an
    /// anchor carries it. Absorbing only on visible text keeps a collapsed space between an
    /// anchor and the text it targets from landing the anchor on the wrong run.
    fn absorb_pending_anchors(&mut self) {
        if self.pending_anchors.is_empty() {
            return;
        }
        let Some(run) = self.current.as_mut() else {
            return;
        };
        let names = std::mem::take(&mut self.pending_anchors);
        push_unique_anchors(&mut run.anchors, &names);
    }

    fn push_space(
        &mut self,
        emphasis: &InlineEmphasis,
        link: &Option<String>,
        citation: &Option<String>,
    ) {
        if self.last_was_space {
            return;
        }
        self.open_run(emphasis, link, citation);
        if let Some(run) = self.current.as_mut() {
            run.text.push(' ');
        }
        self.last_was_space = true;
    }

    /// Ensure the current run carries the given emphasis, link, and citation, flushing it
    /// and starting a fresh run when any of the three differs.
    fn open_run(
        &mut self,
        emphasis: &InlineEmphasis,
        link: &Option<String>,
        citation: &Option<String>,
    ) {
        if self.run_matches(emphasis, link, citation) {
            return;
        }
        self.flush_current();
        self.current = Some(InlineRun {
            text: String::new(),
            emphasis: emphasis.clone(),
            link: link.clone(),
            citation: citation.clone(),
            anchors: Vec::new(),
        });
    }

    fn run_matches(
        &self,
        emphasis: &InlineEmphasis,
        link: &Option<String>,
        citation: &Option<String>,
    ) -> bool {
        match &self.current {
            Some(run) => {
                &run.emphasis == emphasis && &run.link == link && &run.citation == citation
            }
            None => false,
        }
    }

    fn flush_current(&mut self) {
        let Some(run) = self.current.take() else {
            return;
        };
        if run.text.is_empty() {
            return;
        }
        self.runs.push(run);
    }

    fn finish(mut self) -> Vec<InlineRun> {
        self.trim_trailing_space();
        self.flush_current();
        self.attach_trailing_anchors();
        self.runs
    }

    /// Attach anchors still pending after the last run to that run, or emit a text-less run
    /// to hold them when the block produced no run at all. Either way an anchor that trails
    /// the block's text is never lost.
    fn attach_trailing_anchors(&mut self) {
        if self.pending_anchors.is_empty() {
            return;
        }
        let names = std::mem::take(&mut self.pending_anchors);
        if let Some(last) = self.runs.last_mut() {
            push_unique_anchors(&mut last.anchors, &names);
            return;
        }
        self.runs.push(InlineRun {
            text: String::new(),
            emphasis: InlineEmphasis::none(),
            link: None,
            citation: None,
            anchors: names,
        });
    }

    /// Drop a single trailing space left on the final run, matching how plain block text
    /// trims its trailing whitespace.
    fn trim_trailing_space(&mut self) {
        let Some(run) = self.current.as_mut() else {
            return;
        };
        if run.text.ends_with(' ') {
            run.text.pop();
        }
    }
}

/// Decode an anchor name into the form a fragment is compared against.
///
/// The value is percent-decoded so `id="a%20b"` and a link to `#a%20b` name the same
/// target, then control-stripped so no escape sequence from remote markup survives in a
/// name that later reaches the terminal.
fn decode_anchor_name(raw: &str) -> String {
    let decoded = percent_decode_str(raw).decode_utf8_lossy();
    strip_control_characters(&decoded)
}

/// Append each name not already present, preserving order.
///
/// Anchor names are compared by value, and an `id` is unique in a document, so appending
/// only new names collapses the duplicate an element produces when both the block walk and
/// the inline walk record it.
fn push_unique_anchors(destination: &mut Vec<String>, names: &[String]) {
    for name in names {
        if destination.contains(name) {
            continue;
        }
        destination.push(name.clone());
    }
}

/// Resolve a reference against the document base URL when one is present.
///
/// A reference that cannot be resolved is kept exactly as authored rather than dropped,
/// so a malformed base or reference never loses the link or image source.
fn resolve_reference(reference: &str, base_url: Option<&Url>) -> String {
    let Some(base) = base_url else {
        return reference.to_string();
    };
    match base.join(reference) {
        Ok(resolved) => resolved.to_string(),
        Err(_) => reference.to_string(),
    }
}

/// Determine the base URL relative references resolve against.
///
/// A `<base href>` wins: it resolves against `document_url` when relative and is taken
/// as written when absolute. With no usable `<base href>`, `document_url` is the base.
/// With neither, the result is `None` and references are left exactly as authored.
fn resolve_base_url(arena: &[Node], document_url: Option<Url>) -> Option<Url> {
    let Some(href) = find_base_href(arena, DOCUMENT_HANDLE) else {
        return document_url;
    };
    let sanitized = sanitize_reference(href);
    let declared = match document_url.as_ref() {
        Some(document) => document.join(&sanitized).ok(),
        None => Url::parse(&sanitized).ok(),
    };
    declared.or(document_url)
}

fn find_base_href(arena: &[Node], node: usize) -> Option<String> {
    let entry = &arena[node];
    if let Some(href) = base_href_attribute(entry) {
        return Some(href);
    }
    for child in &entry.children {
        if let Some(found) = find_base_href(arena, *child) {
            return Some(found);
        }
    }
    None
}

fn base_href_attribute(entry: &Node) -> Option<String> {
    if !entry.is_element {
        return None;
    }
    if local_name(&entry.name) != "base" {
        return None;
    }
    entry
        .attributes
        .iter()
        .find(|attribute| local_name(&attribute.name) == "href")
        .map(|attribute| attribute.value.to_string())
}

fn extract_title(arena: &[Node]) -> Option<String> {
    let title = find_title(arena, DOCUMENT_HANDLE)?;
    let mut raw = String::new();
    gather_plain_text(arena, title, &mut raw);
    Some(raw)
}

fn find_title(arena: &[Node], node: usize) -> Option<usize> {
    let entry = &arena[node];
    if entry.is_element && local_name(&entry.name) == "title" {
        return Some(node);
    }
    for child in &entry.children {
        if let Some(found) = find_title(arena, *child) {
            return Some(found);
        }
    }
    None
}

fn gather_plain_text(arena: &[Node], node: usize, buffer: &mut String) {
    let entry = &arena[node];
    if let Some(text) = &entry.text {
        buffer.push_str(text);
        return;
    }
    for child in &entry.children {
        gather_plain_text(arena, *child, buffer);
    }
}

/// Whether a tag maps to a block-level semantic node.
///
/// Inline `<code>` is intentionally absent: inside a text block it folds into an inline
/// run's emphasis rather than standing alone, so it is treated as inline content here.
fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "ul"
            | "ol"
            | "blockquote"
            | "pre"
            | "hr"
            | "img"
            | "table"
            | "figure"
            | "details"
            | "summary"
            | "form"
            | "input"
            | "textarea"
            | "select"
            | "button"
            | "iframe"
            | "object"
            | "embed"
            | "video"
            | "audio"
            | "nav"
            | "main"
            | "aside"
            | "footer"
            | "header"
            | "section"
    ) || heading_level(tag).is_some()
}

/// A human-readable name for an embedded-content element, used as its placeholder label.
fn embedded_label(tag: &str) -> &'static str {
    match tag {
        "iframe" => "inline frame",
        "object" => "object",
        "embed" => "embedded object",
        "video" => "video",
        "audio" => "audio",
        _ => "embedded content",
    }
}

/// Whether an element carries an attribute whose value equals `wanted`.
fn attribute_equals(entry: &Node, name: &str, wanted: &str) -> bool {
    entry
        .attributes
        .iter()
        .any(|attribute| local_name(&attribute.name) == name && attribute.value.as_ref() == wanted)
}

/// The text, or `None` when it is empty, so a blank label collapses to no label.
fn non_empty_text(text: String) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    Some(text)
}

fn is_table_section(tag: &str) -> bool {
    matches!(tag, "thead" | "tbody" | "tfoot")
}

/// Truncate every row to [`MAX_TABLE_COLUMNS`] cells, reporting whether any row was cut.
fn truncate_row_columns(rows: &mut [SemanticNode]) -> bool {
    let mut truncated = false;
    for row in rows.iter_mut() {
        truncated |= truncate_one_row(row);
    }
    truncated
}

fn truncate_one_row(row: &mut SemanticNode) -> bool {
    let SemanticNode::TableRow { children } = row else {
        return false;
    };
    if children.len() <= MAX_TABLE_COLUMNS {
        return false;
    }
    children.truncate(MAX_TABLE_COLUMNS);
    true
}

fn table_truncation_message(rows_truncated: bool, columns_truncated: bool) -> String {
    if rows_truncated && columns_truncated {
        return format!(
            "A large table was truncated to its first {MAX_TABLE_ROWS} rows and {MAX_TABLE_COLUMNS} columns"
        );
    }
    if rows_truncated {
        return format!("A large table was truncated to its first {MAX_TABLE_ROWS} rows");
    }
    format!("A large table was truncated to its first {MAX_TABLE_COLUMNS} columns")
}

/// The button behavior an `<input>`'s type normalizes to, or `None` when it is not a
/// submit/reset/button control and should stay an `Input`.
fn button_kind_for_input(kind: InputKind) -> Option<ButtonKind> {
    match kind {
        InputKind::Submit => Some(ButtonKind::Submit),
        InputKind::Reset => Some(ButtonKind::Reset),
        InputKind::Button => Some(ButtonKind::Button),
        _ => None,
    }
}

/// The fixed label a normalized `<input type=submit|reset|button>` renders when it has
/// no `value` attribute, matching default browser rendering.
fn default_button_label(kind: ButtonKind) -> &'static str {
    match kind {
        ButtonKind::Submit => "Submit",
        ButtonKind::Reset => "Reset",
        ButtonKind::Button => "Button",
    }
}

/// Keep only the last selected option when a non-multiple select's markup selects more
/// than one, matching how a browser resolves the invalid-but-common case rather than
/// rejecting the page.
fn keep_last_selected(options: &mut [SelectOption]) {
    let Some(last_selected) = options.iter().rposition(|option| option.selected) else {
        return;
    };
    for (index, option) in options.iter_mut().enumerate() {
        option.selected = index == last_selected;
    }
}

fn heading_level(tag: &str) -> Option<u8> {
    match tag {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

fn sanitize_inline(raw: String) -> String {
    collapse_whitespace(&strip_control_characters_preserving_layout(&raw))
}

fn sanitize_reference(raw: String) -> String {
    strip_control_characters(&raw).trim().to_string()
}

fn suppressed_scripts_message(count: usize) -> String {
    if count == 1 {
        return "1 script element was ignored and not executed".to_string();
    }
    format!("{count} script elements were ignored and not executed")
}

fn local_name(name: &QualName) -> &str {
    &name.local
}

fn empty_name() -> QualName {
    QualName::new(None, Namespace::from(""), LocalName::from(""))
}
