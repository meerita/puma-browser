// @file crates/browser-core/src/address_resolver_tests.rs
// @description Unit tests for the private Windows absolute-path predicate in the address resolver.
// @layer core
// @created meerita <meerita@icloud.com>

use super::is_windows_absolute_path;

#[test]
fn backslash_drive_letter_path_is_windows_absolute() {
    assert!(is_windows_absolute_path("C:\\Users\\me\\page.html"));
}

#[test]
fn forward_slash_drive_letter_path_is_windows_absolute() {
    assert!(is_windows_absolute_path("C:/Users/me/page.html"));
}

#[test]
fn lowercase_drive_letter_path_is_windows_absolute() {
    assert!(is_windows_absolute_path("d:\\data\\index.html"));
}

#[test]
fn unc_path_is_windows_absolute() {
    assert!(is_windows_absolute_path("\\\\server\\share\\page.html"));
}

#[test]
fn posix_absolute_path_is_not_windows_absolute() {
    assert!(!is_windows_absolute_path("/etc/hosts"));
}

#[test]
fn web_address_is_not_windows_absolute() {
    assert!(!is_windows_absolute_path("https://example.com"));
}

#[test]
fn drive_letter_without_root_separator_is_not_windows_absolute() {
    assert!(!is_windows_absolute_path("C:relative"));
}

#[test]
fn bare_drive_letter_is_not_windows_absolute() {
    assert!(!is_windows_absolute_path("C:"));
}
