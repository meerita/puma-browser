// @file crates/browser-mcp/src/ssrf.rs
// @description SSRF protection guard: rejects private, loopback, and link-local URLs.
// @layer mcp
// @created meerita <meerita@icloud.com>

use std::net::IpAddr;

use browser_core::BrowserUrl;

use crate::McpError;

pub(crate) fn ssrf_guard(url: &BrowserUrl) -> Result<(), McpError> {
    let raw = url.as_str();

    let scheme_end = raw.find("://").unwrap_or(0);
    let scheme = &raw[..scheme_end];
    if scheme != "http" && scheme != "https" {
        return Err(McpError::SsrfBlocked);
    }

    if let Some(host) = extract_host(raw) {
        if let Ok(ip) = host
            .trim_matches(|c: char| c == '[' || c == ']')
            .parse::<IpAddr>()
        {
            if is_blocked_address(ip) {
                return Err(McpError::SsrfBlocked);
            }
        }
    }

    Ok(())
}

fn extract_host(url: &str) -> Option<&str> {
    let after_scheme = url.find("://").map(|i| &url[i + 3..])?;
    let host_and_port = after_scheme.split('/').next()?;
    if host_and_port.starts_with('[') {
        host_and_port.find(']').map(|i| &host_and_port[..=i])
    } else {
        Some(host_and_port.split(':').next().unwrap_or(host_and_port))
    }
}

fn is_blocked_address(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback() || v4.is_private() || v4.is_link_local() || v4.is_unspecified()
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_unspecified()
                // fe80::/10 link-local — not covered by std's is_unicast_link_local before 1.x stabilisation
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

#[cfg(test)]
#[path = "ssrf_tests.rs"]
mod tests;
