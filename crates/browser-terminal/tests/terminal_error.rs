// @file crates/browser-terminal/tests/terminal_error.rs
// @description Verifies CoreError mapping and that user_message stays short and leaks no internals.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_core::CoreError;
use browser_terminal::TerminalError;

#[test]
fn core_error_maps_into_terminal_error() {
    let mapped: TerminalError = CoreError::NavigationFailed.into();
    assert!(matches!(mapped, TerminalError::Core(_)));
}

#[test]
fn user_message_is_short_and_contains_no_paths_or_crate_names() {
    let messages = [
        TerminalError::from(CoreError::NavigationFailed).user_message(),
        TerminalError::from(CoreError::TabNotFound).user_message(),
        TerminalError::RenderFailed.user_message(),
    ];

    for message in messages {
        assert!(!message.is_empty(), "message must not be empty");
        assert!(message.len() <= 80, "message must be short: {message:?}");
        assert!(
            !message.contains('/'),
            "message must not leak a path: {message:?}"
        );
        assert!(
            !message.contains("::"),
            "message must not leak a path: {message:?}"
        );
        assert!(
            !message.to_lowercase().contains("browser"),
            "message must not leak a crate name: {message:?}"
        );
        assert!(
            !message.contains("Error"),
            "message must not leak an internal variant name: {message:?}"
        );
        assert!(
            !message.chars().any(|character| character.is_control()),
            "message must not carry a control or escape character: {message:?}"
        );
    }
}

#[test]
fn render_failed_message_is_user_facing() {
    let message = TerminalError::RenderFailed.user_message();
    assert_eq!(message, "Could not render the page");
}

#[test]
fn local_file_error_variants_yield_their_exact_messages() {
    let cases = [
        (CoreError::LocalFileNotFound, "File not found"),
        (
            CoreError::LocalPathIsDirectory,
            "Not a file (it is a directory)",
        ),
        (CoreError::LocalFileTooLarge, "File too large"),
        (CoreError::LocalFileReadFailed, "Could not read the file"),
    ];

    for (error, expected) in cases {
        assert_eq!(TerminalError::from(error).user_message(), expected);
    }
}

#[test]
fn local_file_error_messages_leak_no_path_or_crate_name() {
    let messages = [
        TerminalError::from(CoreError::LocalFileNotFound).user_message(),
        TerminalError::from(CoreError::LocalPathIsDirectory).user_message(),
        TerminalError::from(CoreError::LocalFileTooLarge).user_message(),
        TerminalError::from(CoreError::LocalFileReadFailed).user_message(),
    ];

    for message in messages {
        assert!(!message.is_empty(), "message must not be empty");
        assert!(message.len() <= 80, "message must be short: {message:?}");
        assert!(
            !message.contains('/'),
            "message must not leak a path: {message:?}"
        );
        assert!(
            !message.contains("::"),
            "message must not leak a path: {message:?}"
        );
        assert!(
            !message.to_lowercase().contains("browser"),
            "message must not leak a crate name: {message:?}"
        );
        assert!(
            !message.chars().any(|character| character.is_control()),
            "message must not carry a control or escape character: {message:?}"
        );
    }
}
