// @file crates/browser-network/src/request_method.rs
// @description HTTP method for a form submission request.
// @layer network
// @created meerita <meerita@icloud.com>

/// The HTTP method a form submission uses.
///
/// This is a domain enum and is deliberately not `serde`-derived; adapters own the
/// wire representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestMethod {
    Get,
    Post,
}
