// @file crates/browser-network/src/request_headers_tests.rs
// @description Unit tests for RequestHeaders's User-Agent and Accept-Language string building.
// @layer network
// @created meerita <meerita@icloud.com>

use super::RequestHeaders;

#[test]
fn full_detection_produces_platform_and_arch_user_agent() {
    let headers = RequestHeaders::new("0.29.0", Some("macOS"), Some("15.5"), "aarch64", None);

    assert_eq!(
        headers.user_agent,
        "Puma/0.29.0 (macOS 15.5; arm64; +https://github.com/meerita/puma-browser)"
    );
}

#[test]
fn missing_os_detection_omits_platform_token() {
    let headers = RequestHeaders::new("0.29.0", None, None, "aarch64", None);

    assert_eq!(
        headers.user_agent,
        "Puma/0.29.0 (arm64; +https://github.com/meerita/puma-browser)"
    );
}

#[test]
fn default_degrades_to_arch_only_user_agent() {
    let headers = RequestHeaders::default();

    assert!(headers.user_agent.starts_with("Puma/"));
    assert!(headers
        .user_agent
        .ends_with("+https://github.com/meerita/puma-browser)"));
}

#[test]
fn non_english_locale_adds_fallback_language_tags() {
    let headers = RequestHeaders::new("0.29.0", None, None, "x86_64", Some("es-ES"));

    assert_eq!(headers.accept_language, "es-ES, es, en;q=0.5");
}

#[test]
fn english_locale_is_sent_as_is() {
    let headers = RequestHeaders::new("0.29.0", None, None, "x86_64", Some("en-US"));

    assert_eq!(headers.accept_language, "en-US");
}

#[test]
fn missing_locale_defaults_to_en_us() {
    let headers = RequestHeaders::new("0.29.0", None, None, "x86_64", None);

    assert_eq!(headers.accept_language, "en-US");
}
