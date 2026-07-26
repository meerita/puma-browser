//! @file crates/browser-privacy/src/lib.rs
//! @description Privacy crate root: cookie policy and the privacy error taxonomy.
//! @layer privacy
//! @created meerita <meerita@icloud.com>

mod cookie_policy;
mod error;
mod tracking_redirect;

pub use cookie_policy::CookiePolicy;
pub use error::PrivacyError;
pub use tracking_redirect::unwrap_tracking_redirect;
