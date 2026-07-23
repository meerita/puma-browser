// @file crates/browser-cli/src/run_mode_tests.rs
// @description Verifies argument resolution selects MCP, a URL load target, a blank page, or an error.
// @layer cli
// @created meerita <meerita@icloud.com>

use super::{resolve_mode, ResolvedMode};

fn resolved_for(arguments: &[&str]) -> ResolvedMode {
    resolve_mode(arguments.iter().map(|argument| argument.to_string()))
}

#[test]
fn no_arguments_opens_the_terminal_on_a_blank_page() {
    assert!(matches!(resolved_for(&[]), ResolvedMode::TerminalBlank));
}

#[test]
fn mcp_keyword_selects_mcp_mode() {
    assert!(matches!(resolved_for(&["mcp"]), ResolvedMode::Mcp));
}

#[test]
fn terminal_keyword_without_a_url_opens_a_blank_page() {
    assert!(matches!(
        resolved_for(&["terminal"]),
        ResolvedMode::TerminalBlank
    ));
}

#[test]
fn valid_url_argument_becomes_the_terminal_load_target() {
    let resolved = resolved_for(&["https://example.com"]);
    let ResolvedMode::TerminalUrl(url) = resolved else {
        panic!("a valid URL must resolve to a terminal load target");
    };
    assert_eq!(url.host_str(), Some("example.com"));
}

#[test]
fn bare_host_argument_is_assumed_to_be_https() {
    let resolved = resolved_for(&["example.com"]);
    let ResolvedMode::TerminalUrl(url) = resolved else {
        panic!("a bare host must resolve to a terminal load target");
    };
    assert_eq!(url.scheme(), "https");
}

#[test]
fn unsupported_scheme_argument_resolves_to_a_usage_error() {
    assert!(matches!(
        resolved_for(&["ftp://example.com"]),
        ResolvedMode::UsageError(_)
    ));
}

#[test]
fn malformed_url_argument_resolves_to_a_usage_error() {
    assert!(matches!(
        resolved_for(&["http://"]),
        ResolvedMode::UsageError(_)
    ));
}

#[test]
fn mcp_keyword_wins_over_a_later_url_argument() {
    assert!(matches!(
        resolved_for(&["mcp", "https://example.com"]),
        ResolvedMode::Mcp
    ));
}
