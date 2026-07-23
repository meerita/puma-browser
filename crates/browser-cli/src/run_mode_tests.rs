// @file crates/browser-cli/src/run_mode_tests.rs
// @description Verifies the run-mode argument parser selects the terminal and MCP adapters.
// @layer cli
// @created meerita <meerita@icloud.com>

use super::{parse_run_mode, RunMode};

fn run_mode_for(arguments: &[&str]) -> RunMode {
    parse_run_mode(arguments.iter().map(|argument| argument.to_string()))
}

#[test]
fn no_arguments_selects_terminal_mode() {
    assert_eq!(run_mode_for(&[]), RunMode::Terminal);
}

#[test]
fn terminal_argument_selects_terminal_mode() {
    assert_eq!(run_mode_for(&["terminal"]), RunMode::Terminal);
}

#[test]
fn mcp_argument_selects_mcp_mode() {
    assert_eq!(run_mode_for(&["mcp"]), RunMode::Mcp);
}

#[test]
fn unknown_argument_falls_back_to_terminal_mode() {
    assert_eq!(run_mode_for(&["--nonsense"]), RunMode::Terminal);
}

#[test]
fn known_mode_after_unknown_argument_is_selected() {
    assert_eq!(run_mode_for(&["--nonsense", "mcp"]), RunMode::Mcp);
}
