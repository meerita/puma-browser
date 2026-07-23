// @file crates/browser-terminal/tests/terminal_app.rs
// @description Verifies the placeholder TerminalApp constructs and reports a typed error.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::NavigationController;
use browser_terminal::{TerminalApp, TerminalError};

#[test]
fn run_before_rendering_is_implemented_returns_render_failed() {
    let mut app = TerminalApp::new(NavigationController::new());
    let outcome = app.run();
    assert!(matches!(outcome, Err(TerminalError::RenderFailed)));
}

#[test]
fn app_exposes_the_navigation_controller_it_drives() {
    let app = TerminalApp::new(NavigationController::new());
    // Reading the controller back confirms construction wired it in.
    let _controller = app.controller();
}
