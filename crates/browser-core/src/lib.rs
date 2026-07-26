//! @file crates/browser-core/src/lib.rs
//! @description Core crate root: tab and navigation domain types, error taxonomy, controller.
//! @layer core
//! @created meerita <meerita@icloud.com>

mod address_resolver;
mod current_page;
mod error;
mod ids;
mod navigation_target;
mod tab_id;
mod tab_state;

pub use address_resolver::resolve_address;
pub use browser_html::{Document, DocumentTitle};
pub use browser_layout::CellBuffer;
pub use browser_network::BrowserUrl;
pub use error::CoreError;
pub use ids::{BookmarkId, HistoryEntryId};
pub use navigation_target::{classify_navigation, NavigationTarget};
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
    history_stack: Vec<CurrentPage>,
}

/// The most pages the back history retains. Older pages are dropped when the stack
/// overflows so a long browsing session cannot grow memory without bound.
const MAX_HISTORY_DEPTH: usize = 50;

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
        let (progress_tx, _) = tokio::sync::watch::channel(0usize);
        self.load_with_progress(url, progress_tx).await
    }

    /// Fetch and parse the document at `url`, streaming byte-count updates to `progress`.
    ///
    /// Behaves identically to [`load`](Self::load) but reports the running total of bytes
    /// received through `progress` after each chunk so callers can display live progress
    /// without depending on network internals.
    pub async fn load_with_progress(
        &mut self,
        url: BrowserUrl,
        progress: tokio::sync::watch::Sender<usize>,
    ) -> Result<(), CoreError> {
        let fetched = browser_network::fetch_with_progress(&url, progress).await?;
        let body_byte_count = fetched.body_bytes().len();
        let wire_byte_count = fetched.wire_byte_count();
        let byte_count = if wire_byte_count > 0 {
            wire_byte_count
        } else {
            body_byte_count
        };
        let document = browser_html::parse_html_with_base(
            fetched.body_bytes(),
            fetched.charset(),
            Some(fetched.final_url().as_str()),
        )?;
        let title = document.title().cloned();
        // Push the current page to history before replacing it, so Backspace can restore
        // it without a second network round-trip.
        if let Some(previous) = self.current_page.take() {
            if self.history_stack.len() >= MAX_HISTORY_DEPTH {
                self.history_stack.remove(0);
            }
            self.history_stack.push(previous);
        }
        self.current_page = Some(CurrentPage::new(
            fetched.final_url().clone(),
            document,
            title,
            byte_count,
        ));
        Ok(())
    }

    /// Returns `true` when there is at least one page in the history stack.
    pub fn can_go_back(&self) -> bool {
        !self.history_stack.is_empty()
    }

    /// Restores the most recently visited page from the history stack.
    ///
    /// Returns `true` if a page was restored, `false` when the stack was empty. The
    /// restored page becomes the current page without a network call.
    pub fn go_back(&mut self) -> bool {
        match self.history_stack.pop() {
            Some(page) => {
                self.current_page = Some(page);
                true
            }
            None => false,
        }
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

    /// The parsed document for the current page, or `None` when no page is loaded.
    pub fn current_document(&self) -> Option<&Document> {
        self.current_page.as_ref().map(|page| page.document())
    }

    /// Whether a page is currently loaded.
    pub fn has_page(&self) -> bool {
        self.current_page.is_some()
    }

    /// The byte count of the current page's raw response body, or zero when no page is loaded.
    pub fn page_byte_count(&self) -> usize {
        self.current_page
            .as_ref()
            .map_or(0, CurrentPage::byte_count)
    }

    /// How many `<script>` elements the current page suppressed, or zero when no page is
    /// loaded.
    pub fn script_count(&self) -> usize {
        match &self.current_page {
            Some(page) => page.document().script_count(),
            None => 0,
        }
    }

    /// Closes the tab with the given identifier.
    ///
    /// Not implemented in this milestone; returns [`CoreError::TabNotFound`].
    pub fn close_tab(&mut self, _tab: TabId) -> Result<(), CoreError> {
        Err(CoreError::TabNotFound)
    }
}
