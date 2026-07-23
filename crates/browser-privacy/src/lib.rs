//! @file crates/browser-privacy/src/lib.rs
//! @description Privacy crate root: cookie policy and the privacy error taxonomy.
//! @layer privacy
//! @created meerita <meerita@icloud.com>

mod cookie_policy;
mod error;

pub use cookie_policy::CookiePolicy;
pub use error::PrivacyError;
