// @file crates/browser-terminal/src/cookie_view.rs
// @description Formats secret-free cookie records into sanitized inspection lines for display.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::{CookieRecord, RejectionReason, SameSite};

use crate::history_view::strip_control;

/// Which decision a detail listing shows: the accepted cookies or the rejected ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CookieFilter {
    Accepted,
    Rejected,
}

/// The summary popup lines for `/cookies`: the accepted and rejected totals plus the
/// first-party and third-party split of the rejections. Every line is local text, so no
/// remote-derived string reaches the terminal on this path.
pub(crate) fn summary_lines(records: &[CookieRecord]) -> Vec<String> {
    let accepted = records.iter().filter(|record| record.accepted()).count();
    let rejected = records.len() - accepted;
    let first_party_rejected = records
        .iter()
        .filter(|record| !record.accepted() && record.first_party())
        .count();
    let third_party_rejected = rejected - first_party_rejected;
    vec![
        "Cookies".to_string(),
        format!("  Accepted: {accepted}"),
        format!("  Rejected: {rejected}"),
        format!("  First-party rejected: {first_party_rejected}"),
        format!("  Third-party rejected: {third_party_rejected}"),
        "  Esc to close".to_string(),
    ]
}

/// The detail popup lines for `/cookies accepted` or `/cookies rejected`: a header with the
/// count, then one sanitized line per matching record, or a `none` line when the filter
/// matches nothing. Every remote-derived field is sanitized by [`format_cookie_line`].
pub(crate) fn decision_lines(records: &[CookieRecord], filter: CookieFilter) -> Vec<String> {
    let want_accepted = matches!(filter, CookieFilter::Accepted);
    let mut cookie_lines: Vec<String> = records
        .iter()
        .filter(|record| record.accepted() == want_accepted)
        .map(format_cookie_line)
        .collect();
    let header = match filter {
        CookieFilter::Accepted => format!("Accepted cookies ({})", cookie_lines.len()),
        CookieFilter::Rejected => format!("Rejected cookies ({})", cookie_lines.len()),
    };
    let mut lines = vec![header];
    if cookie_lines.is_empty() {
        lines.push("  none".to_string());
    } else {
        lines.append(&mut cookie_lines);
    }
    lines.push("  Esc to close".to_string());
    lines
}

/// Formats one cookie record into a single inspection line, sanitizing the remote-derived
/// name and origin so no control byte from a `Set-Cookie` header can carry an escape
/// sequence into the terminal. The record never holds a cookie value, so none can appear.
pub(crate) fn format_cookie_line(record: &CookieRecord) -> String {
    compose_cookie_line(
        record.name(),
        record.origin(),
        record.first_party(),
        record.reason().map(reason_label),
        record.has_expiry(),
        same_site_label(record.same_site()),
        record.secure(),
        record.http_only(),
    )
}

/// Composes a cookie line from its already-classified parts, stripping control characters
/// from the two remote-derived fields (name and origin) before they enter the line. A
/// present `reason` marks a rejected cookie and is shown; an absent one is an accepted
/// cookie. Kept separate from [`format_cookie_line`] so the sanitization is testable
/// without a `CookieRecord`, which only the core can build.
#[allow(clippy::too_many_arguments)]
fn compose_cookie_line(
    name: &str,
    origin: &str,
    first_party: bool,
    reason: Option<&str>,
    has_expiry: bool,
    same_site: &str,
    secure: bool,
    http_only: bool,
) -> String {
    let sanitized_name = strip_control(name);
    let name_field = if sanitized_name.trim().is_empty() {
        "(unnamed)".to_string()
    } else {
        sanitized_name
    };
    let origin_field = strip_control(origin);
    let party = if first_party {
        "first-party"
    } else {
        "third-party"
    };
    let expiry = if has_expiry { "persistent" } else { "session" };
    let flags = flag_list(secure, http_only);
    let mut line = format!("  {name_field}  {origin_field}  {party}");
    if let Some(reason) = reason {
        line.push_str(&format!("  reason={reason}"));
    }
    line.push_str(&format!("  expiry={expiry}  samesite={same_site}  {flags}"));
    line
}

/// The Secure and HttpOnly flags as a short label, so a line always names its flag state
/// rather than leaving the reader to infer it from an absence.
fn flag_list(secure: bool, http_only: bool) -> &'static str {
    match (secure, http_only) {
        (true, true) => "secure httponly",
        (true, false) => "secure",
        (false, true) => "httponly",
        (false, false) => "no-flags",
    }
}

/// A short display word for a rejection reason. Deliberately free of internal variant
/// names so the inspection view carries no crate internals.
fn reason_label(reason: RejectionReason) -> &'static str {
    match reason {
        RejectionReason::Policy => "policy",
        RejectionReason::ThirdParty => "third-party policy",
        RejectionReason::Ask => "ask: no interactive prompt yet",
        RejectionReason::PublicSuffix => "public-suffix domain",
        RejectionReason::SecureOverInsecure => "secure cookie over http",
        RejectionReason::Malformed => "malformed",
    }
}

/// A short display word for a SameSite value.
fn same_site_label(same_site: SameSite) -> &'static str {
    match same_site {
        SameSite::Strict => "strict",
        SameSite::Lax => "lax",
        SameSite::None => "none",
        SameSite::Unset => "unset",
    }
}

#[cfg(test)]
#[path = "cookie_view_tests.rs"]
mod tests;
