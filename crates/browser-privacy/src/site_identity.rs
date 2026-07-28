// @file crates/browser-privacy/src/site_identity.rs
// @description Registrable-domain and first/third-party classification via the public suffix list.
// @layer privacy
// @created meerita <meerita@icloud.com>

/// Whether a cookie's origin is the same registrable site as the page the user navigated
/// to (`FirstParty`) or a different one (`ThirdParty`).
///
/// A domain enum, deliberately not `serde`-derived; adapters own any wire or storage shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieScope {
    FirstParty,
    ThirdParty,
}

/// The registrable domain (eTLD+1) of `host`, or `None` when `host` is itself a public
/// suffix or has no registrable label above the suffix.
///
/// `www.example.com` and `analytics.example.com` both resolve to `example.com`;
/// `a.example.co.uk` resolves to `example.co.uk`; a bare public suffix such as `co.uk`
/// resolves to `None`.
pub fn registrable_domain(host: &str) -> Option<String> {
    psl::domain_str(host).map(str::to_owned)
}

/// Classifies a cookie's origin host against the top-level navigation host.
///
/// Returns `None` when `cookie_host` is a public suffix, so the caller can reject a cookie
/// that tries to set itself on a registrable suffix. Otherwise it compares registrable
/// domains: a match is `FirstParty`, anything else (including a top-level host with no
/// registrable domain) is `ThirdParty`. This is a registrable-domain comparison, not a
/// naive hostname match.
pub fn classify(cookie_host: &str, top_level_host: &str) -> Option<CookieScope> {
    let cookie_domain = registrable_domain(cookie_host)?;
    let scope = match registrable_domain(top_level_host) {
        Some(top_level_domain) if top_level_domain == cookie_domain => CookieScope::FirstParty,
        _ => CookieScope::ThirdParty,
    };
    Some(scope)
}
