// @file crates/browser-terminal/src/clipboard_tests.rs
// @description Unit tests for the OSC 52 sequence builder and its size-bound decision.
// @layer terminal
// @created meerita <meerita@icloud.com>

use super::{build_osc52_sequence, text_fits_osc52_limit, MAX_OSC52_BYTES};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;

#[test]
fn ascii_text_is_wrapped_in_the_osc52_frame() {
    let sequence = build_osc52_sequence("hello");
    let expected = format!("\x1b]52;c;{}\x07", STANDARD.encode("hello"));
    assert_eq!(sequence, expected);
    assert_eq!(sequence, "\x1b]52;c;aGVsbG8=\x07");
}

#[test]
fn multi_byte_text_is_base64_encoded_from_utf8_bytes() {
    let selection = "café — 日本語";
    let sequence = build_osc52_sequence(selection);
    let expected = format!("\x1b]52;c;{}\x07", STANDARD.encode(selection.as_bytes()));
    assert_eq!(sequence, expected);
}

#[test]
fn empty_text_produces_a_frame_with_no_payload() {
    let sequence = build_osc52_sequence("");
    assert_eq!(sequence, "\x1b]52;c;\x07");
}

#[test]
fn text_at_the_limit_is_allowed_through_osc52() {
    let selection = "a".repeat(MAX_OSC52_BYTES);
    assert!(text_fits_osc52_limit(&selection));
}

#[test]
fn text_beyond_the_limit_is_rejected_for_osc52() {
    let selection = "a".repeat(MAX_OSC52_BYTES + 1);
    assert!(!text_fits_osc52_limit(&selection));
}
