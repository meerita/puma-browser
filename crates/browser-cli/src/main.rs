//! @file crates/browser-cli/src/main.rs
//! @description Composition root: parse the run mode, wire the core to an adapter, run to exit.
//! @layer cli
//! @created meerita <meerita@icloud.com>

use anyhow::{anyhow, Result};
use browser_core::NavigationController;
use browser_mcp::McpServer;
use browser_terminal::{InitialView, TerminalApp};

/// Which output adapter the binary drives.
///
/// The terminal adapter is the default; `mcp` selects the stdio MCP server. These are
/// siblings over the same navigation core and never call into each other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Terminal,
    Mcp,
}

impl RunMode {
    fn from_argument(argument: &str) -> Option<RunMode> {
        match argument {
            "terminal" => Some(RunMode::Terminal),
            "mcp" => Some(RunMode::Mcp),
            _ => None,
        }
    }
}

/// Selects the run mode from the process arguments, defaulting to the terminal.
///
/// The first argument that names a known mode wins; anything else is ignored. A richer
/// parser is deferred until a later phase needs one.
fn parse_run_mode(arguments: impl Iterator<Item = String>) -> RunMode {
    arguments
        .filter_map(|argument| RunMode::from_argument(&argument))
        .next()
        .unwrap_or(RunMode::Terminal)
}

fn main() -> Result<()> {
    let run_mode = parse_run_mode(std::env::args().skip(1));
    let controller = NavigationController::new();
    run(run_mode, controller)
}

fn run(run_mode: RunMode, controller: NavigationController) -> Result<()> {
    match run_mode {
        RunMode::Terminal => run_terminal(controller),
        RunMode::Mcp => run_mcp(controller),
    }
}

fn run_terminal(controller: NavigationController) -> Result<()> {
    // Start on a blank view; the composition root does not yet parse a URL to load.
    let mut app = TerminalApp::new(controller, InitialView::Blank);
    // Surface only the adapter's safe status message, never raw error detail.
    app.run().map_err(|error| anyhow!(error.user_message()))
}

fn run_mcp(controller: NavigationController) -> Result<()> {
    let mut server = McpServer::new(controller);
    // Only the stable reason code is safe to show; raw error detail stays internal.
    server
        .serve()
        .map_err(|error| anyhow!("MCP server failed: {}", error.reason_code()))
}

#[cfg(test)]
#[path = "run_mode_tests.rs"]
mod tests;
