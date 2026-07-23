//! @file crates/browser-html/src/lib.rs
//! @description HTML crate root: semantic document model, node ids, and the HTML error taxonomy.
//! @layer html
//! @created meerita <meerita@icloud.com>

mod document;
mod error;
mod node_id;
mod semantic_node;

pub use document::{Document, DocumentTitle};
pub use error::HtmlError;
pub use node_id::NodeId;
pub use semantic_node::SemanticNode;
