//! @file crates/browser-privacy/src/lib.rs
//! @description Privacy crate root: cookie policy, classification, parsing, and the decision engine.
//! @layer privacy
//! @created meerita <meerita@icloud.com>

mod cookie_decision;
mod cookie_policy;
mod cookie_value;
mod error;
mod parsed_cookie;
mod site_identity;
mod tracking_redirect;

pub use cookie_decision::{decide, CookieContext, CookieDecision, RejectionReason};
pub use cookie_policy::CookiePolicy;
pub use cookie_value::CookieValue;
pub use error::PrivacyError;
pub use parsed_cookie::{parse, ParsedCookie, SameSite};
pub use site_identity::{classify, registrable_domain, CookieScope};
pub use tracking_redirect::unwrap_tracking_redirect;
