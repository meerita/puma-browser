// @file crates/browser-terminal/src/cookie_view_tests.rs
// @description Tests for cookie line composition, sanitization, and flag labels.
// @layer terminal
// @created meerita <meerita@icloud.com>

use super::{compose_cookie_line, flag_list};

#[test]
fn a_control_character_in_the_name_is_stripped_before_it_reaches_the_line() {
    let line = compose_cookie_line(
        "sid\u{1b}[31m",
        "https://example.com",
        true,
        None,
        false,
        "lax",
        false,
        false,
    );
    assert!(
        !line.contains('\u{1b}'),
        "escape byte must not survive into the line: {line:?}"
    );
    assert!(
        !line.chars().any(|character| character.is_control()),
        "no control character may reach the display line: {line:?}"
    );
    assert!(
        line.contains("sid[31m"),
        "visible name text is kept: {line}"
    );
}

#[test]
fn a_control_character_in_the_origin_is_stripped() {
    let line = compose_cookie_line(
        "sid",
        "https://exa\u{7}mple.com",
        false,
        Some("third-party policy"),
        true,
        "strict",
        true,
        true,
    );
    assert!(!line.chars().any(|character| character.is_control()));
    assert!(line.contains("https://example.com"));
}

#[test]
fn an_accepted_line_carries_no_reason() {
    let line = compose_cookie_line(
        "sid",
        "https://example.com",
        true,
        None,
        false,
        "lax",
        false,
        false,
    );
    assert!(
        !line.contains("reason="),
        "accepted line has no reason: {line}"
    );
    assert!(line.contains("first-party"));
    assert!(line.contains("expiry=session"));
    assert!(line.contains("samesite=lax"));
}

#[test]
fn a_rejected_line_shows_its_reason() {
    let line = compose_cookie_line(
        "track",
        "https://ads.example",
        false,
        Some("third-party policy"),
        true,
        "none",
        true,
        false,
    );
    assert!(line.contains("reason=third-party policy"), "line: {line}");
    assert!(line.contains("third-party"));
    assert!(line.contains("expiry=persistent"));
}

#[test]
fn an_empty_name_renders_as_unnamed() {
    let line = compose_cookie_line(
        "",
        "https://example.com",
        true,
        None,
        false,
        "unset",
        false,
        false,
    );
    assert!(line.contains("(unnamed)"), "line: {line}");
}

#[test]
fn the_flag_label_names_every_combination() {
    assert_eq!(flag_list(true, true), "secure httponly");
    assert_eq!(flag_list(true, false), "secure");
    assert_eq!(flag_list(false, true), "httponly");
    assert_eq!(flag_list(false, false), "no-flags");
}
