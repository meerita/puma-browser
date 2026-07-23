// @file crates/browser-mcp/tests/mcp_error.rs
// @description Verifies McpError reason-code mapping and CoreError wrapping at the client boundary.
// @layer mcp
// @created meerita <meerita@icloud.com>

use browser_core::{CoreError, NavigationController};
use browser_mcp::{McpError, McpServer};
use browser_network::NetworkError;

#[test]
fn core_error_maps_into_mcp_error() {
    let mcp_error: McpError = CoreError::NavigationFailed.into();
    assert!(matches!(mcp_error, McpError::Core(_)));
}

#[test]
fn navigation_failure_reports_navigation_failed_reason_code() {
    let mcp_error = McpError::from(CoreError::NavigationFailed);
    assert_eq!(mcp_error.reason_code(), "NAVIGATION_FAILED");
}

#[test]
fn tab_not_found_reports_tab_not_found_reason_code() {
    let mcp_error = McpError::from(CoreError::TabNotFound);
    assert_eq!(mcp_error.reason_code(), "TAB_NOT_FOUND");
}

#[test]
fn permission_denied_reports_permission_denied_reason_code() {
    let mcp_error = McpError::PermissionDenied;
    assert_eq!(mcp_error.reason_code(), "PERMISSION_DENIED");
}

#[test]
fn network_failure_reports_network_error_reason_code() {
    let mcp_error = McpError::from(CoreError::from(NetworkError::Timeout));
    assert_eq!(mcp_error.reason_code(), "NETWORK_ERROR");
}

#[test]
fn missing_document_reports_document_not_loaded_reason_code() {
    let mcp_error = McpError::DocumentNotLoaded;
    assert_eq!(mcp_error.reason_code(), "DOCUMENT_NOT_LOADED");
}

#[test]
fn placeholder_server_serve_returns_typed_error() {
    let mut server = McpServer::new(NavigationController::new());
    let error = server
        .serve()
        .expect_err("serve is a placeholder in this milestone and must return an error");
    assert_eq!(error.reason_code(), "NAVIGATION_FAILED");
}

#[test]
fn placeholder_server_exposes_its_controller() {
    let server = McpServer::new(NavigationController::new());
    // The adapter holds the core it will serve; the getter proves the wiring exists.
    let _controller: &NavigationController = server.controller();
}
