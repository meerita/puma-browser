// @file crates/browser-privacy/src/cookie_decision.rs
// @description Resolved-policy decision for a parsed cookie: accept (session-only or not) or reject.
// @layer privacy
// @created meerita <meerita@icloud.com>

use crate::cookie_policy::CookiePolicy;
use crate::parsed_cookie::ParsedCookie;
use crate::site_identity::CookieScope;

/// Why a cookie was rejected.
///
/// `ThirdParty` and `PublicSuffix` are produced by the caller from the classification step
/// (a third-party scope whose resolved policy is `Reject`, and a `None` from `classify`).
/// The remaining reasons are produced by `decide` itself. A domain enum, not `serde`-derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    Policy,
    ThirdParty,
    Ask,
    PublicSuffix,
    SecureOverInsecure,
    Malformed,
}

/// The outcome of applying a resolved policy to a cookie.
///
/// A domain enum, not `serde`-derived; adapters own any wire or storage shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieDecision {
    Accept { session_only: bool },
    Reject(RejectionReason),
}

/// The context a cookie decision needs beyond the cookie itself: its classified scope and
/// whether the request that offered it ran over a secure transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CookieContext {
    pub scope: CookieScope,
    pub request_is_secure: bool,
}

/// Decides accept or reject for a cookie under an already-resolved policy.
///
/// The policy passed in is the effective one for this cookie's scope after any per-site
/// exception has been applied. `Reject` and `Ask` reject outright (`Ask` has no interactive
/// prompt yet, so it rejects with a distinct reason). A `Secure` cookie offered over an
/// insecure request is rejected. Otherwise the cookie is accepted; it is session-only when
/// the policy is `Session` or when the cookie carried no expiry.
pub fn decide(
    policy: CookiePolicy,
    cookie: &ParsedCookie,
    context: &CookieContext,
) -> CookieDecision {
    if matches!(policy, CookiePolicy::Reject) {
        return CookieDecision::Reject(RejectionReason::Policy);
    }
    if matches!(policy, CookiePolicy::Ask) {
        return CookieDecision::Reject(RejectionReason::Ask);
    }

    let secure_cookie_over_insecure_request = cookie.secure() && !context.request_is_secure;
    if secure_cookie_over_insecure_request {
        return CookieDecision::Reject(RejectionReason::SecureOverInsecure);
    }

    let session_only = matches!(policy, CookiePolicy::Session) || !cookie.has_expiry();
    CookieDecision::Accept { session_only }
}
