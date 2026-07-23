// @file crates/browser-core/src/current_page.rs
// @description Holds the single page the navigation controller has loaded and can render.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_html::{Document, DocumentTitle};
use browser_network::BrowserUrl;

/// The page currently held by the navigation controller.
///
/// This milestone tracks one page at a time, not a set of tabs. It keeps the URL the
/// fetch finally resolved to (after any redirects), the parsed document, and the
/// document's title so an adapter can show it without walking the node stream.
#[derive(Debug)]
pub(crate) struct CurrentPage {
    final_url: BrowserUrl,
    document: Document,
    title: Option<DocumentTitle>,
}

impl CurrentPage {
    pub(crate) fn new(
        final_url: BrowserUrl,
        document: Document,
        title: Option<DocumentTitle>,
    ) -> Self {
        Self {
            final_url,
            document,
            title,
        }
    }

    pub(crate) fn final_url(&self) -> &BrowserUrl {
        &self.final_url
    }

    pub(crate) fn document(&self) -> &Document {
        &self.document
    }

    pub(crate) fn title(&self) -> Option<&DocumentTitle> {
        self.title.as_ref()
    }
}
