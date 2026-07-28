// @file crates/browser-core/src/cookie_settings.rs
// @description Default cookie policy pair for first- and third-party scopes, and policy word parsing.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_privacy::CookiePolicy;

/// The default cookie policy for each scope, applied when no per-site exception overrides it.
///
/// Both scopes default to [`CookiePolicy::Reject`]: the browser accepts no cookie until the
/// user opts a site in. The two fields are public because this is a plain settings pair the
/// composition root fills from configuration, not a type with an invariant to protect.
#[derive(Debug, Clone, Copy)]
pub struct CookiePolicyPair {
    pub first_party: CookiePolicy,
    pub third_party: CookiePolicy,
}

impl Default for CookiePolicyPair {
    /// Reject in both scopes. This default is a privacy invariant, not a convenience.
    fn default() -> Self {
        Self {
            first_party: CookiePolicy::Reject,
            third_party: CookiePolicy::Reject,
        }
    }
}

/// Maps a policy word to a [`CookiePolicy`], defaulting to [`CookiePolicy::Reject`].
///
/// An unknown or misspelled word resolves to `Reject` so a bad configuration value or a
/// mistyped command can only ever be more private, never less.
pub fn parse_policy(word: &str) -> CookiePolicy {
    match word.trim().to_ascii_lowercase().as_str() {
        "allow" => CookiePolicy::Allow,
        "session" => CookiePolicy::Session,
        "ask" => CookiePolicy::Ask,
        _ => CookiePolicy::Reject,
    }
}
