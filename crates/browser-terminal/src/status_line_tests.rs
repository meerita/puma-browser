// @file crates/browser-terminal/src/status_line_tests.rs
// @description Unit tests for the status-line composer: arm hint and script-count segments.
// @layer terminal
// @created meerita <meerita@icloud.com>

use super::compose_status_line;

#[test]
fn the_status_line_shows_the_label_and_scroll_percentage() {
    let line = compose_status_line("example.com", 42, 0, false);
    assert!(line.contains("example.com"));
    assert!(line.contains("42%"));
}

#[test]
fn the_arm_hint_appears_only_when_the_quit_is_armed() {
    let armed = compose_status_line("example.com", 0, 0, true);
    assert!(armed.contains("Press Esc again to quit"));

    let disarmed = compose_status_line("example.com", 0, 0, false);
    assert!(!disarmed.contains("Press Esc again to quit"));
    assert!(disarmed.contains("Esc Esc to quit"));
}

#[test]
fn the_script_count_appears_only_when_non_zero() {
    let without_scripts = compose_status_line("example.com", 0, 0, false);
    assert!(!without_scripts.contains("blocked"));

    let with_scripts = compose_status_line("example.com", 0, 3, false);
    assert!(with_scripts.contains("3 scripts blocked"));
}

#[test]
fn a_single_blocked_script_uses_the_singular_form() {
    let line = compose_status_line("example.com", 0, 1, false);
    assert!(line.contains("1 script blocked"));
    assert!(!line.contains("1 scripts"));
}
