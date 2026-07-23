//! @file crates/browser-network/src/lib.rs
//! @description Network crate root: validated URLs, cache modes, and the network error taxonomy.
//! @layer network
//! @created meerita <meerita@icloud.com>

mod browser_url;
mod cache_mode;
mod error;

pub use browser_url::BrowserUrl;
pub use cache_mode::CacheMode;
pub use error::NetworkError;
