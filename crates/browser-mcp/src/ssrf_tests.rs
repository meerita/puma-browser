// @file crates/browser-mcp/src/ssrf_tests.rs
// @description Unit tests for the SSRF guard function.
// @layer mcp
// @created meerita <meerita@icloud.com>

use browser_core::BrowserUrl;

use super::ssrf_guard;
use crate::McpError;

#[test]
fn loopback_ipv4_is_blocked() {
    let url = BrowserUrl::parse("http://127.0.0.1/").expect("valid URL");
    assert!(matches!(ssrf_guard(&url), Err(McpError::SsrfBlocked)));
}

#[test]
fn loopback_ipv6_is_blocked() {
    let url = BrowserUrl::parse("http://[::1]/").expect("valid URL");
    assert!(matches!(ssrf_guard(&url), Err(McpError::SsrfBlocked)));
}

#[test]
fn private_rfc1918_is_blocked() {
    let url = BrowserUrl::parse("http://192.168.1.1/").expect("valid URL");
    assert!(matches!(ssrf_guard(&url), Err(McpError::SsrfBlocked)));
}

#[test]
fn link_local_is_blocked() {
    let url = BrowserUrl::parse("http://169.254.169.254/").expect("valid URL");
    assert!(matches!(ssrf_guard(&url), Err(McpError::SsrfBlocked)));
}

#[test]
fn public_ip_is_allowed() {
    let url = BrowserUrl::parse("https://93.184.216.34/").expect("valid URL");
    assert!(ssrf_guard(&url).is_ok());
}

#[test]
fn file_scheme_is_blocked() {
    // BrowserUrl accepts file:// at construction; ssrf_guard rejects it via scheme check.
    let url = BrowserUrl::parse("file:///etc/passwd").expect("BrowserUrl accepts file scheme");
    assert!(matches!(ssrf_guard(&url), Err(McpError::SsrfBlocked)));
}

#[test]
fn loopback_with_userinfo_is_blocked() {
    // Userinfo in the URL must not bypass the IP check.
    let url = BrowserUrl::parse("http://attacker@127.0.0.1/").expect("valid URL");
    assert!(matches!(ssrf_guard(&url), Err(McpError::SsrfBlocked)));
}

#[test]
fn private_ip_with_userinfo_is_blocked() {
    let url = BrowserUrl::parse("http://x@192.168.1.1/admin").expect("valid URL");
    assert!(matches!(ssrf_guard(&url), Err(McpError::SsrfBlocked)));
}
