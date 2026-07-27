// @file crates/browser-core/src/navigation_source.rs
// @description How a navigation was initiated, used to weight typed visits in ranking.
// @layer core
// @created meerita <meerita@icloud.com>

/// Where a navigation request came from.
///
/// This is a domain enum and is deliberately not `serde`-derived: no adapter serializes
/// it, and keeping it free of a wire representation means a storage or protocol change
/// never reaches this type. Recording treats a URL the user typed as a stronger signal of
/// intent to return than one arrived at by following a link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavigationSource {
    /// The user typed or pasted the address into the address bar.
    AddressBar,
    /// The user followed a link on the current page.
    Link,
    /// The current page was reloaded.
    Reload,
    /// An MCP client opened the address.
    Mcp,
}

impl NavigationSource {
    /// Whether this navigation counts as a typed visit for ranking.
    ///
    /// Address-bar and MCP navigations name a destination directly, so both weigh as
    /// typed. Following a link or reloading does not.
    pub fn was_typed(self) -> bool {
        matches!(self, NavigationSource::AddressBar | NavigationSource::Mcp)
    }
}
