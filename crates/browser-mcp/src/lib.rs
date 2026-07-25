//! @file crates/browser-mcp/src/lib.rs
//! @description MCP stdio server: three read-only tools over the navigation controller.
//! @layer mcp
//! @created meerita <meerita@icloud.com>

mod error;
mod extract;
mod permission;
mod response;
mod ssrf;

pub use error::McpError;

use std::sync::Arc;

use browser_core::{BrowserUrl, NavigationController};
use rmcp::handler::server::{router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerInfo};
use rmcp::{tool, tool_handler, tool_router, ErrorData, ServerHandler, ServiceExt};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use extract::{extract_links, extract_text};
use permission::{PermissionSet, PermissionState};
use response::tagged_response;
use ssrf::ssrf_guard;

#[derive(Debug, Deserialize, JsonSchema)]
struct BrowserOpenParams {
    #[schemars(description = "URL to navigate to. Must use https:// or http://.")]
    url: String,
}

#[derive(Clone)]
pub struct McpServer {
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
    controller: Arc<Mutex<NavigationController>>,
    permissions: PermissionSet,
}

#[tool_router]
impl McpServer {
    pub fn new(controller: NavigationController) -> Self {
        Self {
            tool_router: Self::tool_router(),
            controller: Arc::new(Mutex::new(controller)),
            permissions: PermissionSet::defaults(),
        }
    }

    pub async fn run(self) -> Result<(), McpError> {
        let service = self
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|error| McpError::Protocol(anyhow::anyhow!("{}", error)))?;
        service
            .waiting()
            .await
            .map_err(|error| McpError::Protocol(anyhow::anyhow!("{}", error)))?;
        Ok(())
    }

    #[tool(
        name = "browser_open",
        description = "Navigate to a URL. Returns the page title, byte count, and script count. The URL must use https:// or http://. Private and loopback addresses are blocked."
    )]
    async fn browser_open(
        &self,
        Parameters(params): Parameters<BrowserOpenParams>,
    ) -> Result<CallToolResult, ErrorData> {
        if self.permissions.navigate != PermissionState::Allow {
            return Err(ErrorData::invalid_request("PERMISSION_DENIED", None));
        }

        let browser_url = BrowserUrl::parse(&params.url)
            .map_err(|_| ErrorData::invalid_params("Invalid URL", None))?;

        ssrf_guard(&browser_url).map_err(|_| ErrorData::invalid_request("SSRF_BLOCKED", None))?;

        let mut ctrl = self.controller.lock().await;
        ctrl.load(browser_url).await.map_err(|error| {
            ErrorData::internal_error(McpError::from(error).reason_code().to_string(), None)
        })?;

        let url_str = ctrl
            .current_url()
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();
        let title = ctrl
            .current_title()
            .map(|t| t.as_str().to_string())
            .unwrap_or_default();
        let byte_count = ctrl.page_byte_count();
        let script_count = ctrl.script_count();

        let payload = tagged_response(
            &url_str,
            json!({
                "title": title,
                "byte_count": byte_count,
                "script_count": script_count,
            }),
        );

        let content = ContentBlock::json(payload)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        Ok(CallToolResult::success(vec![content]))
    }

    #[tool(
        name = "browser_read",
        description = "Return the current page as plain text. Call browser_open first to load a page. Returns DOCUMENT_NOT_LOADED if no page is loaded."
    )]
    async fn browser_read(&self) -> Result<CallToolResult, ErrorData> {
        if self.permissions.read_pages != PermissionState::Allow {
            return Err(ErrorData::invalid_request("PERMISSION_DENIED", None));
        }

        let ctrl = self.controller.lock().await;

        let document = ctrl
            .current_document()
            .ok_or_else(|| ErrorData::invalid_request("DOCUMENT_NOT_LOADED", None))?;

        let url_str = ctrl
            .current_url()
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();

        let text = extract_text(document.children());

        let payload = tagged_response(&url_str, json!({ "text": text }));

        let content = ContentBlock::json(payload)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        Ok(CallToolResult::success(vec![content]))
    }

    #[tool(
        name = "browser_list_links",
        description = "Return all links on the current page as a JSON array of objects with 'text' and 'url' fields. Call browser_open first."
    )]
    async fn browser_list_links(&self) -> Result<CallToolResult, ErrorData> {
        if self.permissions.read_pages != PermissionState::Allow {
            return Err(ErrorData::invalid_request("PERMISSION_DENIED", None));
        }

        let ctrl = self.controller.lock().await;

        let document = ctrl
            .current_document()
            .ok_or_else(|| ErrorData::invalid_request("DOCUMENT_NOT_LOADED", None))?;

        let url_str = ctrl
            .current_url()
            .map(|u| u.as_str().to_string())
            .unwrap_or_default();

        let links = extract_links(document.children());
        let links_json = serde_json::to_value(&links)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;

        let payload = tagged_response(&url_str, json!({ "links": links_json }));

        let content = ContentBlock::json(payload)
            .map_err(|error| ErrorData::internal_error(error.to_string(), None))?;
        Ok(CallToolResult::success(vec![content]))
    }
}

#[tool_handler]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("puma-browser", env!("CARGO_PKG_VERSION")),
        )
    }
}
