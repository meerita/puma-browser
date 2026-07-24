// @file crates/browser-network/src/fetched_document.rs
// @description Value object holding a fetched response: final URL, content type, charset, raw body.
// @layer network
// @created meerita <meerita@icloud.com>

use crate::browser_url::BrowserUrl;

/// The result of a successful fetch.
///
/// Holds the URL the request finally resolved to after any redirects, the response
/// content type, the charset declared in that content type (if any), and the raw,
/// undecoded response body. The network layer does not decode the body: decoding
/// belongs at the parse boundary, where a `<meta charset>` inside the markup can be
/// honored. The charset is surfaced separately so the parser can use it as a hint.
pub struct FetchedDocument {
    final_url: BrowserUrl,
    content_type: String,
    charset: Option<String>,
    body: Vec<u8>,
}

impl FetchedDocument {
    pub(crate) fn new(
        final_url: BrowserUrl,
        content_type: String,
        charset: Option<String>,
        body: Vec<u8>,
    ) -> Self {
        Self {
            final_url,
            content_type,
            charset,
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

    /// The charset declared in the response `Content-Type`, if any.
    pub fn charset(&self) -> Option<&str> {
        self.charset.as_deref()
    }

    /// The raw, undecoded response body bytes.
    pub fn body_bytes(&self) -> &[u8] {
        &self.body
    }
}
