// @file crates/browser-core/src/cookie_record.rs
// @description Secret-free inspection record for one cookie decision; never holds the value.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_privacy::{ParsedCookie, RejectionReason, SameSite};

/// The record of one cookie decision, held for inspection through `/cookies`.
///
/// It carries the cookie's origin, name, and safe attributes plus whether it was accepted
/// and, if not, why. It deliberately never holds the cookie value, so it needs no redaction
/// and can never leak a secret through `Debug`, a log line, or an MCP response.
#[derive(Debug, Clone)]
pub struct CookieRecord {
    origin: String,
    name: String,
    first_party: bool,
    accepted: bool,
    reason: Option<RejectionReason>,
    has_expiry: bool,
    same_site: SameSite,
    secure: bool,
    http_only: bool,
}

impl CookieRecord {
    /// Records an accepted cookie, copying only its safe attributes from `cookie`.
    pub(crate) fn new_accepted(origin: String, cookie: &ParsedCookie, first_party: bool) -> Self {
        Self {
            origin,
            name: cookie.name().to_string(),
            first_party,
            accepted: true,
            reason: None,
            has_expiry: cookie.has_expiry(),
            same_site: cookie.same_site(),
            secure: cookie.secure(),
            http_only: cookie.http_only(),
        }
    }

    /// Records a rejected cookie with its reason, copying only its safe attributes.
    pub(crate) fn rejected(
        origin: String,
        cookie: &ParsedCookie,
        first_party: bool,
        reason: RejectionReason,
    ) -> Self {
        Self {
            origin,
            name: cookie.name().to_string(),
            first_party,
            accepted: false,
            reason: Some(reason),
            has_expiry: cookie.has_expiry(),
            same_site: cookie.same_site(),
            secure: cookie.secure(),
            http_only: cookie.http_only(),
        }
    }

    /// Records a cookie that could not be parsed, so no attributes are known.
    ///
    /// A malformed line yields no name or attributes; only the origin and the reason are
    /// recorded, so the user still sees that a cookie was offered and refused.
    pub(crate) fn malformed(origin: String) -> Self {
        Self {
            origin,
            name: String::new(),
            first_party: false,
            accepted: false,
            reason: Some(RejectionReason::Malformed),
            has_expiry: false,
            same_site: SameSite::Unset,
            secure: false,
            http_only: false,
        }
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn first_party(&self) -> bool {
        self.first_party
    }

    pub fn accepted(&self) -> bool {
        self.accepted
    }

    pub fn reason(&self) -> Option<RejectionReason> {
        self.reason
    }

    pub fn has_expiry(&self) -> bool {
        self.has_expiry
    }

    pub fn same_site(&self) -> SameSite {
        self.same_site
    }

    pub fn secure(&self) -> bool {
        self.secure
    }

    pub fn http_only(&self) -> bool {
        self.http_only
    }
}
