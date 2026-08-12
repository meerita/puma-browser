// @file crates/browser-core/src/current_page.rs
// @description Holds the single page the navigation controller has loaded and can render.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_html::{Document, DocumentTitle};
use browser_network::BrowserUrl;

use crate::form_field_values::FormFieldValues;

/// The page currently held by the navigation controller.
///
/// This milestone tracks one page at a time, not a set of tabs. It keeps the URL the
/// fetch finally resolved to (after any redirects), the parsed document, the
/// document's title so an adapter can show it without walking the node stream, the
/// raw byte count of the response body, how many cookies this navigation accepted
/// and rejected so an adapter can show a per-page indicator, and the page's live form
/// field state.
#[derive(Debug)]
pub(crate) struct CurrentPage {
    final_url: BrowserUrl,
    document: Document,
    title: Option<DocumentTitle>,
    byte_count: usize,
    accepted_cookie_count: usize,
    rejected_cookie_count: usize,
    form_field_values: FormFieldValues,
}

impl CurrentPage {
    pub(crate) fn new(
        final_url: BrowserUrl,
        document: Document,
        title: Option<DocumentTitle>,
        byte_count: usize,
        accepted_cookie_count: usize,
        rejected_cookie_count: usize,
        form_field_values: FormFieldValues,
    ) -> Self {
        Self {
            final_url,
            document,
            title,
            byte_count,
            accepted_cookie_count,
            rejected_cookie_count,
            form_field_values,
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

    pub(crate) fn byte_count(&self) -> usize {
        self.byte_count
    }

    pub(crate) fn accepted_cookie_count(&self) -> usize {
        self.accepted_cookie_count
    }

    pub(crate) fn rejected_cookie_count(&self) -> usize {
        self.rejected_cookie_count
    }

    pub(crate) fn form_field_values(&self) -> &FormFieldValues {
        &self.form_field_values
    }

    pub(crate) fn form_field_values_mut(&mut self) -> &mut FormFieldValues {
        &mut self.form_field_values
    }

    /// Splits the page into its parsed document and its live field state, so a caller
    /// can resolve a control against the document and reseed the state that same
    /// lookup found, without the two borrows aliasing the whole page.
    pub(crate) fn document_and_form_field_values_mut(
        &mut self,
    ) -> (&Document, &mut FormFieldValues) {
        (&self.document, &mut self.form_field_values)
    }
}
