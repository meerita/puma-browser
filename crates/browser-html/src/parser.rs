// @file crates/browser-html/src/parser.rs
// @description Parses HTML into a recursive Document tree of sanitized, script-free nodes.
// @layer html
// @created meerita <meerita@icloud.com>

use std::borrow::Cow;
use std::cell::{Cell, Ref, RefCell};

use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::{parse_document, Attribute, LocalName, Namespace, ParseOpts, QualName};

use crate::document::{Document, DocumentTitle};
use crate::error::HtmlError;
use crate::inline_run::InlineRun;
use crate::sanitize::{
    collapse_whitespace, strip_control_characters, strip_control_characters_preserving_layout,
};
use crate::semantic_node::SemanticNode;

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

/// Parse an HTML source string into a recursive [`Document`] tree.
///
/// All text taken from the source is stripped of control characters before it enters a
/// node or the title, `<script>` elements are counted and never retained, and the node
/// count and nesting depth are bounded so untrusted input cannot exhaust memory.
pub fn parse_html(source: &str) -> Result<Document, HtmlError> {
    let dom = parse_document(DomBuilder::new(), ParseOpts::default()).one(source);

    if let Some(error) = dom.limit_error() {
        return Err(error);
    }

    let arena = dom.into_nodes();
    let title = extract_title(&arena).map(|raw| DocumentTitle::new(&raw));

    let mut extractor = TreeExtractor::new(&arena);
    let mut children = extractor.walk_children(DOCUMENT_HANDLE);
    let script_count = extractor.script_count();

    if script_count > 0 {
        children.push(SemanticNode::Warning {
            message: suppressed_scripts_message(script_count),
        });
    }

    Ok(Document::new(children, title, script_count))
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
/// accumulates across the whole walk so it can be reported once parsing ends.
struct TreeExtractor<'a> {
    arena: &'a [Node],
    script_count: usize,
}

impl<'a> TreeExtractor<'a> {
    fn new(arena: &'a [Node]) -> TreeExtractor<'a> {
        TreeExtractor {
            arena,
            script_count: 0,
        }
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
            "hr" => output.push(SemanticNode::Separator),
            "img" => self.push_image(element, output),
            _ => self.walk_children_into(element, output),
        }
    }

    fn push_heading(&mut self, element: usize, level: u8, output: &mut Vec<SemanticNode>) {
        let Some(runs) = self.single_run(element) else {
            return;
        };
        output.push(SemanticNode::Heading { level, runs });
    }

    fn push_paragraph(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let Some(runs) = self.single_run(element) else {
            return;
        };
        output.push(SemanticNode::Paragraph { runs });
    }

    fn push_list(&mut self, element: usize, ordered: bool, output: &mut Vec<SemanticNode>) {
        let children = self.list_items(element);
        if children.is_empty() {
            return;
        }
        output.push(SemanticNode::List { ordered, children });
    }

    fn push_quote(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let children = self.block_children(element);
        if children.is_empty() {
            return;
        }
        output.push(SemanticNode::Quote { children });
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
        items.push(SemanticNode::ListItem { children });
    }

    fn is_list_item_element(&self, node: usize) -> bool {
        let entry = &self.arena[node];
        entry.is_element && local_name(&entry.name) == "li"
    }

    /// Gather the block-level children of a container.
    ///
    /// When the container holds only inline text with no block wrapper (a `<li>` or
    /// `<blockquote>` whose content is bare text), that text becomes a single paragraph
    /// so the content is not lost.
    fn block_children(&mut self, element: usize) -> Vec<SemanticNode> {
        let walked = self.walk_children(element);
        if !walked.is_empty() {
            return walked;
        }
        let Some(runs) = self.single_run(element) else {
            return Vec::new();
        };
        vec![SemanticNode::Paragraph { runs }]
    }

    /// Gather a text block's content into exactly one plain run.
    ///
    /// Multiple runs and inline emphasis are a later phase; every text block produces a
    /// single unstyled run here. An empty block yields no run so it is dropped upstream.
    fn single_run(&mut self, element: usize) -> Option<Vec<InlineRun>> {
        let text = self.block_text(element);
        if text.is_empty() {
            return None;
        }
        Some(vec![InlineRun::plain(text)])
    }

    fn push_image(&mut self, element: usize, output: &mut Vec<SemanticNode>) {
        let alt = self
            .attribute(element, "alt")
            .map(sanitize_inline)
            .unwrap_or_default();
        let title = self.attribute(element, "title").map(sanitize_inline);
        let source = self.attribute(element, "src").map(sanitize_reference);
        output.push(SemanticNode::ImagePlaceholder { alt, title, source });
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

    fn block_text(&mut self, element: usize) -> String {
        let mut raw = String::new();
        self.gather_text(element, &mut raw);
        collapse_whitespace(&strip_control_characters_preserving_layout(&raw))
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

    fn child_handles(&self, node: usize) -> Vec<usize> {
        self.arena[node].children.clone()
    }
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
