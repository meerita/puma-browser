//! @file crates/browser-mcp/src/lib.rs
//! @description MCP adapter crate root: server skeleton and error taxonomy over browser-core.
//! @layer mcp
//! @created meerita <meerita@icloud.com>

mod error;

pub use error::McpError;

use browser_core::{CoreError, NavigationController};

/// Serves the navigation core to an MCP client over stdio.
///
/// This is the output adapter the MCP binary builds on, a sibling of the terminal
/// adapter that it never calls into. It is a placeholder in this milestone: the stdio
/// loop, tools, resources, and the permission model are not implemented yet, so
/// [`McpServer::serve`] reports a typed error rather than accepting a connection.
///
/// Web-content isolation is a hard invariant for the later build that fills this in:
/// page text is always tagged as untrusted remote content, tool responses never carry
/// cookie values, tokens, or password values, and a web page can never invoke a tool,
/// discover a client, or change a privacy setting.
#[derive(Debug)]
pub struct McpServer {
    controller: NavigationController,
}

impl McpServer {
    pub fn new(controller: NavigationController) -> Self {
        Self { controller }
    }

    /// Borrows the navigation core this adapter serves.
    pub fn controller(&self) -> &NavigationController {
        &self.controller
    }

    /// Runs the MCP stdio server until the client disconnects.
    ///
    /// Not implemented in this milestone. The server has no navigation core to drive
    /// yet, so it returns [`McpError::Core`] reporting the `NAVIGATION_FAILED` reason
    /// code rather than panicking or accepting a connection.
    pub fn serve(&mut self) -> Result<(), McpError> {
        Err(McpError::from(CoreError::NavigationFailed))
    }
}
