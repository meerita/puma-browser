// @file crates/browser-privacy/tests/tracking_redirect.rs
// @description Behavior tests for unwrap_tracking_redirect: decoding DuckDuckGo /l/?uddg= wrappers.
// @layer privacy
// @created meerita <meerita@icloud.com>

use browser_privacy::unwrap_tracking_redirect;

#[test]
fn a_duckduckgo_wrapper_returns_the_decoded_destination_and_drops_the_signature() {
    let wrapper = "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fwww.example.com%2F&rut=abc";

    assert_eq!(
        unwrap_tracking_redirect(wrapper),
        Some("https://www.example.com/".to_string())
    );
}

#[test]
fn a_destination_carrying_its_own_query_decodes_with_the_inner_query_intact() {
    let wrapper = "https://duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fp%3Fa%3D1";

    assert_eq!(
        unwrap_tracking_redirect(wrapper),
        Some("https://example.com/p?a=1".to_string())
    );
}

#[test]
fn a_non_duckduckgo_host_returns_none() {
    let wrapper = "https://evil.test/l/?uddg=https%3A%2F%2Fwww.example.com%2F";

    assert_eq!(unwrap_tracking_redirect(wrapper), None);
}

#[test]
fn a_duckduckgo_url_on_a_different_path_returns_none() {
    let wrapper = "https://duckduckgo.com/?uddg=https%3A%2F%2Fwww.example.com%2F";

    assert_eq!(unwrap_tracking_redirect(wrapper), None);
}

#[test]
fn a_duckduckgo_wrapper_without_the_destination_parameter_returns_none() {
    let wrapper = "https://duckduckgo.com/l/?rut=abc";

    assert_eq!(unwrap_tracking_redirect(wrapper), None);
}

#[test]
fn a_duckduckgo_wrapper_with_an_empty_destination_returns_none() {
    let wrapper = "https://duckduckgo.com/l/?uddg=";

    assert_eq!(unwrap_tracking_redirect(wrapper), None);
}

#[test]
fn a_non_url_input_returns_none() {
    assert_eq!(unwrap_tracking_redirect("not a url"), None);
}
