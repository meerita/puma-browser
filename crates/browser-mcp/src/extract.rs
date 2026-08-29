// @file crates/browser-mcp/src/extract.rs
// @description Text and link extraction from the SemanticNode tree for MCP responses.
// @layer mcp
// @created meerita <meerita@icloud.com>

use browser_html::{InlineRun, SemanticNode};

/// Whether an MCP link entry comes from an author-intended hyperlink (`<a href>`) or a
/// citation (`<q cite>`), so a client can tell the two apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LinkKind {
    Hyperlink,
    Citation,
}

#[derive(Debug, serde::Serialize)]
pub(crate) struct LinkEntry {
    pub text: String,
    pub url: String,
    pub kind: LinkKind,
}

pub(crate) fn extract_text(nodes: &[SemanticNode]) -> String {
    let mut output = String::new();
    for node in nodes {
        let block = node_to_text(node);
        if !block.is_empty() {
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&block);
        }
    }
    output
}

pub(crate) fn extract_links(nodes: &[SemanticNode]) -> Vec<LinkEntry> {
    let mut links = Vec::new();
    for node in nodes {
        collect_links(node, &mut links);
    }
    links
}

fn node_to_text(node: &SemanticNode) -> String {
    match node {
        SemanticNode::Heading { level, runs, .. } => {
            let prefix = "#".repeat(*level as usize);
            format!("{} {}", prefix, runs_to_text(runs))
        }
        SemanticNode::Paragraph { runs, .. } => runs_to_text(runs),
        SemanticNode::List { children, .. }
        | SemanticNode::ListItem { children, .. }
        | SemanticNode::Quote { children, .. }
        | SemanticNode::Landmark { children, .. }
        | SemanticNode::Details { children, .. }
        | SemanticNode::Table { children }
        | SemanticNode::TableRow { children }
        | SemanticNode::TableCell { children, .. } => extract_text(children),
        SemanticNode::Form(form) => extract_text(&form.children),
        SemanticNode::Figure { children, caption } => {
            let mut parts = extract_text(children);
            if let Some(caption_runs) = caption {
                let caption_text = runs_to_text(caption_runs);
                if !caption_text.is_empty() {
                    if !parts.is_empty() {
                        parts.push('\n');
                    }
                    parts.push_str(&caption_text);
                }
            }
            parts
        }
        SemanticNode::CodeBlock { text } | SemanticNode::PreformattedBlock { text } => text.clone(),
        SemanticNode::ImagePlaceholder { alt, .. } => {
            if alt.is_empty() {
                String::new()
            } else {
                format!("[Image: {}]", alt)
            }
        }
        SemanticNode::Summary { runs, .. } => runs_to_text(runs),
        SemanticNode::Button(button) => runs_to_text(&button.runs),
        SemanticNode::Separator => "---".to_string(),
        SemanticNode::EmbeddedContent { label } => format!("[Embedded: {}]", label),
        SemanticNode::Warning { message } => format!("[Warning: {}]", message),
        // A fragment target renders nothing, so it contributes no text. Its names come
        // from remote markup and must never reach an agent as page content.
        SemanticNode::AnchorTarget { .. } => String::new(),
        SemanticNode::Input(input) => input.label.as_deref().unwrap_or("").to_string(),
        SemanticNode::Select(select) => select.label.as_deref().unwrap_or("").to_string(),
        SemanticNode::Textarea(textarea) => textarea.label.as_deref().unwrap_or("").to_string(),
    }
}

fn runs_to_text(runs: &[InlineRun]) -> String {
    runs.iter()
        .map(|run| run.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn collect_links(node: &SemanticNode, output: &mut Vec<LinkEntry>) {
    match node {
        SemanticNode::Heading { runs, .. }
        | SemanticNode::Paragraph { runs, .. }
        | SemanticNode::Summary { runs, .. } => {
            collect_links_from_runs(runs, output);
        }
        SemanticNode::Button(button) => collect_links_from_runs(&button.runs, output),
        SemanticNode::List { children, .. }
        | SemanticNode::ListItem { children, .. }
        | SemanticNode::Quote { children, .. }
        | SemanticNode::Landmark { children, .. }
        | SemanticNode::Details { children, .. }
        | SemanticNode::Table { children }
        | SemanticNode::TableRow { children }
        | SemanticNode::TableCell { children, .. } => {
            for child in children {
                collect_links(child, output);
            }
        }
        SemanticNode::Form(form) => {
            for child in &form.children {
                collect_links(child, output);
            }
        }
        SemanticNode::Figure { children, caption } => {
            for child in children {
                collect_links(child, output);
            }
            if let Some(caption_runs) = caption {
                collect_links_from_runs(caption_runs, output);
            }
        }
        SemanticNode::CodeBlock { .. }
        | SemanticNode::PreformattedBlock { .. }
        | SemanticNode::ImagePlaceholder { .. }
        | SemanticNode::Separator
        | SemanticNode::EmbeddedContent { .. }
        | SemanticNode::Warning { .. }
        | SemanticNode::Input(_)
        | SemanticNode::Select(_)
        | SemanticNode::Textarea(_)
        | SemanticNode::AnchorTarget { .. } => {}
    }
}

fn collect_links_from_runs(runs: &[InlineRun], output: &mut Vec<LinkEntry>) {
    for run in runs {
        if let Some(url) = &run.link {
            if !url.is_empty() {
                output.push(LinkEntry {
                    text: run.text.clone(),
                    url: url.clone(),
                    kind: LinkKind::Hyperlink,
                });
            }
        }
        if let Some(url) = &run.citation {
            if !url.is_empty() {
                output.push(LinkEntry {
                    text: run.text.clone(),
                    url: url.clone(),
                    kind: LinkKind::Citation,
                });
            }
        }
    }
}

#[cfg(test)]
#[path = "extract_tests.rs"]
mod tests;
