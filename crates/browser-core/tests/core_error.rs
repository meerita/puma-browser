// @file crates/browser-core/tests/core_error.rs
// @description Verifies each inner-crate error maps into the matching CoreError variant.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::CoreError;
use browser_html::HtmlError;
use browser_network::NetworkError;
use browser_privacy::PrivacyError;
use browser_storage::StorageError;

#[test]
fn network_error_maps_into_core_error() {
    let mapped: CoreError = NetworkError::DnsFailure.into();
    assert!(matches!(mapped, CoreError::Network(_)));
}

#[test]
fn html_error_maps_into_core_error() {
    let mapped: CoreError = HtmlError::EmptyInput.into();
    assert!(matches!(mapped, CoreError::Parse(_)));
}

#[test]
fn storage_error_maps_into_core_error() {
    let mapped: CoreError = StorageError::NotFound.into();
    assert!(matches!(mapped, CoreError::Storage(_)));
}

#[test]
fn privacy_error_maps_into_core_error() {
    let mapped: CoreError = PrivacyError::CookieRejected.into();
    assert!(matches!(mapped, CoreError::Privacy(_)));
}
