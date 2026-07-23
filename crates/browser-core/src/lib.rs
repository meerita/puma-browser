//! @file crates/browser-core/src/lib.rs
//! @description Core crate root: tab and navigation domain types, error taxonomy, controller.
//! @layer core
//! @created meerita <meerita@icloud.com>

mod error;
mod ids;
mod tab_id;
mod tab_state;

pub use error::CoreError;
pub use ids::{BookmarkId, HistoryEntryId};
pub use tab_id::TabId;
pub use tab_state::TabState;

/// Orchestrates navigation across tabs, history, and the rendering pipeline.
///
/// This is the application core the output adapters build on. It is a placeholder in
/// this milestone: the navigation loop, tab operations, forms, and downloads are not
/// implemented yet, so methods report a typed error rather than acting.
#[derive(Debug, Default)]
pub struct NavigationController;

impl NavigationController {
    pub fn new() -> Self {
        Self
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
