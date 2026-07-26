// @file crates/browser-terminal/tests/terminal_app.rs
// @description Verifies the terminal app constructs for each initial view and exposes its core.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::NavigationController;
use browser_terminal::{TerminalApp, TerminalSettings, ViewState};

fn settings() -> TerminalSettings {
    TerminalSettings {
        copy_on_select: true,
        force_osc52: false,
        search_enabled: true,
        unwrap_tracking: true,
    }
}

#[test]
fn app_constructs_with_a_blank_initial_view() {
    let app = TerminalApp::new(NavigationController::new(), ViewState::Blank, settings());
    // Reading the controller back confirms construction wired it in.
    let _controller = app.controller();
}

#[test]
fn app_constructs_with_an_error_initial_view() {
    let view = ViewState::Error("Connection failed".to_string());
    let app = TerminalApp::new(NavigationController::new(), view, settings());
    let _controller = app.controller();
}
