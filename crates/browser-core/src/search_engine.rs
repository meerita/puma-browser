// @file crates/browser-core/src/search_engine.rs
// @description Web search engine value type that builds a results URL from a query.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_network::BrowserUrl;

use crate::error::CoreError;

/// The web search engine a `/search` query is sent to.
///
/// Holds the results-page base URL and the name of the query parameter. The default is the
/// no-JavaScript DuckDuckGo lite endpoint, which renders as plain text in a scriptless
/// browser. The endpoint is fixed here; there is no user-configurable engine yet.
pub struct SearchEngine {
    base_url: &'static str,
    query_parameter: &'static str,
}

impl SearchEngine {
    /// Build the results URL for `query`, percent-encoded and scheme-validated.
    ///
    /// The query is encoded through [`BrowserUrl::with_query_parameter`], so it runs the
    /// same scheme check as any other request and cannot alter the URL structure. A
    /// network failure to build the URL is mapped into [`CoreError`] by `?`.
    pub fn result_url(&self, query: &str) -> Result<BrowserUrl, CoreError> {
        Ok(BrowserUrl::with_query_parameter(
            self.base_url,
            self.query_parameter,
            query,
        )?)
    }
}

impl Default for SearchEngine {
    fn default() -> Self {
        Self {
            base_url: "https://lite.duckduckgo.com/lite/",
            query_parameter: "q",
        }
    }
}
