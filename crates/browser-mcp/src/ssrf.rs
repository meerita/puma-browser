// @file crates/browser-mcp/src/ssrf.rs
// @description SSRF protection guard: rejects private, loopback, and link-local URLs.
// @layer mcp
// @created meerita <meerita@icloud.com>

use std::net::IpAddr;

use browser_core::BrowserUrl;

use crate::McpError;

pub(crate) fn ssrf_guard(url: &BrowserUrl) -> Result<(), McpError> {
    let scheme = url.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(McpError::SsrfBlocked);
    }

    if let Some(host_str) = url.host_str() {
        if let Ok(ip) = host_str
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
