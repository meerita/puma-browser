// @file crates/browser-terminal/src/clipboard.rs
// @description System clipboard writes: native via arboard, OSC 52 fallback for remote sessions.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::io::Write;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;

/// Upper bound on the UTF-8 byte length that may be sent through an OSC 52
/// sequence. Terminals silently drop or mishandle very large OSC payloads, so
/// oversized selections skip the fallback rather than emit a truncated write.
const MAX_OSC52_BYTES: usize = 100_000;

/// Result of a clipboard write, reported back to the caller for its
/// confirmation message. Carries no copied text.
pub(crate) enum ClipboardOutcome {
    CopiedNative,
    CopiedOsc52,
    Failed,
}

/// Copy `text` to the system clipboard. Tries the native clipboard first; on
/// failure, or when `force_osc52` is set, falls back to an OSC 52 write so copy
/// works over SSH and on terminals without a native clipboard path.
///
/// Only ever called from a user mouse gesture. The copied text is never logged,
/// stored, or formatted for `Debug`.
pub(crate) fn copy_to_clipboard(text: &str, force_osc52: bool) -> ClipboardOutcome {
    if !force_osc52 && copy_to_native_clipboard(text) {
        return ClipboardOutcome::CopiedNative;
    }
    if !text_fits_osc52_limit(text) {
        return ClipboardOutcome::Failed;
    }
    copy_via_osc52(text)
}

/// True when the UTF-8 payload is small enough to send through OSC 52.
fn text_fits_osc52_limit(text: &str) -> bool {
    text.len() <= MAX_OSC52_BYTES
}

/// Attempt a native clipboard write. Returns whether the write succeeded; any
/// error is swallowed so the caller can fall back to OSC 52.
fn copy_to_native_clipboard(text: &str) -> bool {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        return false;
    };
    clipboard.set_text(text).is_ok()
}

/// Emit an OSC 52 sequence carrying the base64-encoded selection to stdout.
fn copy_via_osc52(text: &str) -> ClipboardOutcome {
    let sequence = build_osc52_sequence(text);
    let mut stdout = std::io::stdout();
    if stdout.write_all(sequence.as_bytes()).is_err() {
        return ClipboardOutcome::Failed;
    }
    if stdout.flush().is_err() {
        return ClipboardOutcome::Failed;
    }
    ClipboardOutcome::CopiedOsc52
}

/// Build the `ESC ] 52 ; c ; <base64> BEL` clipboard-write sequence. The payload
/// is base64 (standard alphabet, no line wraps), so page text can never inject
/// raw control bytes into the terminal.
pub(crate) fn build_osc52_sequence(text: &str) -> String {
    let encoded = STANDARD.encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x07")
}

#[cfg(test)]
#[path = "clipboard_tests.rs"]
mod tests;
