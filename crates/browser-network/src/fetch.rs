// @file crates/browser-network/src/fetch.rs
// @description Resource acquisition: HTTP/HTTPS fetch and bounded local file:// reads, dispatched on scheme.
// @layer network
// @created meerita <meerita@icloud.com>

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{CONTENT_TYPE, LOCATION};
use tokio::sync::watch;
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

/// Acquire `url` and return the raw document, dispatching on its scheme.
///
/// A `file://` URL is read from disk; any other URL is fetched over HTTP or HTTPS.
/// The public signature is identical for both paths so callers do not distinguish
/// local from remote acquisition.
pub async fn fetch(url: &BrowserUrl) -> Result<FetchedDocument, NetworkError> {
    let (progress_tx, _) = watch::channel(0usize);
    fetch_with_progress(url, progress_tx).await
}

/// Acquire `url` and stream byte-count updates to `progress` as chunks arrive.
///
/// Behaves identically to [`fetch`] but reports the running total of bytes received
/// through `progress` after each chunk. For `file://` URLs, which are read in one
/// shot, the final file size is sent once after the read completes. The send is
/// silently ignored when all receivers have been dropped.
pub async fn fetch_with_progress(
    url: &BrowserUrl,
    progress: watch::Sender<usize>,
) -> Result<FetchedDocument, NetworkError> {
    match url.scheme() {
        "file" => {
            let document = read_local_file(url).await?;
            let _ = progress.send(document.body_bytes().len());
            Ok(document)
        }
        _ => fetch_over_http_with_progress(url, &progress).await,
    }
}

/// Read a local file named by a `file://` URL into a [`FetchedDocument`].
///
/// The read is bounded by [`MAX_RESPONSE_BYTES`] via a metadata check before the body
/// is loaded. A directory is rejected. The content type is guessed from the file
/// extension and no charset is declared, so the parse boundary detects the encoding.
/// Raw `std::io::Error` values are mapped to typed variants and never cross outward.
async fn read_local_file(url: &BrowserUrl) -> Result<FetchedDocument, NetworkError> {
    let Some(path) = url.path_buf() else {
        return Err(NetworkError::InvalidUrl);
    };
    let metadata = tokio::fs::metadata(&path).await.map_err(map_file_error)?;
    if metadata.is_dir() {
        return Err(NetworkError::PathIsDirectory);
    }
    if metadata.len() > MAX_RESPONSE_BYTES {
        return Err(NetworkError::FileTooLarge);
    }
    let body = tokio::fs::read(&path).await.map_err(map_file_error)?;
    let content_type = content_type_for_path(&path);
    Ok(FetchedDocument::new(url.clone(), content_type, None, body))
}

/// Map a filesystem error to a typed variant, keeping raw io detail out of callers.
fn map_file_error(error: std::io::Error) -> NetworkError {
    if error.kind() == std::io::ErrorKind::NotFound {
        return NetworkError::FileNotFound;
    }
    NetworkError::FileReadFailed
}

/// Guess a content type from a file extension: HTML extensions map to `text/html`,
/// everything else to `text/plain`. The parse boundary handles the actual bytes.
fn content_type_for_path(path: &std::path::Path) -> String {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    match extension.as_deref() {
        Some("html") | Some("htm") | Some("xhtml") => "text/html".to_string(),
        _ => "text/plain".to_string(),
    }
}

async fn fetch_over_http_with_progress(
    url: &BrowserUrl,
    progress: &watch::Sender<usize>,
) -> Result<FetchedDocument, NetworkError> {
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
            return collect_document_reporting_progress(response, progress).await;
        }
        let location = redirect_location(&response);
        let Some(location) = location else {
            return collect_document_reporting_progress(response, progress).await;
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

async fn collect_document_reporting_progress(
    response: reqwest::Response,
    progress: &watch::Sender<usize>,
) -> Result<FetchedDocument, NetworkError> {
    let final_url = BrowserUrl::parse(response.url().as_str())?;
    let content_type = content_type_of(&response);
    let charset = charset_from_content_type(&content_type);
    if let Some(length) = response.content_length() {
        if length > MAX_RESPONSE_BYTES {
            return Err(NetworkError::ResponseTooLarge);
        }
    }
    let body = read_bounded_body_reporting_progress(response, progress).await?;
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

async fn read_bounded_body_reporting_progress(
    response: reqwest::Response,
    progress: &watch::Sender<usize>,
) -> Result<Vec<u8>, NetworkError> {
    let mut collected: Vec<u8> = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| NetworkError::RequestFailed)?;
        collected.extend_from_slice(&chunk);
        if collected.len() as u64 > MAX_RESPONSE_BYTES {
            return Err(NetworkError::ResponseTooLarge);
        }
        let _ = progress.send(collected.len());
    }
    Ok(collected)
}

#[cfg(test)]
#[path = "fetch_tests.rs"]
mod tests;
