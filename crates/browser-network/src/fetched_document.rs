// @file crates/browser-network/src/fetched_document.rs
// @description Value object holding a fetched response: final URL, content type, decoded body.
// @layer network
// @created meerita <meerita@icloud.com>

use crate::browser_url::BrowserUrl;

/// The result of a successful fetch.
///
/// Holds the URL the request finally resolved to after any redirects, the response
/// content type, and the body decoded to a `String`. The body is decoded with a lossy
/// UTF-8 fallback, so it is always valid UTF-8 and never carries raw remote bytes.
pub struct FetchedDocument {
    final_url: BrowserUrl,
    content_type: String,
    body: String,
}

impl FetchedDocument {
    pub(crate) fn new(final_url: BrowserUrl, content_type: String, body: String) -> Self {
        Self {
            final_url,
            content_type,
            body,
        }
    }

    /// The URL the request resolved to after following any redirects.
    pub fn final_url(&self) -> &BrowserUrl {
        &self.final_url
    }

    /// The value of the response `Content-Type` header, or an empty string if absent.
    pub fn content_type(&self) -> &str {
        &self.content_type
    }

    /// The response body decoded to UTF-8 with a lossy fallback.
    pub fn body(&self) -> &str {
        &self.body
    }
}
