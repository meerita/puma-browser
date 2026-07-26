// @file crates/browser-core/src/navigation_target.rs
// @description Classifies a navigation reference as a same-page anchor jump or a page fetch.
// @layer core
// @created meerita <meerita@icloud.com>

use std::path::Path;

use browser_network::BrowserUrl;

use crate::address_resolver::resolve_address;
use crate::error::CoreError;

/// What a navigation reference resolves to relative to the current page.
///
/// The base URL (everything except the fragment) decides the kind: a reference whose base
/// equals the loaded page's base is an in-page jump that needs no request, and any other
/// reference is a fetch that carries the fragment so the viewport can be positioned once
/// the new page renders.
#[derive(Debug)]
pub enum NavigationTarget {
    /// A move within the current page to the given fragment. `None` and an empty fragment
    /// both mean the top of the page.
    SamePageAnchor { fragment: Option<String> },
    /// A request for `url`, carrying the fragment to honor after the page loads.
    Fetch {
        url: BrowserUrl,
        fragment: Option<String>,
    },
}

/// Classify `target_reference` against the currently loaded page.
///
/// A bare `#fragment` is always a same-page jump and requires a current page; with none it
/// returns [`CoreError::NavigationFailed`]. Any other reference is resolved through the
/// same address path used everywhere else, so scheme validation and `file://` handling are
/// unchanged. Its fragment is split off, and its base is compared with the current page's
/// base: equal bases (with a page loaded) are a same-page jump, and everything else is a
/// fetch of the fragment-free base URL.
pub fn classify_navigation(
    current: Option<&BrowserUrl>,
    target_reference: &str,
    working_directory: &Path,
) -> Result<NavigationTarget, CoreError> {
    let trimmed = target_reference.trim();
    if let Some(fragment) = trimmed.strip_prefix('#') {
        if current.is_none() {
            return Err(CoreError::NavigationFailed);
        }
        return Ok(NavigationTarget::SamePageAnchor {
            fragment: Some(fragment.to_string()),
        });
    }
    let resolved = resolve_address(trimmed, working_directory)?;
    let fragment = resolved.fragment().map(str::to_string);
    let base = resolved.without_fragment();
    if is_same_page(current, &base) {
        return Ok(NavigationTarget::SamePageAnchor { fragment });
    }
    Ok(NavigationTarget::Fetch {
        url: base,
        fragment,
    })
}

/// Whether `base` names the same page as the currently loaded one, comparing without
/// fragments so `page` and `page#section` are the same page.
fn is_same_page(current: Option<&BrowserUrl>, base: &BrowserUrl) -> bool {
    let Some(current) = current else {
        return false;
    };
    current.without_fragment().as_str() == base.as_str()
}
