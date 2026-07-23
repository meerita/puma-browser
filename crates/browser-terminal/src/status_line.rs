// @file crates/browser-terminal/src/status_line.rs
// @description Composes the bottom status-line text from sanitized page and viewport state.
// @layer terminal
// @created meerita <meerita@icloud.com>

/// Separates the status-line segments.
const SEGMENT_SEPARATOR: &str = " | ";

/// Builds the status-line text shown on the bottom row.
///
/// `label` is the already-sanitized page title, URL, or state word. The blocked-script
/// segment appears only when scripts were suppressed, and the arm hint replaces the
/// plain quit hint while the quit is armed.
pub(crate) fn compose_status_line(
    label: &str,
    scroll_percent: u16,
    script_count: usize,
    quit_armed: bool,
) -> String {
    let mut segments = vec![label.to_string(), format!("{scroll_percent}%")];
    if script_count > 0 {
        segments.push(blocked_scripts_segment(script_count));
    }
    segments.push(quit_segment(quit_armed));
    segments.join(SEGMENT_SEPARATOR)
}

fn blocked_scripts_segment(script_count: usize) -> String {
    if script_count == 1 {
        return "1 script blocked".to_string();
    }
    format!("{script_count} scripts blocked")
}

fn quit_segment(quit_armed: bool) -> String {
    if quit_armed {
        return "Press Esc again to quit".to_string();
    }
    "Esc Esc to quit".to_string()
}

#[cfg(test)]
#[path = "status_line_tests.rs"]
mod tests;
