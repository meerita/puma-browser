// @file crates/browser-network/src/fetch.rs
// @description HTTP/HTTPS fetch: timeout, safe capped redirects, bounded raw body, charset hint.
// @layer network
// @created meerita <meerita@icloud.com>

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{CONTENT_TYPE, LOCATION};
use url::Url;

use crate::browser_url::BrowserUrl;
use crate::error::NetworkError;
use crate::fetched_document::FetchedDocument;

/// Largest response body accepted, in bytes. Enforced while streaming, not after
/// buffering, so an oversized body is abandoned as soon as the cap is crossed.
const MAX_RESPONSE_BYTES: u64 = 10 * 1024 * 1024;

/// Largest number of redirects followed before the request is abandoned.
const MAX_REDIRECT_COUNT: usize = 10;

/// How long a single request may take before it is abandoned.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Fetch `url` over HTTP or HTTPS and return the raw response.
///
/// Redirects are followed manually so each hop can be validated: the target scheme
/// must stay `http` or `https`, an HTTPS to HTTP downgrade is refused, and the chain
/// is capped at [`MAX_REDIRECT_COUNT`]. The body is bounded at [`MAX_RESPONSE_BYTES`]
/// while streaming and returned undecoded; decoding to Unicode happens at the parse
/// boundary, where a `<meta charset>` can be honored.
pub async fn fetch(url: &BrowserUrl) -> Result<FetchedDocument, NetworkError> {
    let client = build_client()?;
    let mut current = Url::parse(url.as_str()).map_err(|_| NetworkError::InvalidUrl)?;
    let mut redirect_count: usize = 0;
    loop {
        let response = client
            .get(current.clone())
            .send()
            .await
            .map_err(map_send_error)?;
        if !response.status().is_redirection() {
            return collect_document(response).await;
        }
        let location = redirect_location(&response);
        let Some(location) = location else {
            return collect_document(response).await;
        };
        let next = resolve_redirect(&current, &location)?;
        redirect_count += 1;
        if redirect_count > MAX_REDIRECT_COUNT {
            return Err(NetworkError::TooManyRedirects);
        }
        current = next;
    }
}

fn build_client() -> Result<reqwest::Client, NetworkError> {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| NetworkError::RequestFailed)
}

fn map_send_error(error: reqwest::Error) -> NetworkError {
    if error.is_timeout() {
        return NetworkError::Timeout;
    }
    NetworkError::RequestFailed
}

/// The `Location` header value as an owned string, if present and valid text.
fn redirect_location(response: &reqwest::Response) -> Option<String> {
    response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string())
}

/// Resolve a redirect target against the current URL and enforce redirect safety.
///
/// The target may be relative, so it is joined onto the current URL first. The result
/// must use `http` or `https`, and an HTTPS origin may not be downgraded to HTTP.
fn resolve_redirect(current: &Url, location: &str) -> Result<Url, NetworkError> {
    let next = current
        .join(location)
        .map_err(|_| NetworkError::RequestFailed)?;
    if !scheme_is_http(next.scheme()) {
        return Err(NetworkError::RequestFailed);
    }
    if redirect_is_downgrade(current, &next) {
        return Err(NetworkError::RequestFailed);
    }
    Ok(next)
}

fn scheme_is_http(scheme: &str) -> bool {
    scheme == "http" || scheme == "https"
}

fn redirect_is_downgrade(current: &Url, next: &Url) -> bool {
    current.scheme() == "https" && next.scheme() == "http"
}

async fn collect_document(response: reqwest::Response) -> Result<FetchedDocument, NetworkError> {
    let final_url = BrowserUrl::parse(response.url().as_str())?;
    let content_type = content_type_of(&response);
    let charset = charset_from_content_type(&content_type);
    if let Some(length) = response.content_length() {
        if length > MAX_RESPONSE_BYTES {
            return Err(NetworkError::ResponseTooLarge);
        }
    }
    let body = read_bounded_body(response).await?;
    Ok(FetchedDocument::new(final_url, content_type, charset, body))
}

fn content_type_of(response: &reqwest::Response) -> String {
    response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string()
}

/// Extract the charset parameter from a `Content-Type` header value, if present.
///
/// The value is read after a literal `charset=` up to the next parameter delimiter, with
/// surrounding quotes removed. The parser resolves and validates the label; the network
/// layer only surfaces it.
fn charset_from_content_type(content_type: &str) -> Option<String> {
    let lowered = content_type.to_ascii_lowercase();
    let index = lowered.find("charset")?;
    let after = content_type[index + "charset".len()..].trim_start();
    let value = after.strip_prefix('=')?.trim();
    let label = value
        .trim_matches('"')
        .split(';')
        .next()
        .unwrap_or("")
        .trim();
    if label.is_empty() {
        return None;
    }
    Some(label.to_string())
}

/// Stream the body into memory, stopping as soon as the size cap is crossed, and return
/// the raw bytes undecoded.
async fn read_bounded_body(response: reqwest::Response) -> Result<Vec<u8>, NetworkError> {
    let mut collected: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| NetworkError::RequestFailed)?;
        collected.extend_from_slice(&chunk);
        if collected.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(NetworkError::ResponseTooLarge);
        }
    }
    Ok(collected)
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
