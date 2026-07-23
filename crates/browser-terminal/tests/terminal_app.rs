// @file crates/browser-terminal/tests/terminal_app.rs
// @description Verifies the terminal app constructs for each initial view and exposes its core.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::NavigationController;
use browser_terminal::{InitialView, TerminalApp};

#[test]
fn app_constructs_with_a_blank_initial_view() {
    let app = TerminalApp::new(NavigationController::new(), InitialView::Blank);
    // Reading the controller back confirms construction wired it in.
    let _controller = app.controller();
}

#[test]
fn app_constructs_with_an_error_initial_view() {
    let view = InitialView::Error("Connection failed".to_string());
    let app = TerminalApp::new(NavigationController::new(), view);
    let _controller = app.controller();
}
