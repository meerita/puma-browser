// @file crates/browser-html/src/node_id.rs
// @description Newtype identifier for nodes in the semantic document tree.
// @layer html
// @created meerita <meerita@icloud.com>

/// Identifies a single node within a [`crate::Document`]'s semantic tree.
///
/// A newtype over `u32` so a node identifier can never be confused with another
/// numeric value at a call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

impl NodeId {
    pub fn new(value: u32) -> Self {
        Self(value)
    }

    pub fn value(self) -> u32 {
        self.0
    }
}
