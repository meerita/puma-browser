// @file crates/browser-html/src/encoding.rs
// @description Detects and decodes document bytes into Unicode at the HTML parse boundary.
// @layer html
// @created meerita <meerita@icloud.com>

/// How many leading bytes the `<meta charset>` pre-scan inspects.
///
/// The scan reads only this many bytes so a huge document cannot drive an unbounded
/// search for a charset declaration; a `<meta>` past this point is not honored.
const META_PRESCAN_LIMIT: usize = 1024;

/// A text encoding the parser can decode document bytes with.
///
/// Most encodings are delegated to `encoding_rs`; UTF-32 LE and BE are decoded by hand
/// because `encoding_rs` does not cover them. The inner kind is private so the
/// `encoding_rs` type never leaks across the crate boundary; callers read the label
/// through [`Encoding::name`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encoding(EncodingKind);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EncodingKind {
    Standard(&'static encoding_rs::Encoding),
    Utf32Le,
    Utf32Be,
}

impl Encoding {
    /// UTF-8, the conservative fallback when no encoding can be detected.
    pub fn utf8() -> Encoding {
        Encoding(EncodingKind::Standard(encoding_rs::UTF_8))
    }

    fn utf16le() -> Encoding {
        Encoding(EncodingKind::Standard(encoding_rs::UTF_16LE))
    }

    fn utf16be() -> Encoding {
        Encoding(EncodingKind::Standard(encoding_rs::UTF_16BE))
    }

    /// Resolve an encoding from a charset label, or `None` when the label is unknown.
    ///
    /// UTF-32 labels are matched by hand because `encoding_rs` does not support them; any
    /// other label is resolved through `encoding_rs`, which matches labels
    /// case-insensitively. An unknown or hostile label yields `None`, so detection falls
    /// through to the next step rather than trusting it.
    pub fn from_label(label: &str) -> Option<Encoding> {
        let trimmed = label.trim();
        if let Some(utf32) = utf32_from_label(trimmed) {
            return Some(utf32);
        }
        encoding_rs::Encoding::for_label(trimmed.as_bytes())
            .map(|inner| Encoding(EncodingKind::Standard(inner)))
    }

    /// The canonical label of this encoding, for document inspection.
    pub fn name(&self) -> &'static str {
        match self.0 {
            EncodingKind::Standard(inner) => inner.name(),
            EncodingKind::Utf32Le => "UTF-32LE",
            EncodingKind::Utf32Be => "UTF-32BE",
        }
    }
}

/// The encoding detected for a document and the encoding actually used to decode it.
///
/// The two coincide in this build because no post-detection override exists yet. The
/// type carries both so the document-inspection surface can report the declared and the
/// active encoding separately once an encoding override is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectedEncoding {
    detected: Encoding,
    active: Encoding,
}

impl DetectedEncoding {
    pub(crate) fn new(detected: Encoding, active: Encoding) -> DetectedEncoding {
        DetectedEncoding { detected, active }
    }

    /// The encoding detection resolved from the byte-order mark, header, or metadata.
    pub fn detected(&self) -> Encoding {
        self.detected
    }

    /// The encoding actually used to decode the document bytes.
    pub fn active(&self) -> Encoding {
        self.active
    }
}

/// Detect a document's encoding and the length of any leading byte-order mark to skip.
///
/// Detection order is byte-order mark, then the `Content-Type` charset hint, then a
/// `<meta charset>` found by a bounded pre-scan, then a UTF-8 fallback. Only the
/// byte-order-mark path reports a non-zero mark length; the other paths carry no mark.
pub(crate) fn detect_encoding(bytes: &[u8], charset_hint: Option<&str>) -> (Encoding, usize) {
    if let Some(detected) = detect_bom(bytes) {
        return detected;
    }
    if let Some(encoding) = charset_hint.and_then(Encoding::from_label) {
        return (encoding, 0);
    }
    if let Some(encoding) = scan_meta_charset(bytes) {
        return (encoding, 0);
    }
    (Encoding::utf8(), 0)
}

/// Decode document bytes with the detected encoding, skipping a leading byte-order mark.
///
/// Malformed bytes are replaced with the Unicode replacement character rather than
/// causing an error, so no byte sequence can make decoding fail or panic.
pub(crate) fn decode(bytes: &[u8], encoding: Encoding, mark_length: usize) -> String {
    let body = &bytes[mark_length.min(bytes.len())..];
    match encoding.0 {
        EncodingKind::Standard(inner) => decode_standard(inner, body),
        EncodingKind::Utf32Le => decode_utf32(body, Endianness::Little),
        EncodingKind::Utf32Be => decode_utf32(body, Endianness::Big),
    }
}

fn decode_standard(encoding: &'static encoding_rs::Encoding, body: &[u8]) -> String {
    let (decoded, _had_errors) = encoding.decode_without_bom_handling(body);
    decoded.into_owned()
}

/// Which byte order a UTF-32 stream uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endianness {
    Little,
    Big,
}

/// Decode a UTF-32 byte stream by hand, four bytes per scalar value.
///
/// A trailing group of fewer than four bytes and any code point outside the Unicode
/// scalar range both decode to the replacement character, so a truncated or invalid
/// stream never fails.
fn decode_utf32(body: &[u8], endianness: Endianness) -> String {
    body.chunks(4)
        .map(|group| decode_utf32_scalar(group, endianness))
        .collect()
}

fn decode_utf32_scalar(group: &[u8], endianness: Endianness) -> char {
    let Ok(quad) = <[u8; 4]>::try_from(group) else {
        return char::REPLACEMENT_CHARACTER;
    };
    let code_point = match endianness {
        Endianness::Little => u32::from_le_bytes(quad),
        Endianness::Big => u32::from_be_bytes(quad),
    };
    char::from_u32(code_point).unwrap_or(char::REPLACEMENT_CHARACTER)
}

/// Match a UTF-32 charset label, which `encoding_rs` does not recognize.
fn utf32_from_label(label: &str) -> Option<Encoding> {
    if label.eq_ignore_ascii_case("utf-32le") || label.eq_ignore_ascii_case("utf-32-le") {
        return Some(Encoding(EncodingKind::Utf32Le));
    }
    if label.eq_ignore_ascii_case("utf-32be") || label.eq_ignore_ascii_case("utf-32-be") {
        return Some(Encoding(EncodingKind::Utf32Be));
    }
    None
}

/// Detect an encoding from a leading byte-order mark, returning the mark's byte length.
///
/// The UTF-32 marks are tested before the UTF-16 marks because the UTF-32 LE mark
/// (`FF FE 00 00`) begins with the UTF-16 LE mark (`FF FE`); testing UTF-16 first would
/// misread a UTF-32 LE document.
fn detect_bom(bytes: &[u8]) -> Option<(Encoding, usize)> {
    if bytes.starts_with(&[0x00, 0x00, 0xFE, 0xFF]) {
        return Some((Encoding(EncodingKind::Utf32Be), 4));
    }
    if bytes.starts_with(&[0xFF, 0xFE, 0x00, 0x00]) {
        return Some((Encoding(EncodingKind::Utf32Le), 4));
    }
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Some((Encoding::utf8(), 3));
    }
    if bytes.starts_with(&[0xFE, 0xFF]) {
        return Some((Encoding::utf16be(), 2));
    }
    if bytes.starts_with(&[0xFF, 0xFE]) {
        return Some((Encoding::utf16le(), 2));
    }
    None
}

/// Scan the leading bytes for a `<meta charset>` declaration.
///
/// The scan reads at most [`META_PRESCAN_LIMIT`] bytes, treating them as ASCII so the
/// document is never decoded twice. The first `<meta>` carrying a recognized charset
/// wins; a declaration past the limit is not seen.
fn scan_meta_charset(bytes: &[u8]) -> Option<Encoding> {
    let limit = bytes.len().min(META_PRESCAN_LIMIT);
    let window = ascii_lowercase_window(&bytes[..limit]);
    let mut cursor = 0;
    while let Some(relative) = window[cursor..].find("<meta") {
        let meta_start = cursor + relative;
        let tag_end = tag_end_from(&window, meta_start);
        if let Some(encoding) = meta_tag_encoding(&window[meta_start..tag_end]) {
            return Some(encoding);
        }
        cursor = tag_end.max(meta_start + 1);
    }
    None
}

/// Resolve the encoding declared by a single `<meta …>` tag, if any.
fn meta_tag_encoding(tag: &str) -> Option<Encoding> {
    let label = charset_label(tag)?;
    Encoding::from_label(&label)
}

/// Extract the value of a `charset` attribute from a lowercased `<meta>` tag.
///
/// Both `<meta charset=…>` and `<meta http-equiv content="…charset=…">` carry the label
/// after a literal `charset=`, so the value is read from the first such occurrence up to
/// the next delimiter.
fn charset_label(tag: &str) -> Option<String> {
    let index = tag.find("charset")?;
    let after = tag[index + "charset".len()..].trim_start();
    let value = after
        .strip_prefix('=')?
        .trim_start()
        .trim_start_matches(['"', '\'']);
    let label: String = value
        .chars()
        .take_while(|byte| is_label_char(*byte))
        .collect();
    if label.is_empty() {
        return None;
    }
    Some(label)
}

fn is_label_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | ':' | '.')
}

/// The byte offset of the `>` closing a tag opened at `start`, or the window end.
fn tag_end_from(window: &str, start: usize) -> usize {
    match window[start..].find('>') {
        Some(relative) => start + relative,
        None => window.len(),
    }
}

/// Copy a byte window into an all-ASCII, lowercased string for structural scanning.
///
/// Every byte maps to exactly one ASCII character so byte offsets from `str::find` stay
/// valid: ASCII bytes are lowercased and any other byte becomes a space, which cannot be
/// mistaken for markup or a charset label.
fn ascii_lowercase_window(window: &[u8]) -> String {
    window.iter().map(|byte| ascii_scan_char(*byte)).collect()
}

fn ascii_scan_char(byte: u8) -> char {
    if byte.is_ascii() {
        return byte.to_ascii_lowercase() as char;
    }
    ' '
}
