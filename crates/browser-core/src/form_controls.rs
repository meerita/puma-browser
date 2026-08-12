// @file crates/browser-core/src/form_controls.rs
// @description Recursive walk over a parsed document's form controls, stopping at nested form boundaries.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_html::{ButtonElement, Document, FormElement, NodeId, SemanticNode};

/// Every `Input`/`Select`/`Textarea` descendant of `form`, in document order.
///
/// Does not recurse into a nested `SemanticNode::Form`: a control belongs to the
/// nearest enclosing form. Buttons are not collected here; submission gathers the
/// activated button separately through [`find_button_in_form`].
pub(crate) fn collect_controls(form: &FormElement) -> Vec<&SemanticNode> {
    let mut controls = Vec::new();
    push_controls(&form.children, &mut controls);
    controls
}

fn push_controls<'a>(children: &'a [SemanticNode], controls: &mut Vec<&'a SemanticNode>) {
    for child in children {
        push_control(child, controls);
    }
}

fn push_control<'a>(node: &'a SemanticNode, controls: &mut Vec<&'a SemanticNode>) {
    let is_collectible = matches!(
        node,
        SemanticNode::Input(_) | SemanticNode::Select(_) | SemanticNode::Textarea(_)
    );
    if is_collectible {
        controls.push(node);
        return;
    }
    if matches!(node, SemanticNode::Form(_)) {
        return;
    }
    if let Some(children) = children_of(node) {
        push_controls(children, controls);
    }
}

/// The enclosing `FormElement` for a `Form`/`Input`/`Select`/`Textarea`/`Button` id, or
/// `None` if the id is unknown or belongs to no form.
///
/// Used by radio-toggling and by submission to resolve a control's or button's owning
/// form.
pub(crate) fn find_enclosing_form(document: &Document, id: NodeId) -> Option<&FormElement> {
    find_form_in(document.children(), id)
}

fn find_form_in(children: &[SemanticNode], id: NodeId) -> Option<&FormElement> {
    children
        .iter()
        .find_map(|child| form_containing_id(child, id))
}

fn form_containing_id(node: &SemanticNode, id: NodeId) -> Option<&FormElement> {
    if let SemanticNode::Form(form) = node {
        return form_if_owns(form, id);
    }
    children_of(node).and_then(|children| find_form_in(children, id))
}

fn form_if_owns(form: &FormElement, id: NodeId) -> Option<&FormElement> {
    let owns = form.id == id || find_control_by_id(&form.children, id).is_some();
    if owns {
        return Some(form);
    }
    None
}

/// The form control or button node whose id is `id`, anywhere in `document`, or `None`
/// if the id is unknown or belongs to no form.
///
/// Lets an output adapter inspect a control's static shape (its input kind, its select
/// options, its button kind) by id, reusing the same form-tree walk submission and
/// radio-grouping already use, rather than duplicating it.
pub(crate) fn find_control(document: &Document, id: NodeId) -> Option<&SemanticNode> {
    let form = find_enclosing_form(document, id)?;
    find_control_by_id(&form.children, id)
}

/// The button within `form` whose id is `id`, or `None` if `id` does not name one of
/// its buttons.
///
/// Searched the same way [`collect_controls`] walks a form's controls: stopping at a
/// nested form boundary.
pub(crate) fn find_button_in_form(form: &FormElement, id: NodeId) -> Option<&ButtonElement> {
    match find_control_by_id(&form.children, id) {
        Some(SemanticNode::Button(button)) => Some(button),
        _ => None,
    }
}

/// The form control or button node whose id is `id`, among `children` and their
/// descendants, not recursing past a nested `SemanticNode::Form`.
fn find_control_by_id(children: &[SemanticNode], id: NodeId) -> Option<&SemanticNode> {
    children.iter().find_map(|child| control_with_id(child, id))
}

fn control_with_id(node: &SemanticNode, id: NodeId) -> Option<&SemanticNode> {
    if control_id(node) == Some(id) {
        return Some(node);
    }
    if matches!(node, SemanticNode::Form(_)) {
        return None;
    }
    children_of(node).and_then(|children| find_control_by_id(children, id))
}

/// The stable id of a node that is itself a form control or button, or `None` for any
/// other node shape.
fn control_id(node: &SemanticNode) -> Option<NodeId> {
    match node {
        SemanticNode::Input(input) => Some(input.id),
        SemanticNode::Select(select) => Some(select.id),
        SemanticNode::Textarea(textarea) => Some(textarea.id),
        SemanticNode::Button(button) => Some(button.id),
        SemanticNode::Heading { .. }
        | SemanticNode::Paragraph { .. }
        | SemanticNode::List { .. }
        | SemanticNode::ListItem { .. }
        | SemanticNode::Table { .. }
        | SemanticNode::TableRow { .. }
        | SemanticNode::TableCell { .. }
        | SemanticNode::Quote { .. }
        | SemanticNode::CodeBlock { .. }
        | SemanticNode::PreformattedBlock { .. }
        | SemanticNode::ImagePlaceholder { .. }
        | SemanticNode::Figure { .. }
        | SemanticNode::Form(_)
        | SemanticNode::Separator
        | SemanticNode::Landmark { .. }
        | SemanticNode::Details { .. }
        | SemanticNode::Summary { .. }
        | SemanticNode::EmbeddedContent { .. }
        | SemanticNode::Warning { .. } => None,
    }
}

/// The children of a container variant, or `None` for a leaf or form-control variant.
///
/// Shared by every walk in this module, and by the load-time field seeding walk, so
/// they all recurse through the same set of container shapes.
pub(crate) fn children_of(node: &SemanticNode) -> Option<&[SemanticNode]> {
    match node {
        SemanticNode::List { children, .. }
        | SemanticNode::ListItem { children, .. }
        | SemanticNode::Table { children }
        | SemanticNode::TableRow { children }
        | SemanticNode::TableCell { children, .. }
        | SemanticNode::Quote { children, .. }
        | SemanticNode::Figure { children, .. }
        | SemanticNode::Landmark { children, .. }
        | SemanticNode::Details { children, .. } => Some(children),
        SemanticNode::Heading { .. }
        | SemanticNode::Paragraph { .. }
        | SemanticNode::CodeBlock { .. }
        | SemanticNode::PreformattedBlock { .. }
        | SemanticNode::ImagePlaceholder { .. }
        | SemanticNode::Separator
        | SemanticNode::Summary { .. }
        | SemanticNode::EmbeddedContent { .. }
        | SemanticNode::Warning { .. }
        | SemanticNode::Form(_)
        | SemanticNode::Input(_)
        | SemanticNode::Select(_)
        | SemanticNode::Textarea(_)
        | SemanticNode::Button(_) => None,
    }
}
