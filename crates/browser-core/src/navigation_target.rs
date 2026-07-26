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

/// Whether `classify_navigation` should try to unwrap a known tracking-redirect wrapper
/// into its real destination before classifying. The caller decides; the default product
/// behavior is `Enabled`, disabled only by the user's environment toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackingUnwrap {
    Enabled,
    Disabled,
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
    tracking_unwrap: TrackingUnwrap,
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
    let resolved = unwrap_when_enabled(current, resolved, tracking_unwrap);
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

/// Replace a tracking-redirect wrapper with its decoded destination when unwrapping is
/// enabled and the destination is safe, otherwise return `resolved` unchanged.
///
/// The unwrap is skipped when disabled, when `resolved` is not a known wrapper, when the
/// decoded destination fails to parse, or when it fails the origin-aware safety check. Any
/// of those cases falls back to the wrapper URL, so the feature never widens what a link can
/// reach and never errors.
fn unwrap_when_enabled(
    current: Option<&BrowserUrl>,
    resolved: BrowserUrl,
    tracking_unwrap: TrackingUnwrap,
) -> BrowserUrl {
    if tracking_unwrap == TrackingUnwrap::Disabled {
        return resolved;
    }
    let Some(destination) = browser_privacy::unwrap_tracking_redirect(resolved.as_str()) else {
        return resolved;
    };
    let Ok(destination_url) = BrowserUrl::parse(&destination) else {
        return resolved;
    };
    if !unwrapped_destination_is_safe(current, &destination_url) {
        return resolved;
    }
    destination_url
}

/// Whether a destination decoded from a tracking-redirect wrapper is safe to navigate to.
///
/// The unwrap is a redirect the browser performs itself, so it obeys the same
/// unsafe-transition rules a server redirect from a web origin obeys: the destination must
/// be `http` or `https` (never `file://`, which `BrowserUrl::parse` would otherwise accept),
/// and an `https` origin may not be downgraded to `http`. A destination that fails either
/// rule falls back to the wrapper, so the unwrap can never widen what a link can reach.
fn unwrapped_destination_is_safe(current: Option<&BrowserUrl>, destination: &BrowserUrl) -> bool {
    if !destination_scheme_is_web(destination) {
        return false;
    }
    !is_downgrade(current, destination)
}

/// Whether `destination` uses an `http` or `https` scheme.
fn destination_scheme_is_web(destination: &BrowserUrl) -> bool {
    destination.scheme() == "http" || destination.scheme() == "https"
}

/// Whether navigating from `current` to `destination` downgrades an `https` origin to `http`.
fn is_downgrade(current: Option<&BrowserUrl>, destination: &BrowserUrl) -> bool {
    let Some(current) = current else {
        return false;
    };
    current.scheme() == "https" && destination.scheme() == "http"
}

/// Whether `base` names the same page as the currently loaded one, comparing without
/// fragments so `page` and `page#section` are the same page.
fn is_same_page(current: Option<&BrowserUrl>, base: &BrowserUrl) -> bool {
    let Some(current) = current else {
        return false;
    };
    current.without_fragment().as_str() == base.as_str()
}
