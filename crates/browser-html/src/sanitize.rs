// @file crates/browser-html/src/sanitize.rs
// @description Control-character stripping for remote text entering nodes and the title.
// @layer html
// @created meerita <meerita@icloud.com>

/// Remove every control character from remote text.
///
/// Remote content can embed terminal escape sequences (`\x1b`), carriage returns, and
/// NUL bytes. Deleting all control characters at this boundary guarantees none can
/// travel further into the pipeline and reach the terminal as raw bytes.
pub(crate) fn strip_control_characters(raw: &str) -> String {
    raw.chars()
        .filter(|character| !character.is_control())
        .collect()
}

/// Remove control characters while keeping newline and tab.
///
/// Preformatted and code blocks carry meaning in their newlines and tabs, so those two
/// survive; every other control character, including `\x1b`, carriage return, and NUL,
/// is still removed so no escape sequence can reach the terminal later.
pub(crate) fn strip_control_characters_preserving_layout(raw: &str) -> String {
    raw.chars()
        .filter(|character| !is_removable_control(*character))
        .collect()
}

/// Collapse each run of whitespace into a single space and trim both ends.
pub(crate) fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn is_removable_control(character: char) -> bool {
    character.is_control() && character != '\n' && character != '\t'
}
