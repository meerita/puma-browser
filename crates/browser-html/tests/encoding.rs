// @file crates/browser-html/tests/encoding.rs
// @description Behavior tests for encoding detection and decoding at the parse boundary.
// @layer html
// @created meerita <meerita@icloud.com>

use browser_html::{parse_html, Document, SemanticNode};

/// The concatenated text of the first paragraph in a document.
fn first_paragraph_text(document: &Document) -> String {
    for node in document.children() {
        if let SemanticNode::Paragraph { runs, .. } = node {
            return runs.iter().map(|run| run.text.as_str()).collect();
        }
    }
    panic!("expected a paragraph node");
}

fn utf16le_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

fn utf16be_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFE, 0xFF];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    bytes
}

fn utf32le(text: &str, with_bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if with_bom {
        bytes.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x00]);
    }
    for character in text.chars() {
        bytes.extend_from_slice(&(character as u32).to_le_bytes());
    }
    bytes
}

fn utf32be(text: &str, with_bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if with_bom {
        bytes.extend_from_slice(&[0x00, 0x00, 0xFE, 0xFF]);
    }
    for character in text.chars() {
        bytes.extend_from_slice(&(character as u32).to_be_bytes());
    }
    bytes
}

#[test]
fn utf8_bom_is_stripped_and_body_decodes() {
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice("<p>Héllo</p>".as_bytes());

    let document = parse_html(&bytes, None).expect("UTF-8 with a BOM must parse");

    assert_eq!(first_paragraph_text(&document), "Héllo");
    assert_eq!(document.encoding().active().name(), "UTF-8");
}

#[test]
fn utf16_le_document_with_a_bom_decodes_correctly() {
    let bytes = utf16le_with_bom("<p>Café</p>");

    let document = parse_html(&bytes, None).expect("UTF-16 LE with a BOM must parse");

    assert_eq!(first_paragraph_text(&document), "Café");
    assert_eq!(document.encoding().active().name(), "UTF-16LE");
}

#[test]
fn utf16_be_document_with_a_bom_decodes_correctly() {
    let bytes = utf16be_with_bom("<p>Café</p>");

    let document = parse_html(&bytes, None).expect("UTF-16 BE with a BOM must parse");

    assert_eq!(first_paragraph_text(&document), "Café");
    assert_eq!(document.encoding().active().name(), "UTF-16BE");
}

#[test]
fn utf32_le_document_with_a_bom_decodes_correctly() {
    let bytes = utf32le("<p>Café</p>", true);

    let document = parse_html(&bytes, None).expect("UTF-32 LE with a BOM must parse");

    assert_eq!(first_paragraph_text(&document), "Café");
    assert_eq!(document.encoding().active().name(), "UTF-32LE");
}

#[test]
fn utf32_be_document_with_a_bom_decodes_correctly() {
    let bytes = utf32be("<p>Café</p>", true);

    let document = parse_html(&bytes, None).expect("UTF-32 BE with a BOM must parse");

    assert_eq!(first_paragraph_text(&document), "Café");
    assert_eq!(document.encoding().active().name(), "UTF-32BE");
}

#[test]
fn utf32_le_without_a_bom_decodes_through_the_charset_hint() {
    let bytes = utf32le("<p>Zürich</p>", false);

    let document = parse_html(&bytes, Some("utf-32le")).expect("UTF-32 LE via hint must parse");

    assert_eq!(first_paragraph_text(&document), "Zürich");
    assert_eq!(document.encoding().active().name(), "UTF-32LE");
}

#[test]
fn header_charset_hint_decodes_windows_1252() {
    // 0xE9 is `é` in windows-1252 but not valid stand-alone UTF-8.
    let mut bytes = b"<p>caf".to_vec();
    bytes.push(0xE9);
    bytes.extend_from_slice(b"</p>");

    let document =
        parse_html(&bytes, Some("windows-1252")).expect("windows-1252 via hint must parse");

    assert_eq!(first_paragraph_text(&document), "café");
    assert_eq!(document.encoding().active().name(), "windows-1252");
}

#[test]
fn meta_charset_decodes_when_no_bom_and_no_header() {
    let mut bytes = br#"<meta charset="windows-1252"><p>caf"#.to_vec();
    bytes.push(0xE9);
    bytes.extend_from_slice(b"</p>");

    let document = parse_html(&bytes, None).expect("a meta charset document must parse");

    assert_eq!(first_paragraph_text(&document), "café");
    assert_eq!(document.encoding().active().name(), "windows-1252");
}

#[test]
fn header_charset_overrides_a_conflicting_later_meta() {
    // The body is UTF-8, but declares windows-1252 in a meta tag. The header hint (UTF-8)
    // must win, so the multi-byte `é` decodes as one character rather than two.
    let bytes = r#"<meta charset="windows-1252"><p>café</p>"#.as_bytes();

    let document = parse_html(bytes, Some("utf-8")).expect("header-hinted UTF-8 must parse");

    assert_eq!(first_paragraph_text(&document), "café");
    assert_eq!(document.encoding().active().name(), "UTF-8");
}

#[test]
fn a_byte_order_mark_overrides_the_charset_hint() {
    let bytes = utf16le_with_bom("<p>Hi</p>");

    let document = parse_html(&bytes, Some("windows-1252")).expect("a BOM document must parse");

    assert_eq!(first_paragraph_text(&document), "Hi");
    assert_eq!(document.encoding().active().name(), "UTF-16LE");
}

#[test]
fn invalid_utf8_bytes_become_replacement_characters() {
    let mut bytes = b"<p>a".to_vec();
    bytes.push(0xFF);
    bytes.extend_from_slice(b"b</p>");

    let document = parse_html(&bytes, None).expect("invalid UTF-8 must parse, not panic");

    let text = first_paragraph_text(&document);
    assert!(
        text.contains('\u{FFFD}'),
        "an invalid byte must become U+FFFD"
    );
    assert!(text.contains('a') && text.contains('b'));
}

#[test]
fn a_truncated_utf32_stream_does_not_panic() {
    let mut bytes = utf32le("<p>Hi</p>", false);
    // A trailing group of fewer than four bytes must decode to a replacement, not panic.
    bytes.extend_from_slice(&[0x41, 0x00]);

    let document = parse_html(&bytes, Some("utf-32le")).expect("a truncated stream must parse");

    assert_eq!(first_paragraph_text(&document), "Hi");
}

#[test]
fn a_meta_charset_past_the_prescan_limit_is_not_honored() {
    // Push the meta declaration past the 1 KB pre-scan window with a leading comment, so
    // detection cannot see it and falls back to UTF-8; the windows-1252 byte then decodes
    // to a replacement character rather than to `é`.
    let mut bytes = b"<!-- ".to_vec();
    bytes.extend(std::iter::repeat_n(b'.', 1200));
    bytes.extend_from_slice(b" -->");
    bytes.extend_from_slice(br#"<meta charset="windows-1252"><p>caf"#);
    bytes.push(0xE9);
    bytes.extend_from_slice(b"</p>");

    let document = parse_html(&bytes, None).expect("the document must still parse");

    assert_eq!(document.encoding().active().name(), "UTF-8");
    assert!(first_paragraph_text(&document).contains('\u{FFFD}'));
}

#[test]
fn a_plain_document_reports_utf8_as_detected_and_active() {
    let document = parse_html(b"<p>plain</p>", None).expect("a plain document must parse");

    assert_eq!(document.encoding().detected().name(), "UTF-8");
    assert_eq!(document.encoding().active().name(), "UTF-8");
}
