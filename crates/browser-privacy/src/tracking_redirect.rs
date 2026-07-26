// @file crates/browser-privacy/src/tracking_redirect.rs
// @description Recognizes search-engine tracking-redirect wrappers and decodes their destination.
// @layer privacy
// @created meerita <meerita@icloud.com>

/// A known search-engine tracking-redirect wrapper: activating a link on this host and
/// path sends the click through the engine's tracker, with the real destination carried in
/// a query parameter. Matching on host and path and the parameter's presence, not the
/// parameter name alone, avoids mis-reading a legitimate URL that happens to carry a
/// `url=`/`q=` parameter.
struct WrapperDescriptor {
    host: &'static str,
    path: &'static str,
    destination_parameter: &'static str,
}

/// The wrappers Puma unwraps. Adding an engine is a new entry here, not new code.
const KNOWN_WRAPPERS: &[WrapperDescriptor] = &[WrapperDescriptor {
    host: "duckduckgo.com",
    path: "/l/",
    destination_parameter: "uddg",
}];

/// The decoded destination of a known tracking-redirect wrapper, or `None`.
///
/// Returns `Some(destination)` only when `candidate` parses as a URL whose host and path
/// match a known wrapper and whose query carries the wrapper's destination parameter with a
/// non-empty value. The value is percent-decoded by the URL parser. The rest of the
/// wrapper's query (the tracker signature) is discarded. This is a pure string-to-string
/// transform: no network, no I/O, no scheme validation. The caller re-validates the
/// returned string before navigating to it.
pub fn unwrap_tracking_redirect(candidate: &str) -> Option<String> {
    let parsed = url::Url::parse(candidate).ok()?;
    let host = parsed.host_str()?;
    let descriptor = KNOWN_WRAPPERS
        .iter()
        .find(|wrapper| wrapper.host == host && wrapper.path == parsed.path())?;
    let destination = parsed
        .query_pairs()
        .find(|(name, _)| name == descriptor.destination_parameter)
        .map(|(_, value)| value.into_owned())?;
    if destination.is_empty() {
        return None;
    }
    Some(destination)
}
