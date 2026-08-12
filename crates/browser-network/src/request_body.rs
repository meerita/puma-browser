// @file crates/browser-network/src/request_body.rs
// @description Request body carried by a form submission request.
// @layer network
// @created meerita <meerita@icloud.com>

/// The body a form submission request carries.
///
/// `None` is a body kind, not `Option::None`: it names the absence of a body, mirroring
/// [`crate::CacheMode::None`]. This is a domain enum and is deliberately not
/// `serde`-derived; adapters own the wire representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestBody {
    None,
    UrlEncoded(Vec<(String, String)>),
}
