// @file crates/browser-privacy/src/parsed_cookie.rs
// @description RFC 6265 Set-Cookie parsing into a secret-safe ParsedCookie via the cookie crate.
// @layer privacy
// @created meerita <meerita@icloud.com>

use crate::cookie_value::CookieValue;
use crate::error::PrivacyError;

/// The `SameSite` attribute of a cookie, with an explicit `Unset` for the missing case.
///
/// A domain enum, deliberately not `serde`-derived; adapters own any wire or storage shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    Strict,
    Lax,
    None,
    Unset,
}

/// A parsed `Set-Cookie` line with its attributes, holding the value in a redacted wrapper.
///
/// Fields are private; access is through getters. The value is a `CookieValue`, so a
/// derived `Debug` on this type still cannot print the secret.
#[derive(Debug, Clone)]
pub struct ParsedCookie {
    name: String,
    value: CookieValue,
    domain: Option<String>,
    path: Option<String>,
    has_expiry: bool,
    same_site: SameSite,
    secure: bool,
    http_only: bool,
}

impl ParsedCookie {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> &CookieValue {
        &self.value
    }

    pub fn domain(&self) -> Option<&str> {
        self.domain.as_deref()
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
    }

    /// True when the line carried an `Expires` or `Max-Age` attribute (a persistent
    /// cookie). A cookie with neither is a session cookie.
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

/// Parses a single raw `Set-Cookie` header value into a `ParsedCookie`.
///
/// A line that does not parse as a cookie returns `PrivacyError::CookieMalformed`. The
/// `cookie` crate type never appears in the public signature; its output is mapped into
/// `ParsedCookie` here.
pub fn parse(set_cookie_line: &str) -> Result<ParsedCookie, PrivacyError> {
    let raw_cookie =
        cookie::Cookie::parse(set_cookie_line).map_err(|_| PrivacyError::CookieMalformed)?;

    let has_expiry = matches!(raw_cookie.expires(), Some(cookie::Expiration::DateTime(_)))
        || raw_cookie.max_age().is_some();

    Ok(ParsedCookie {
        name: raw_cookie.name().to_owned(),
        value: CookieValue::new(raw_cookie.value()),
        domain: raw_cookie.domain().map(str::to_owned),
        path: raw_cookie.path().map(str::to_owned),
        has_expiry,
        same_site: map_same_site(raw_cookie.same_site()),
        secure: raw_cookie.secure().unwrap_or(false),
        http_only: raw_cookie.http_only().unwrap_or(false),
    })
}

/// Maps the `cookie` crate's `SameSite` (with absence as `Option::None`) into the crate's
/// own `SameSite` enum, keeping the crate type out of the public surface.
fn map_same_site(raw_same_site: Option<cookie::SameSite>) -> SameSite {
    match raw_same_site {
        Some(cookie::SameSite::Strict) => SameSite::Strict,
        Some(cookie::SameSite::Lax) => SameSite::Lax,
        Some(cookie::SameSite::None) => SameSite::None,
        None => SameSite::Unset,
    }
}
