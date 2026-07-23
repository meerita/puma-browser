// @file crates/browser-network/src/browser_url.rs
// @description Validated, normalized URL value object; scheme-checked before any network use.
// @layer network
// @created meerita <meerita@icloud.com>

use std::fmt;

use url::Url;

use crate::error::NetworkError;

/// The schemes the browser is allowed to request.
const ALLOWED_SCHEMES: [&str; 3] = ["http", "https", "file"];

/// The scheme assumed when the user types a bare host with no scheme.
const DEFAULT_SCHEME_PREFIX: &str = "https://";

/// A parsed, normalized URL whose scheme is validated at construction.
///
/// The scheme is checked in `parse` before the value can be handed to any network
/// call. `Debug` and `Display` strip userinfo so embedded credentials never reach
/// logs or the terminal.
pub struct BrowserUrl(Url);

impl BrowserUrl {
    /// Parse and validate `input`.
    ///
    /// Input with no scheme (for example `minid.net` or `www.minid.net`) is assumed
    /// to be `https`, matching what a user typing a bare host expects. An explicit
    /// scheme is always kept as written; a bare host is never downgraded to `http`.
    /// Malformed input returns [`NetworkError::InvalidUrl`], and any scheme outside
    /// http/https/file returns [`NetworkError::UnsupportedScheme`].
    pub fn parse(input: &str) -> Result<Self, NetworkError> {
        let parsed = parse_with_default_scheme(input.trim())?;
        let scheme = parsed.scheme();
        if !ALLOWED_SCHEMES.contains(&scheme) {
            return Err(NetworkError::UnsupportedScheme {
                scheme: scheme.to_string(),
            });
        }
        Ok(Self(parsed))
    }

    pub fn scheme(&self) -> &str {
        self.0.scheme()
    }

    pub fn host_str(&self) -> Option<&str> {
        self.0.host_str()
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// A copy of the URL string with any username and password removed.
    fn credential_safe_string(&self) -> String {
        let mut sanitized = self.0.clone();
        // Clearing userinfo can fail for schemes that cannot carry it; in that case
        // there is nothing to strip, so ignoring the result is safe.
        let _ = sanitized.set_username("");
        let _ = sanitized.set_password(None);
        sanitized.to_string()
    }
}

/// Parse `input`, assuming `https` when it carries no scheme of its own.
///
/// `url::Url::parse` reports schemeless input as `RelativeUrlWithoutBase`. Only that
/// case is retried with an `https://` prefix, so an explicit scheme is never
/// rewritten and a bare host is never silently downgraded to `http`.
fn parse_with_default_scheme(input: &str) -> Result<Url, NetworkError> {
    match Url::parse(input) {
        Ok(parsed) => Ok(parsed),
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            let with_default_scheme = format!("{DEFAULT_SCHEME_PREFIX}{input}");
            Url::parse(&with_default_scheme).map_err(|_| NetworkError::InvalidUrl)
        }
        Err(_) => Err(NetworkError::InvalidUrl),
    }
}

impl fmt::Display for BrowserUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.credential_safe_string())
    }
}

impl fmt::Debug for BrowserUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("BrowserUrl")
            .field(&self.credential_safe_string())
            .finish()
    }
}
