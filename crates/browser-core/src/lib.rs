//! @file crates/browser-core/src/lib.rs
//! @description Core crate root: tab and navigation domain types, error taxonomy, controller.
//! @layer core
//! @created meerita <meerita@icloud.com>

mod current_page;
mod error;
mod ids;
mod tab_id;
mod tab_state;

pub use browser_html::{Document, DocumentTitle};
pub use browser_layout::CellBuffer;
pub use browser_network::BrowserUrl;
pub use error::CoreError;
pub use ids::{BookmarkId, HistoryEntryId};
pub use tab_id::TabId;
pub use tab_state::TabState;

use current_page::CurrentPage;

/// Orchestrates the fetch, parse, and render pipeline for the loaded page.
///
/// This is the application core the output adapters build on. This milestone holds a
/// single page: [`load`](Self::load) fetches and parses it, and [`render`](Self::render)
/// lays it out on demand for a given terminal width. Tabs, history, forms, and downloads
/// are not implemented yet.
#[derive(Debug, Default)]
pub struct NavigationController {
    current_page: Option<CurrentPage>,
}

impl NavigationController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fetch and parse the document at `url`, then hold it as the current page.
    ///
    /// The fetch follows redirects, so the stored URL is the one the request finally
    /// resolved to, not necessarily `url`. Layout is deliberately not run here; it runs
    /// in [`render`](Self::render) because it depends on the terminal width. Network and
    /// parse failures are mapped into [`CoreError`] by `?`.
    pub async fn load(&mut self, url: BrowserUrl) -> Result<(), CoreError> {
        let fetched = browser_network::fetch(&url).await?;
        let document = browser_html::parse_html(fetched.body_bytes(), fetched.charset())?;
        let title = document.title().cloned();
        self.current_page = Some(CurrentPage::new(
            fetched.final_url().clone(),
            document,
            title,
        ));
        Ok(())
    }

    /// Lay the current page out into a cell buffer sized to `width` columns.
    ///
    /// With no page loaded this returns a blank buffer rather than an error, so the
    /// terminal can draw an empty page. With a page loaded it runs layout and maps a
    /// [`LayoutError`](browser_layout::LayoutError) into [`CoreError`] by `?`.
    pub fn render(&self, width: u16) -> Result<CellBuffer, CoreError> {
        let Some(page) = &self.current_page else {
            return Ok(CellBuffer::new(width, 0));
        };
        let buffer = browser_layout::render_document(
            page.document(),
            width,
            &browser_layout::WidthConfig::default(),
        )?;
        Ok(buffer)
    }

    /// The title of the current page, if one is loaded and it declared a title.
    pub fn current_title(&self) -> Option<&DocumentTitle> {
        self.current_page.as_ref().and_then(CurrentPage::title)
    }

    /// The URL the current page finally resolved to, if one is loaded.
    pub fn current_url(&self) -> Option<&BrowserUrl> {
        self.current_page.as_ref().map(CurrentPage::final_url)
    }

    /// Whether a page is currently loaded.
    pub fn has_page(&self) -> bool {
        self.current_page.is_some()
    }

    /// How many `<script>` elements the current page suppressed, or zero when no page is
    /// loaded.
    pub fn script_count(&self) -> usize {
        match &self.current_page {
            Some(page) => page.document().script_count(),
            None => 0,
        }
    }

    /// Loads the document at the given location into the active tab.
    ///
    /// Not implemented in this milestone; returns [`CoreError::NavigationFailed`].
    pub fn navigate(&mut self, _location: &str) -> Result<(), CoreError> {
        Err(CoreError::NavigationFailed)
    }

    /// Closes the tab with the given identifier.
    ///
    /// Not implemented in this milestone; returns [`CoreError::TabNotFound`].
    pub fn close_tab(&mut self, _tab: TabId) -> Result<(), CoreError> {
        Err(CoreError::TabNotFound)
    }
}
