//! @file crates/browser-html/src/lib.rs
//! @description HTML crate root: semantic document model, node ids, and the HTML error taxonomy.
//! @layer html
//! @created meerita <meerita@icloud.com>

mod document;
mod encoding;
mod error;
mod inline_run;
mod input_kind;
mod landmark_role;
mod node_id;
mod parser;
mod sanitize;
mod semantic_node;

pub use document::{Document, DocumentTitle};
pub use encoding::{DetectedEncoding, Encoding};
pub use error::HtmlError;
pub use inline_run::{InlineEmphasis, InlineRun};
pub use input_kind::InputKind;
pub use landmark_role::LandmarkRole;
pub use node_id::NodeId;
pub use parser::parse_html;
pub use semantic_node::SemanticNode;
