// @file crates/browser-core/src/search_engine.rs
// @description Web search engine value type that builds a results URL from a query.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_network::{BrowserUrl, NetworkError};

use crate::error::CoreError;

/// The schemes a search engine base URL may use. A search endpoint is fetched over the
/// network, so `file://` (allowed for local navigation) and every other scheme are
/// rejected: an engine that read the local filesystem would be a privacy hole.
const SEARCH_ENGINE_SCHEMES: [&str; 2] = ["http", "https"];

/// The web search engine a `/search` query is sent to.
///
/// Holds the results-page base URL and the name of the query parameter. The default is the
/// no-JavaScript DuckDuckGo lite endpoint, which renders as plain text in a scriptless
/// browser. The values are owned so the engine can be configured at runtime; an invalid
/// base URL is rejected by [`new`](Self::new) and can never be stored.
#[derive(Debug)]
pub struct SearchEngine {
    base_url: String,
    query_parameter: String,
}

impl SearchEngine {
    /// Build a configured engine, rejecting a base URL that fails scheme validation.
    ///
    /// The base URL is built through [`BrowserUrl::with_query_parameter`] with a probe query,
    /// so it runs the same parse and structure checks as any request, and its scheme is then
    /// restricted to http(s): a `file://` or otherwise non-http(s) or malformed base URL
    /// returns [`CoreError`] and no invalid engine is stored. The probe URL is discarded;
    /// only its successful construction and scheme are the check.
    pub fn new(base_url: String, query_parameter: String) -> Result<Self, CoreError> {
        let probe = BrowserUrl::with_query_parameter(&base_url, &query_parameter, "probe")?;
        if !SEARCH_ENGINE_SCHEMES.contains(&probe.scheme()) {
            return Err(CoreError::Network(NetworkError::UnsupportedScheme {
                scheme: probe.scheme().to_string(),
            }));
        }
        Ok(Self {
            base_url,
            query_parameter,
        })
    }

    /// Build the results URL for `query`, percent-encoded and scheme-validated.
    ///
    /// The query is encoded through [`BrowserUrl::with_query_parameter`], so it runs the
    /// same scheme check as any other request and cannot alter the URL structure. A
    /// network failure to build the URL is mapped into [`CoreError`] by `?`.
    pub fn result_url(&self, query: &str) -> Result<BrowserUrl, CoreError> {
        Ok(BrowserUrl::with_query_parameter(
            &self.base_url,
            &self.query_parameter,
            query,
        )?)
    }

    /// The results-page base URL, for a panel to display the configured engine.
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// The query parameter name, for a panel to display the configured engine.
    pub fn query_parameter(&self) -> &str {
        &self.query_parameter
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self {
            base_url: "https://lite.duckduckgo.com/lite/".to_string(),
            query_parameter: "q".to_string(),
        }
    }
}
