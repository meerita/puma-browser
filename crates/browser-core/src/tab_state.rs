// @file crates/browser-core/src/tab_state.rs
// @description Finite lifecycle state of a browser tab.
// @layer core
// @created meerita <meerita@icloud.com>

/// The lifecycle state of a tab.
///
/// A domain enum, deliberately not `serde`-derived; adapters own any wire or storage
/// representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabState {
    Loading,
    Loaded,
    Error,
    Blank,
}
