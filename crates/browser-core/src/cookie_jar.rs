// @file crates/browser-core/src/cookie_jar.rs
// @description In-memory session cookie jar keyed by registrable domain, with a redacted Debug.
// @layer core
// @created meerita <meerita@icloud.com>

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::time::SystemTime;

use browser_network::BrowserUrl;
use browser_privacy::{registrable_domain, CookieValue};

/// One accepted cookie held in the session jar.
///
/// The value is a [`CookieValue`], so it can never be printed by a derived `Debug`; the
/// jar's own `Debug` prints only counts. Every field is read: `name` and `value` build the
/// outgoing header, `path` forms the per-domain map key, `secure` gates sending over an
/// insecure request, and `expires_at` drives expiry removal.
pub(crate) struct StoredCookie {
    name: String,
    value: CookieValue,
    path: String,
    secure: bool,
    expires_at: Option<SystemTime>,
}

impl StoredCookie {
    pub(crate) fn new(
        name: String,
        value: CookieValue,
        path: String,
        secure: bool,
        expires_at: Option<SystemTime>,
    ) -> Self {
        Self {
            name,
            value,
            path,
            secure,
            expires_at,
        }
    }

    /// Whether this cookie may be sent on a request with the given transport security.
    ///
    /// A `Secure` cookie is withheld from an insecure request; a non-secure cookie is sent
    /// regardless. First-party matching by registrable domain happens in the jar; this is
    /// only the per-cookie secure gate.
    fn can_send(&self, request_is_secure: bool) -> bool {
        !self.secure || request_is_secure
    }

    /// Whether this cookie is still live at `now`.
    ///
    /// A cookie with no expiry instant is a session cookie: it lives until the process ends
    /// and the jar is dropped, so it is always live within the run.
    fn is_unexpired(&self, now: SystemTime) -> bool {
        self.expires_at.is_none_or(|instant| instant > now)
    }

    /// The `name=value` pair for the outgoing `Cookie` header.
    ///
    /// This is the one place a cookie value is revealed: the jar sends it back to the
    /// origin that set it. The value never reaches a log, an error, or the terminal.
    fn header_pair(&self) -> String {
        format!("{}={}", self.name, self.value.reveal())
    }
}

/// The session cookie jar: accepted cookies held in memory for the life of the process.
///
/// Cookies are keyed first by registrable domain, then by `(name, path)`, so two cookies
/// with the same name but different paths coexist and a later `Set-Cookie` for the same
/// name and path replaces the earlier one. The jar holds no persistence: it is dropped
/// when the process ends, which is the only lifetime a cookie value has this milestone.
#[derive(Default)]
pub(crate) struct CookieJar {
    by_domain: HashMap<String, BTreeMap<(String, String), StoredCookie>>,
}

impl CookieJar {
    /// Stores `cookie` under the registrable domain of `origin`.
    ///
    /// A host with no registrable domain is dropped rather than stored under a meaningless
    /// key, so a cookie that could never be matched back on a later request is never kept.
    pub(crate) fn store(&mut self, origin: &BrowserUrl, cookie: StoredCookie) {
        let Some(host) = origin.host_str() else {
            return;
        };
        let Some(domain_key) = registrable_domain(host) else {
            return;
        };
        let map_key = (cookie.name.clone(), cookie.path.clone());
        self.by_domain
            .entry(domain_key)
            .or_default()
            .insert(map_key, cookie);
    }

    /// Builds the outgoing `Cookie` header for `request_url`, or `None` when nothing matches.
    ///
    /// Expired cookies are removed first. Only cookies whose registrable domain matches the
    /// request's registrable domain are considered (first-party send only this milestone),
    /// and a `Secure` cookie is withheld from an insecure request. The pairs are joined in
    /// the stable `(name, path)` order the inner map imposes.
    pub(crate) fn cookie_header_for(&mut self, request_url: &BrowserUrl) -> Option<String> {
        self.remove_expired(SystemTime::now());
        let host = request_url.host_str()?;
        let domain_key = registrable_domain(host)?;
        let request_is_secure = request_url.scheme() == "https";
        let bucket = self.by_domain.get(&domain_key)?;
        let header = bucket
            .values()
            .filter(|cookie| cookie.can_send(request_is_secure))
            .map(StoredCookie::header_pair)
            .collect::<Vec<_>>()
            .join("; ");
        if header.is_empty() {
            return None;
        }
        Some(header)
    }

    /// Drops every cookie whose expiry instant has passed, then any domain left empty.
    fn remove_expired(&mut self, now: SystemTime) {
        for bucket in self.by_domain.values_mut() {
            bucket.retain(|_, cookie| cookie.is_unexpired(now));
        }
        self.by_domain.retain(|_, bucket| !bucket.is_empty());
    }

    /// Removes every cookie from the jar.
    pub(crate) fn clear(&mut self) {
        self.by_domain.clear();
    }

    /// Removes every cookie held under `registrable_domain`.
    ///
    /// `registrable_domain` is the outer key: the caller resolves a site's registrable
    /// domain before calling, so a reject exception drops exactly that site's cookies.
    pub(crate) fn clear_domain(&mut self, registrable_domain: &str) {
        self.by_domain.remove(registrable_domain);
    }

    /// Keeps only the domains for which `keep` returns true, dropping the rest.
    ///
    /// The key passed to `keep` is the registrable domain. This prunes the jar when the
    /// global first-party default tightens to reject: cookies for domains no exception
    /// permits are dropped so the tighter policy takes hold at once.
    pub(crate) fn retain_domains(&mut self, keep: impl Fn(&str) -> bool) {
        self.by_domain.retain(|domain, _| keep(domain));
    }
}

impl fmt::Debug for CookieJar {
    /// Hand-written so a cookie value can never leak through a derived `Debug`. It prints
    /// how many domains and cookies the jar holds, never a name or a value.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cookie_count: usize = self.by_domain.values().map(BTreeMap::len).sum();
        formatter
            .debug_struct("CookieJar")
            .field("domain_count", &self.by_domain.len())
            .field("cookie_count", &cookie_count)
            .finish()
    }
}

#[cfg(test)]
#[path = "cookie_jar_tests.rs"]
mod tests;
