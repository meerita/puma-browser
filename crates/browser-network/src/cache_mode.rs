// @file crates/browser-network/src/cache_mode.rs
// @description HTTP cache mode enum for the network layer.
// @layer network
// @created meerita <meerita@icloud.com>

/// Where, if anywhere, fetched responses are cached.
///
/// This is a domain enum and is deliberately not `serde`-derived; adapters own the
/// wire and storage representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheMode {
    None,
    Memory,
    Disk,
}
