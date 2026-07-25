// @file crates/browser-mcp/src/permission.rs
// @description Hardcoded permission defaults for the MCP server thin slice.
// @layer mcp
// @created meerita <meerita@icloud.com>

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PermissionState {
    Allow,
    Deny,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct PermissionSet {
    pub read_pages: PermissionState,
    pub navigate: PermissionState,
    pub manage_tabs: PermissionState,
    pub fill_forms: PermissionState,
    pub submit_forms: PermissionState,
    pub download_files: PermissionState,
    pub read_cookies: PermissionState,
    pub modify_privacy: PermissionState,
    pub access_local_files: PermissionState,
    pub access_private_networks: PermissionState,
}

impl PermissionSet {
    pub(crate) fn defaults() -> Self {
        Self {
            read_pages: PermissionState::Allow,
            navigate: PermissionState::Allow,
            manage_tabs: PermissionState::Deny,
            fill_forms: PermissionState::Deny,
            submit_forms: PermissionState::Deny,
            download_files: PermissionState::Deny,
            read_cookies: PermissionState::Deny,
            modify_privacy: PermissionState::Deny,
            access_local_files: PermissionState::Deny,
            access_private_networks: PermissionState::Deny,
        }
    }
}
