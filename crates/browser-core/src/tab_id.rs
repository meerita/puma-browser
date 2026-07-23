// @file crates/browser-core/src/tab_id.rs
// @description Newtype identifier for a browser tab.
// @layer core
// @created meerita <meerita@icloud.com>

/// Identifies a single open tab.
///
/// A newtype over `u32` so a tab identifier can never be confused with another
/// numeric value at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(u32);

impl TabId {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn value(self) -> u32 {
        self.0
    }
}
