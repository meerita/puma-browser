// @file crates/browser-network/src/fetch.rs
// @description Resource acquisition: single-hop fetch, redirect-safety helpers, and cookie header transport.
// @layer network
// @created meerita <meerita@icloud.com>

use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::{CONTENT_TYPE, COOKIE, LOCATION, SET_COOKIE};
use reqwest::StatusCode;
use tokio::sync::watch;
use url::Url;

use crate::browser_url::BrowserUrl;
use crate::error::NetworkError;
use crate::fetched_document::FetchedDocument;
use crate::request_body::RequestBody;
use crate::request_headers::RequestHeaders;
use crate::request_method::RequestMethod;

/// Largest response body accepted, in bytes. Enforced while streaming, not after
/// buffering, so an oversized body is abandoned as soon as the cap is crossed.
const MAX_RESPONSE_BYTES: u64 = 10 * 1024 * 1024;

/// Largest combined size of response headers accepted, approximated as the sum of each
/// header's name and value length plus 4 bytes for `": "` and `"\r\n"` per line.
/// Enforced before any header value is trusted, on every redirect hop.
const MAX_RESPONSE_HEADER_BYTES: usize = 32 * 1024;

/// Largest number of redirects followed before the request is abandoned.
///
/// The core-driven redirect loop reads this to bound its hop count, so it is public.
pub const MAX_REDIRECT_COUNT: usize = 10;

/// How long a single request may take before it is abandoned.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// The result of a single HTTP hop.
///
/// A `Redirect` carries the redirect status, the raw `Location` value, and any
/// `Set-Cookie` lines from that response; its body is never read. A `Final` carries
/// the fully collected document. The caller drives the redirect loop and decides,
/// hop by hop, what outgoing `Cookie` header to send next.
pub enum HopOutcome {
    Redirect {
        status: u16,
        location: String,
        set_cookie_lines: Vec<String>,
    },
    Final(FetchedDocument),
}

/// Acquire `url` and return the raw document, dispatching on its scheme.
///
/// A `file://` URL is read from disk; any other URL is fetched over HTTP or HTTPS.
/// The public signature is identical for both paths so callers do not distinguish
/// local from remote acquisition.
pub async fn fetch(
    url: &BrowserUrl,
    headers: &RequestHeaders,
) -> Result<FetchedDocument, NetworkError> {
    let (progress_tx, _) = watch::channel(0usize);
    fetch_with_progress(url, headers, progress_tx).await
}

/// Acquire `url` and stream byte-count updates to `progress` as chunks arrive.
///
/// Behaves identically to [`fetch`] but reports the running total of bytes received
/// through `progress` after each chunk. For `file://` URLs, which are read in one
/// shot, the final file size is sent once after the read completes. The send is
/// silently ignored when all receivers have been dropped.
///
/// This drives its own redirect loop over [`fetch_once`], sending no `Cookie` header,
/// so existing callers that do not manage cookies see unchanged behavior.
pub async fn fetch_with_progress(
    url: &BrowserUrl,
    headers: &RequestHeaders,
    progress: watch::Sender<usize>,
) -> Result<FetchedDocument, NetworkError> {
    let mut current = url.clone();
    let mut redirect_count: usize = 0;
    loop {
        let location = match fetch_once(&current, None, headers, progress.clone()).await? {
            HopOutcome::Final(document) => return Ok(document),
            HopOutcome::Redirect { location, .. } => location,
        };
        redirect_count += 1;
        if redirect_count > MAX_REDIRECT_COUNT {
            return Err(NetworkError::TooManyRedirects);
        }
        current = resolve_redirect(&current, &location)?;
    }
}

/// Perform exactly one HTTP hop for `url` and return its outcome.
///
/// When `cookie_header` is `Some`, its value is sent verbatim as the `Cookie` request
/// header; the network layer never builds or interprets it. Every `Set-Cookie`
/// response header is captured verbatim. A 3xx response carrying a `Location` returns
/// [`HopOutcome::Redirect`] without reading the body; any other response returns
/// [`HopOutcome::Final`] with the body collected under the size cap. A `file://` URL
/// is read from disk and returns `Final` with no `Set-Cookie` lines.
pub async fn fetch_once(
    url: &BrowserUrl,
    cookie_header: Option<&str>,
    headers: &RequestHeaders,
    progress: watch::Sender<usize>,
) -> Result<HopOutcome, NetworkError> {
    if url.scheme() == "file" {
        let document = read_local_file(url).await?;
        let _ = progress.send(document.body_bytes().len());
        return Ok(HopOutcome::Final(document));
    }
    fetch_once_over_http(url, cookie_header, headers, &progress).await
}

async fn fetch_once_over_http(
    url: &BrowserUrl,
    cookie_header: Option<&str>,
    headers: &RequestHeaders,
    progress: &watch::Sender<usize>,
) -> Result<HopOutcome, NetworkError> {
    let client = build_client()?;
    let mut request = client.get(url.as_str());
    request = headers.apply(request);
    if let Some(cookie_header) = cookie_header {
        request = request.header(COOKIE, cookie_header);
    }
    run_http_request(request, progress).await
}

/// Send a built request and turn its response into a [`HopOutcome`].
///
/// Shared by every HTTP-issuing path (`fetch_once`, `submit_once`): checks the
/// response header size cap, captures every `Set-Cookie` line, and returns a
/// `Redirect` outcome without reading the body or a `Final` outcome with the body
/// collected under the size cap.
async fn run_http_request(
    request: reqwest::RequestBuilder,
    progress: &watch::Sender<usize>,
) -> Result<HopOutcome, NetworkError> {
    let response = request.send().await.map_err(map_send_error)?;
    if response_header_bytes(&response) > MAX_RESPONSE_HEADER_BYTES {
        return Err(NetworkError::ResponseHeadersTooLarge);
    }
    let status = response.status();
    let set_cookie_lines = collect_set_cookie_lines(&response);
    if let Some((status, location)) = redirect_target(status, &response) {
        return Ok(HopOutcome::Redirect {
            status,
            location,
            set_cookie_lines,
        });
    }
    let document =
        collect_document_reporting_progress(response, set_cookie_lines, progress).await?;
    Ok(HopOutcome::Final(document))
}

/// Submit a form to `url` with `method` and `body`, sharing the same cookie-header-in/
/// `Set-Cookie`-out/redirect-detection path as [`fetch_once`].
///
/// Only `http` and `https` schemes are accepted; a form must never submit to `file://`
/// or any other scheme. The caller drives any resulting redirect loop, exactly as it
/// does for [`fetch_once`].
pub async fn submit_once(
    url: &BrowserUrl,
    method: RequestMethod,
    body: &RequestBody,
    cookie_header: Option<&str>,
    headers: &RequestHeaders,
    progress: watch::Sender<usize>,
) -> Result<HopOutcome, NetworkError> {
    if !scheme_is_http(url.scheme()) {
        return Err(NetworkError::UnsupportedScheme {
            scheme: url.scheme().to_string(),
        });
    }
    let client = build_client()?;
    let mut request = build_submission_request(&client, url, method, body);
    request = headers.apply(request);
    if let Some(cookie_header) = cookie_header {
        request = request.header(COOKIE, cookie_header);
    }
    run_http_request(request, &progress).await
}

/// Build the `reqwest::RequestBuilder` for one `(method, body)` submission combination.
fn build_submission_request(
    client: &reqwest::Client,
    url: &BrowserUrl,
    method: RequestMethod,
    body: &RequestBody,
) -> reqwest::RequestBuilder {
    match (method, body) {
        (RequestMethod::Get, RequestBody::None) => client.get(url.as_str()),
        (RequestMethod::Get, RequestBody::UrlEncoded(pairs)) => {
            let mut target = url.as_url().clone();
            target.query_pairs_mut().clear().extend_pairs(pairs);
            client.get(target.as_str())
        }
        (RequestMethod::Post, RequestBody::None) => client.post(url.as_str()),
        (RequestMethod::Post, RequestBody::UrlEncoded(pairs)) => {
            client.post(url.as_str()).form(pairs)
        }
    }
}

/// Approximate wire size of a response's headers, in bytes.
///
/// A parsed `HeaderMap` has no exact wire byte count, so each header line is
/// approximated as its name length plus its value length plus 4 (for `": "` and
/// `"\r\n"`), summed over every header.
fn response_header_bytes(response: &reqwest::Response) -> usize {
    response
        .headers()
        .iter()
        .map(|(name, value)| name.as_str().len() + value.len() + 4)
        .sum()
}

/// The redirect target of a response: `Some((status, location))` only when the status
/// is a 3xx and a usable `Location` header is present. Otherwise the response is final.
fn redirect_target(status: StatusCode, response: &reqwest::Response) -> Option<(u16, String)> {
    if !status.is_redirection() {
        return None;
    }
    let location = redirect_location(response)?;
    Some((status.as_u16(), location))
}

/// Every `Set-Cookie` response header value, in order, as owned strings.
///
/// The values are opaque here: the network layer never parses or interprets them.
fn collect_set_cookie_lines(response: &reqwest::Response) -> Vec<String> {
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .map(|value| value.to_string())
        .collect()
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
    Ok(FetchedDocument::new(
        url.clone(),
        content_type,
        None,
        body,
        0usize,
        Vec::new(),
    ))
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
/// Public so the core-driven redirect loop can reuse the same safety checks per hop.
pub fn resolve_redirect(current: &BrowserUrl, location: &str) -> Result<BrowserUrl, NetworkError> {
    let next = current
        .as_url()
        .join(location)
        .map_err(|_| NetworkError::RequestFailed)?;
    if !scheme_is_http(next.scheme()) {
        return Err(NetworkError::RequestFailed);
    }
    if redirect_is_downgrade(current.as_url(), &next) {
        return Err(NetworkError::RequestFailed);
    }
    Ok(BrowserUrl::from_validated(next))
}

fn scheme_is_http(scheme: &str) -> bool {
    scheme == "http" || scheme == "https"
}

fn redirect_is_downgrade(current: &Url, next: &Url) -> bool {
    current.scheme() == "https" && next.scheme() == "http"
}

async fn collect_document_reporting_progress(
    response: reqwest::Response,
    set_cookie_lines: Vec<String>,
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
    let wire_byte_count = response.content_length().map(|n| n as usize).unwrap_or(0);
    let body = read_bounded_body_reporting_progress(response, progress).await?;
    Ok(FetchedDocument::new(
        final_url,
        content_type,
        charset,
        body,
        wire_byte_count,
        set_cookie_lines,
    ))
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
