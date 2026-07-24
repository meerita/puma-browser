// @file crates/browser-core/tests/core_error_network_mapping.rs
// @description Integration tests for From<NetworkError> for CoreError: file variants map distinctly.
// @layer core
// @created meerita <meerita@icloud.com>

use browser_core::CoreError;
use browser_network::NetworkError;

#[test]
fn network_file_not_found_maps_to_local_file_not_found() {
    let mapped = CoreError::from(NetworkError::FileNotFound);
    assert!(matches!(mapped, CoreError::LocalFileNotFound));
}

#[test]
fn network_path_is_directory_maps_to_local_path_is_directory() {
    let mapped = CoreError::from(NetworkError::PathIsDirectory);
    assert!(matches!(mapped, CoreError::LocalPathIsDirectory));
}

#[test]
fn network_file_too_large_maps_to_local_file_too_large() {
    let mapped = CoreError::from(NetworkError::FileTooLarge);
    assert!(matches!(mapped, CoreError::LocalFileTooLarge));
}

#[test]
fn network_file_read_failed_maps_to_local_file_read_failed() {
    let mapped = CoreError::from(NetworkError::FileReadFailed);
    assert!(matches!(mapped, CoreError::LocalFileReadFailed));
}

#[test]
fn network_timeout_maps_to_generic_network_error() {
    let mapped = CoreError::from(NetworkError::Timeout);
    assert!(matches!(mapped, CoreError::Network(NetworkError::Timeout)));
}

#[test]
fn network_unsupported_scheme_maps_to_generic_network_error() {
    let mapped = CoreError::from(NetworkError::UnsupportedScheme {
        scheme: "ftp".to_string(),
    });
    assert!(matches!(mapped, CoreError::Network(_)));
}
