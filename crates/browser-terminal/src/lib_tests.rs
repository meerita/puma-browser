// @file crates/browser-terminal/src/lib_tests.rs
// @description Tests for mouse gesture dispatch, coordinate mapping, and selection highlight.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::{
    action_refreshes_suggestions, advance_interactive_focus, build_settings_model,
    cell_is_selected, clamped_document_coordinate, copied_message, decode_fragment,
    document_coordinate, handle_mouse_event, handle_navigation_action, max_scroll_offset,
    parse_history_request, resolve_anchor_row, retreat_interactive_focus,
    sanitize_fragment_for_display, unique_interactive_targets, CachedPage, CommandOutcome,
    EnvOverrides, HistoryRequest, InputAction, InteractiveTarget, LoadState, NavigationRequest,
    ScrollState, SettingsModel, SettingsMutation, TerminalApp, TerminalSettings, TextSelection,
    UiState, ViewState, ViewportBounds, BODY_AREA_TOP_ROW, CONTENT_PADDING, SETTINGS_ENV_LOCKED,
    SETTINGS_TEXT_INVALID,
};
use crate::settings_view::SettingId;
use crate::ui_state::{SubmitChoice, SETTINGS_AUTOSAVE_DEBOUNCE};
use browser_core::{
    resolve_address, CookiePolicy, CookiePolicyPair, HistoryMode, HistorySettings, HistoryStore,
    NavigationController, NavigationSource, NodeId, SearchEngine, SitePolicyStore, SuggestionEntry,
};

use crate::command::CookiesRequest;
use browser_html::{Document, InlineEmphasis, InlineRun, SemanticNode};
use browser_layout::{
    render_document, AnchorSpan, CellBuffer, CellPosition, FieldSpanKind, WidthConfig,
};
use browser_storage::{ConfigStore, HistoryEntry, NewVisit, StorageError};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use tempfile::tempdir;

fn citation_run(text: &str, cite_url: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: None,
        citation: Some(cite_url.to_string()),
        anchors: Vec::new(),
    }
}

fn link_run(text: &str, url: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: Some(url.to_string()),
        citation: None,
        anchors: Vec::new(),
    }
}

fn link_and_citation_run(text: &str, url: &str, cite_url: &str) -> InlineRun {
    InlineRun {
        text: text.to_string(),
        emphasis: InlineEmphasis::none(),
        link: Some(url.to_string()),
        citation: Some(cite_url.to_string()),
        anchors: Vec::new(),
    }
}

/// A one-row cached page holding a single run, laid out at width 40 so the run starts at
/// column 0, row 0.
fn cache_with_run(run: InlineRun) -> CachedPage {
    let document = Document::new(
        vec![SemanticNode::Paragraph {
            runs: vec![run],
            inline_style: None,
        }],
        None,
        0,
    );
    let buffer = render_document(&document, 40, &WidthConfig::default(), None)
        .expect("a single-run paragraph must lay out for the citation test");
    CachedPage { width: 40, buffer }
}

fn anchor(name: &str, row: u16) -> AnchorSpan {
    AnchorSpan {
        name: name.to_string(),
        row,
    }
}

/// A cached page whose second paragraph carries `anchor_name`, so the anchor sits on a
/// non-zero row and a jump to it is observable as a changed scroll offset.
fn cache_with_anchor(anchor_name: &str) -> CachedPage {
    let first = SemanticNode::Paragraph {
        runs: vec![InlineRun::plain("First paragraph".to_string())],
        inline_style: None,
    };
    let target = SemanticNode::Paragraph {
        runs: vec![InlineRun {
            text: "Target paragraph".to_string(),
            emphasis: InlineEmphasis::none(),
            link: None,
            citation: None,
            anchors: vec![anchor_name.to_string()],
        }],
        inline_style: None,
    };
    let document = Document::new(vec![first, target], None, 0);
    let buffer = render_document(&document, 40, &WidthConfig::default(), None)
        .expect("document must lay out for the anchor test");
    CachedPage { width: 40, buffer }
}

fn terminal_app() -> TerminalApp {
    terminal_app_with_search(true)
}

fn terminal_app_with_search(search_enabled: bool) -> TerminalApp {
    TerminalApp::new(
        NavigationController::new(),
        ViewState::Page,
        TerminalSettings {
            copy_on_select: false,
            force_osc52: false,
            search_enabled,
            unwrap_tracking: true,
            env_overridden: EnvOverrides::default(),
        },
    )
}

fn terminal_app_with_unwrap_tracking(unwrap_tracking: bool) -> TerminalApp {
    TerminalApp::new(
        NavigationController::new(),
        ViewState::Page,
        TerminalSettings {
            copy_on_select: false,
            force_osc52: false,
            search_enabled: true,
            unwrap_tracking,
            env_overridden: EnvOverrides::default(),
        },
    )
}

#[test]
fn an_enabled_unwrap_tracking_setting_maps_to_the_enabled_mode() {
    let app = terminal_app_with_unwrap_tracking(true);

    assert_eq!(
        app.tracking_unwrap_mode(),
        browser_core::TrackingUnwrap::Enabled
    );
}

#[test]
fn a_disabled_unwrap_tracking_setting_maps_to_the_disabled_mode() {
    let app = terminal_app_with_unwrap_tracking(false);

    assert_eq!(
        app.tracking_unwrap_mode(),
        browser_core::TrackingUnwrap::Disabled
    );
}

#[test]
fn resolve_anchor_row_sends_an_empty_fragment_to_the_top() {
    assert_eq!(resolve_anchor_row(Some(""), &[]), Some(0));
    assert_eq!(resolve_anchor_row(None, &[]), Some(0));
}

#[test]
fn resolve_anchor_row_sends_top_to_the_top_regardless_of_case() {
    let anchors = [anchor("body", 9)];
    assert_eq!(resolve_anchor_row(Some("top"), &anchors), Some(0));
    assert_eq!(resolve_anchor_row(Some("TOP"), &anchors), Some(0));
}

#[test]
fn resolve_anchor_row_matches_an_exact_name() {
    let anchors = [anchor("intro", 3), anchor("details", 8)];
    assert_eq!(resolve_anchor_row(Some("details"), &anchors), Some(8));
}

#[test]
fn resolve_anchor_row_falls_back_to_a_case_insensitive_match() {
    let anchors = [anchor("Details", 8)];
    assert_eq!(resolve_anchor_row(Some("details"), &anchors), Some(8));
}

#[test]
fn resolve_anchor_row_prefers_an_exact_match_over_a_case_insensitive_one() {
    let anchors = [anchor("Section", 2), anchor("section", 6)];
    assert_eq!(resolve_anchor_row(Some("section"), &anchors), Some(6));
}

#[test]
fn resolve_anchor_row_returns_the_first_anchor_of_a_shared_name() {
    let anchors = [anchor("dup", 4), anchor("dup", 11)];
    assert_eq!(resolve_anchor_row(Some("dup"), &anchors), Some(4));
}

#[test]
fn resolve_anchor_row_reports_no_row_for_an_unknown_fragment() {
    let anchors = [anchor("intro", 3)];
    assert_eq!(resolve_anchor_row(Some("missing"), &anchors), None);
}

#[test]
fn resolve_anchor_row_decodes_a_percent_encoded_fragment() {
    let anchors = [anchor("a b", 5)];
    assert_eq!(resolve_anchor_row(Some("a%20b"), &anchors), Some(5));
}

#[test]
fn decode_fragment_of_none_is_empty() {
    assert_eq!(decode_fragment(None), "");
}

#[test]
fn sanitize_fragment_for_display_strips_control_characters() {
    assert_eq!(sanitize_fragment_for_display("a\u{1b}b"), "ab");
}

#[test]
fn a_same_page_jump_to_a_present_anchor_moves_the_viewport_there() {
    let application = terminal_app();
    let cache = Some(cache_with_anchor("target"));
    let target_row = cache.as_ref().unwrap().buffer.anchors()[0].row;
    assert!(
        target_row > 0,
        "the anchor must sit below the top of the page"
    );
    let mut scroll = ScrollState::new();
    let mut ui_state = UiState::new(true);

    application.jump_to_anchor(
        Some("target"),
        &cache,
        &mut scroll,
        80,
        &mut ui_state,
        Instant::now(),
    );

    assert_eq!(scroll.offset(), target_row);
    assert_eq!(ui_state.transient_message(), None);
}

#[test]
fn a_same_page_jump_to_a_missing_anchor_reports_it_and_does_not_move() {
    let application = terminal_app();
    let cache = Some(cache_with_anchor("target"));
    let mut scroll = ScrollState::new();
    let mut ui_state = UiState::new(true);

    application.jump_to_anchor(
        Some("absent"),
        &cache,
        &mut scroll,
        80,
        &mut ui_state,
        Instant::now(),
    );

    assert_eq!(scroll.offset(), 0);
    assert_eq!(
        ui_state.transient_message(),
        Some("anchor not found: absent")
    );
}

#[test]
fn a_pending_fragment_is_applied_and_cleared_after_the_page_renders() {
    let application = terminal_app();
    let cache = Some(cache_with_anchor("target"));
    let target_row = cache.as_ref().unwrap().buffer.anchors()[0].row;
    let mut scroll = ScrollState::new();
    let mut ui_state = UiState::new(true);
    ui_state.set_pending_fragment(Some("target".to_string()));

    application.apply_pending_fragment(&mut ui_state, &cache, &mut scroll, 80);

    assert_eq!(scroll.offset(), target_row);
    assert!(!ui_state.has_pending_fragment());
}

#[test]
fn apply_pending_fragment_does_nothing_without_a_pending_fragment() {
    let application = terminal_app();
    let cache = Some(cache_with_anchor("target"));
    let mut scroll = ScrollState::new();
    let mut ui_state = UiState::new(true);

    application.apply_pending_fragment(&mut ui_state, &cache, &mut scroll, 80);

    assert_eq!(scroll.offset(), 0);
    assert!(!ui_state.has_pending_fragment());
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn position(column: u16, row: u16) -> CellPosition {
    CellPosition { column, row }
}

#[test]
fn document_coordinate_shifts_by_padding_and_adds_scroll() {
    let event = mouse(MouseEventKind::Moved, 5, 3);
    let coordinate = document_coordinate(&event, 4, BODY_AREA_TOP_ROW);
    assert_eq!(coordinate, Some(position(5 - CONTENT_PADDING, 3 + 4)));
}

#[test]
fn document_coordinate_in_left_padding_maps_to_none() {
    let event = mouse(MouseEventKind::Moved, 1, 3);
    assert_eq!(document_coordinate(&event, 0, BODY_AREA_TOP_ROW), None);
}

#[test]
fn clamped_coordinate_past_content_clamps_to_last_cell() {
    let buffer = CellBuffer::new(10, 6);
    let event = mouse(MouseEventKind::Drag(MouseButton::Left), 100, 100);
    let coordinate = clamped_document_coordinate(&event, &buffer, 0, BODY_AREA_TOP_ROW);
    assert_eq!(coordinate, position(9, 5));
}

#[test]
fn clamped_coordinate_within_content_is_unchanged() {
    let buffer = CellBuffer::new(10, 6);
    let event = mouse(MouseEventKind::Drag(MouseButton::Left), 5, 2);
    let coordinate = clamped_document_coordinate(&event, &buffer, 1, BODY_AREA_TOP_ROW);
    assert_eq!(coordinate, position(5 - CONTENT_PADDING, 3));
}

#[test]
fn clamped_coordinate_on_empty_buffer_does_not_panic() {
    let buffer = CellBuffer::new(0, 0);
    let event = mouse(MouseEventKind::Drag(MouseButton::Left), 50, 50);
    let coordinate = clamped_document_coordinate(&event, &buffer, 0, BODY_AREA_TOP_ROW);
    assert_eq!(coordinate, position(0, 0));
}

#[test]
fn press_begins_a_selection_without_a_range() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    handle_mouse_event(
        mouse(MouseEventKind::Down(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(true),
        Instant::now(),
        true,
        false,
    );
    assert!(selection.is_dragging());
    assert!(!selection.has_moved());
    assert_eq!(selection.range(), None);
    assert_eq!(navigate_to_url, None);
}

#[test]
fn press_drag_release_keeps_the_highlighted_range() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    let steps = [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ];
    let columns = [3, 7, 7];
    for (kind, column) in steps.into_iter().zip(columns) {
        handle_mouse_event(
            mouse(kind, column, 2),
            Some(&buffer),
            0,
            BODY_AREA_TOP_ROW,
            &mut selection,
            &mut navigate_to_url,
            &mut UiState::new(true),
            Instant::now(),
            true,
            false,
        );
    }
    assert!(selection.has_moved());
    assert_eq!(
        selection.range(),
        Some((
            position(3 - CONTENT_PADDING, 2),
            position(7 - CONTENT_PADDING, 2)
        ))
    );
    assert_eq!(navigate_to_url, None);
}

#[test]
fn release_with_copy_disabled_keeps_the_highlight_and_sets_no_message() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    let mut ui_state = UiState::new(true);
    let now = Instant::now();
    let steps = [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ];
    let columns = [3, 7, 7];
    for (kind, column) in steps.into_iter().zip(columns) {
        handle_mouse_event(
            mouse(kind, column, 2),
            Some(&buffer),
            0,
            BODY_AREA_TOP_ROW,
            &mut selection,
            &mut navigate_to_url,
            &mut ui_state,
            now,
            false,
            false,
        );
    }
    // The drag still produced a highlighted range, but with copy disabled the release
    // copies nothing and shows no confirmation.
    assert!(selection.has_moved());
    assert_eq!(
        selection.range(),
        Some((
            position(3 - CONTENT_PADDING, 2),
            position(7 - CONTENT_PADDING, 2)
        ))
    );
    assert_eq!(ui_state.transient_message(), None);
}

#[test]
fn press_release_without_movement_clears_the_selection() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    handle_mouse_event(
        mouse(MouseEventKind::Down(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(true),
        Instant::now(),
        true,
        false,
    );
    handle_mouse_event(
        mouse(MouseEventKind::Up(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(true),
        Instant::now(),
        true,
        false,
    );
    assert!(!selection.is_dragging());
    assert_eq!(selection.range(), None);
    assert_eq!(navigate_to_url, None);
}

#[test]
fn a_drag_without_a_prior_press_is_ignored() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    handle_mouse_event(
        mouse(MouseEventKind::Drag(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(true),
        Instant::now(),
        true,
        false,
    );
    assert!(!selection.is_dragging());
    assert_eq!(selection.range(), None);
}

#[test]
fn wheel_scroll_events_do_not_touch_the_selection() {
    let buffer = CellBuffer::new(10, 6);
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    handle_mouse_event(
        mouse(MouseEventKind::Down(MouseButton::Left), 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(true),
        Instant::now(),
        true,
        false,
    );
    handle_mouse_event(
        mouse(MouseEventKind::ScrollDown, 5, 2),
        Some(&buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut UiState::new(true),
        Instant::now(),
        true,
        false,
    );
    assert!(selection.is_dragging());
    assert!(!selection.has_moved());
}

#[test]
fn cell_is_selected_covers_the_single_row_span_inclusively() {
    let range = Some((position(2, 1), position(5, 1)));
    assert!(!cell_is_selected(1, 1, range));
    assert!(cell_is_selected(2, 1, range));
    assert!(cell_is_selected(5, 1, range));
    assert!(!cell_is_selected(6, 1, range));
    assert!(!cell_is_selected(3, 0, range));
}

#[test]
fn cell_is_selected_covers_interior_rows_fully_and_ends_partially() {
    let range = Some((position(4, 1), position(3, 3)));
    assert!(!cell_is_selected(3, 1, range));
    assert!(cell_is_selected(4, 1, range));
    assert!(cell_is_selected(0, 2, range));
    assert!(cell_is_selected(9, 2, range));
    assert!(cell_is_selected(3, 3, range));
    assert!(!cell_is_selected(4, 3, range));
}

#[test]
fn cell_is_selected_is_false_without_a_range() {
    assert!(!cell_is_selected(3, 1, None));
}

#[test]
fn copied_message_reports_the_ascii_grapheme_count() {
    assert_eq!(copied_message("hello"), "copied 5 chars to clipboard");
}

/// A cached page of `link_count` single-link paragraphs. Blank lines between paragraphs
/// place the links on evenly spaced rows (0, 2, 4, ...), taller than a small viewport, so
/// some links fall outside a chosen window.
fn cache_with_links(link_count: usize) -> CachedPage {
    let nodes = (0..link_count)
        .map(|number| SemanticNode::Paragraph {
            runs: vec![InlineRun {
                text: format!("Link {number}"),
                emphasis: InlineEmphasis::none(),
                link: Some(format!("https://example.com/{number}")),
                citation: None,
                anchors: Vec::new(),
            }],
            inline_style: None,
        })
        .collect();
    let document = Document::new(nodes, None, 0);
    let buffer = render_document(&document, 40, &WidthConfig::default(), None)
        .expect("linked paragraphs must lay out for the focus test");
    CachedPage { width: 40, buffer }
}

#[test]
fn tabbing_down_onto_a_link_below_the_fold_scrolls_to_reveal_it() {
    let cache = cache_with_links(6);
    let viewport_height = 4;
    let max_offset = max_scroll_offset(cache.buffer.height(), viewport_height);
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();

    // The entry Tab focuses the first visible link (row 0) and does not scroll.
    advance_interactive_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(scroll.offset(), 0);

    // Tabbing down to the fourth link (row 6) scrolls it onto the bottom edge.
    for _ in 0..3 {
        advance_interactive_focus(
            &mut ui_state,
            Some(&cache.buffer),
            &mut scroll,
            ViewportBounds {
                height: viewport_height,
                max_offset,
            },
        );
    }
    assert_eq!(ui_state.focused_interactive_index, Some(3));
    assert_eq!(scroll.offset(), 6 - (viewport_height - 1));
}

#[test]
fn shift_tabbing_up_to_a_link_above_the_window_scrolls_to_reveal_it() {
    let cache = cache_with_links(6);
    let viewport_height = 4;
    let max_offset = max_scroll_offset(cache.buffer.height(), viewport_height);
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    // Focus the fifth link (row 8) with the window scrolled to the bottom.
    ui_state.enter_interactive_navigation(4);
    scroll.scroll_to(8, max_offset);

    // Shift+Tab up to the fourth link (row 6) sits above the window and scrolls up to it.
    retreat_interactive_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(ui_state.focused_interactive_index, Some(3));
    assert_eq!(scroll.offset(), 6);
}

#[test]
fn tabbing_forward_off_the_last_link_wraps_and_scrolls_to_the_top() {
    let cache = cache_with_links(6);
    let viewport_height = 4;
    let max_offset = max_scroll_offset(cache.buffer.height(), viewport_height);
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    ui_state.enter_interactive_navigation(5);
    scroll.scroll_to(8, max_offset);

    advance_interactive_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(ui_state.focused_interactive_index, Some(0));
    assert_eq!(scroll.offset(), 0);
}

#[test]
fn shift_tabbing_backward_off_the_first_link_wraps_and_reveals_the_last() {
    let cache = cache_with_links(6);
    let viewport_height = 4;
    let max_offset = max_scroll_offset(cache.buffer.height(), viewport_height);
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    ui_state.enter_interactive_navigation(0);

    // The last link sits on row 10; the bottom edge lands it at offset 10 - (4 - 1) = 7.
    retreat_interactive_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(ui_state.focused_interactive_index, Some(5));
    assert_eq!(scroll.offset(), 10 - (viewport_height - 1));
}

#[test]
fn tabbing_onto_an_already_visible_link_leaves_the_offset_unchanged() {
    let cache = cache_with_links(6);
    let viewport_height = 6;
    let max_offset = max_scroll_offset(cache.buffer.height(), viewport_height);
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    ui_state.enter_interactive_navigation(0);

    // The second link (row 2) is already inside the window that starts at the top.
    advance_interactive_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(ui_state.focused_interactive_index, Some(1));
    assert_eq!(scroll.offset(), 0);
}

#[test]
fn copied_message_counts_multibyte_characters_as_one_grapheme_each() {
    // "café" is five UTF-8 bytes but four grapheme clusters; a combined-emoji family is
    // many bytes yet a single grapheme. The count must be graphemes, not bytes.
    assert_eq!(copied_message("café"), "copied 4 chars to clipboard");
    assert_eq!(copied_message("👨‍👩‍👧"), "copied 1 chars to clipboard");
}

#[test]
fn copied_message_reports_zero_for_empty_text() {
    assert_eq!(copied_message(""), "copied 0 chars to clipboard");
}

#[test]
fn run_search_is_rejected_with_a_fixed_message_when_search_is_disabled() {
    let mut application = terminal_app_with_search(false);
    let mut ui_state = UiState::new(false);
    let outcome = application.run_search("rust", &mut ui_state, Instant::now());
    assert!(matches!(outcome, CommandOutcome::None));
    assert_eq!(ui_state.transient_message(), Some("search is disabled"));
}

#[test]
fn run_search_with_an_empty_query_shows_the_usage_message_and_loads_nothing() {
    let mut application = terminal_app();
    let mut ui_state = UiState::new(true);
    let outcome = application.run_search("   ", &mut ui_state, Instant::now());
    assert!(matches!(outcome, CommandOutcome::None));
    assert_eq!(ui_state.transient_message(), Some("usage: /search <query>"));
}

#[tokio::test]
async fn run_search_with_a_query_starts_a_load() {
    let mut application = terminal_app();
    let mut ui_state = UiState::new(true);
    let outcome = application.run_search("rust", &mut ui_state, Instant::now());
    // The single-threaded test runtime never polls the spawned load task before it is
    // aborted here, so asserting the Load variant performs no real fetch.
    match outcome {
        CommandOutcome::Load(LoadState::Active { handle, .. }) => handle.abort(),
        _ => panic!("a query with search enabled must start a load"),
    }
    assert_eq!(ui_state.transient_message(), None);
}

/// Runtime settings with every gate on, for the address-bar and history tests.
fn test_settings() -> TerminalSettings {
    TerminalSettings {
        copy_on_select: false,
        force_osc52: false,
        search_enabled: true,
        unwrap_tracking: true,
        env_overridden: EnvOverrides::default(),
    }
}

/// The history settings the terminal tests run under: persistent, with titles.
fn persistent_history_settings() -> HistorySettings {
    HistorySettings::new(HistoryMode::Persistent, 90, true)
}

/// A history store that returns fixed entries and records the clear and remove calls made
/// against it, standing in for SQLite.
#[derive(Default)]
struct FakeHistoryStore {
    entries: Vec<HistoryEntry>,
    cleared_all: Mutex<bool>,
    cleared_sites: Mutex<Vec<String>>,
    removed_ids: Mutex<Vec<u64>>,
}

impl FakeHistoryStore {
    fn with_entries(entries: Vec<HistoryEntry>) -> Self {
        Self {
            entries,
            ..Self::default()
        }
    }

    fn cleared_all(&self) -> bool {
        *self
            .cleared_all
            .lock()
            .expect("the mutex must not be poisoned")
    }

    fn removed_ids(&self) -> Vec<u64> {
        self.removed_ids
            .lock()
            .expect("the mutex must not be poisoned")
            .clone()
    }

    fn cleared_sites(&self) -> Vec<String> {
        self.cleared_sites
            .lock()
            .expect("the mutex must not be poisoned")
            .clone()
    }
}

impl HistoryStore for FakeHistoryStore {
    fn record_visit(&self, visit: NewVisit) -> Result<SuggestionEntry, StorageError> {
        Ok(SuggestionEntry::new(
            visit.url().to_string(),
            visit.host().to_string(),
            1,
            0,
            visit.visited_at(),
        ))
    }

    fn recent_entries(&self, limit: usize) -> Result<Vec<HistoryEntry>, StorageError> {
        Ok(self.entries.iter().take(limit).cloned().collect())
    }

    fn search_entries(&self, query: &str, limit: usize) -> Result<Vec<HistoryEntry>, StorageError> {
        Ok(self
            .entries
            .iter()
            .filter(|entry| entry.url().contains(query))
            .take(limit)
            .cloned()
            .collect())
    }

    fn load_suggestions(&self) -> Result<Vec<SuggestionEntry>, StorageError> {
        Ok(Vec::new())
    }

    fn remove_entry(&self, id: u64) -> Result<(), StorageError> {
        self.removed_ids
            .lock()
            .expect("the mutex must not be poisoned")
            .push(id);
        Ok(())
    }

    fn clear_all(&self) -> Result<(), StorageError> {
        *self
            .cleared_all
            .lock()
            .expect("the mutex must not be poisoned") = true;
        Ok(())
    }

    fn clear_site(&self, host: &str) -> Result<(), StorageError> {
        self.cleared_sites
            .lock()
            .expect("the mutex must not be poisoned")
            .push(host.to_string());
        Ok(())
    }

    fn prune_older_than(&self, _cutoff: i64) -> Result<(), StorageError> {
        Ok(())
    }
}

fn app_with_store(store: Arc<FakeHistoryStore>) -> TerminalApp {
    let history: Option<Arc<dyn HistoryStore + Send + Sync>> = Some(store);
    let controller =
        NavigationController::with_history(history, persistent_history_settings(), Vec::new());
    TerminalApp::new(controller, ViewState::Page, test_settings())
}

/// Mirrors the event loop's Enter decision: a highlighted suggestion is taken over the
/// typed buffer.
fn resolve_submit(ui_state: &mut UiState) -> String {
    match ui_state.take_selected_suggestion() {
        Some(url) => url,
        None => ui_state.take_submit_buffer(),
    }
}

#[test]
fn typing_an_address_populates_suggestions_from_the_controller() {
    let entry = SuggestionEntry::new(
        "https://github.com/".to_string(),
        "github.com".to_string(),
        3,
        2,
        1_000,
    );
    let controller =
        NavigationController::with_history(None, persistent_history_settings(), vec![entry]);
    let application = TerminalApp::new(controller, ViewState::Page, test_settings());
    let mut ui_state = UiState::new(true);
    ui_state.enter_command_mode('g');
    ui_state.command_append_char('i');
    ui_state.command_append_char('t');

    application.refresh_address_suggestions(&mut ui_state);

    assert!(ui_state.has_address_suggestions());
    assert_eq!(ui_state.address_suggestions()[0], "https://github.com/");
}

#[test]
fn a_slash_buffer_shows_no_address_suggestions() {
    let entry = SuggestionEntry::new(
        "https://github.com/".to_string(),
        "github.com".to_string(),
        3,
        2,
        1_000,
    );
    let controller =
        NavigationController::with_history(None, persistent_history_settings(), vec![entry]);
    let application = TerminalApp::new(controller, ViewState::Page, test_settings());
    let mut ui_state = UiState::new(true);
    ui_state.enter_command_mode('/');

    application.refresh_address_suggestions(&mut ui_state);

    assert!(!ui_state.has_address_suggestions());
}

#[test]
fn an_empty_command_buffer_offers_no_suggestions() {
    let controller = NavigationController::with_history(
        None,
        persistent_history_settings(),
        vec![SuggestionEntry::new(
            "https://github.com/".to_string(),
            "github.com".to_string(),
            1,
            0,
            10,
        )],
    );
    let application = TerminalApp::new(controller, ViewState::Page, test_settings());
    let mut ui_state = UiState::new(true);

    application.refresh_address_suggestions(&mut ui_state);

    assert!(!ui_state.has_address_suggestions());
}

#[test]
fn only_buffer_editing_actions_refresh_the_suggestions() {
    assert!(action_refreshes_suggestions(InputAction::CommandAppend(
        'a'
    )));
    assert!(action_refreshes_suggestions(InputAction::EnterCommand('a')));
    assert!(action_refreshes_suggestions(InputAction::CommandDeleteBack));
    assert!(!action_refreshes_suggestions(
        InputAction::SuggestionSelectNext
    ));
    assert!(!action_refreshes_suggestions(
        InputAction::SuggestionDismiss
    ));
}

#[test]
fn enter_with_a_selected_suggestion_resolves_to_that_url_not_the_typed_text() {
    let mut ui_state = UiState::new(true);
    ui_state.enter_command_mode('g');
    ui_state.command_append_char('i');
    ui_state.command_append_char('t');
    ui_state.set_address_suggestions(vec!["https://github.com/".to_string()]);
    ui_state.suggestion_select_next();

    let submitted = resolve_submit(&mut ui_state);

    assert_eq!(submitted, "https://github.com/");
}

#[test]
fn enter_without_a_selection_resolves_to_the_typed_text() {
    let mut ui_state = UiState::new(true);
    ui_state.enter_command_mode('e');
    ui_state.command_append_char('x');
    ui_state.set_address_suggestions(vec!["https://github.com/".to_string()]);

    let submitted = resolve_submit(&mut ui_state);

    assert_eq!(submitted, "ex");
}

#[test]
fn parse_history_request_lists_on_an_empty_argument() {
    assert!(matches!(parse_history_request(""), HistoryRequest::List));
    assert!(matches!(parse_history_request("   "), HistoryRequest::List));
}

#[test]
fn parse_history_request_clears_all_on_a_bare_clear() {
    assert!(matches!(
        parse_history_request("clear"),
        HistoryRequest::ClearAll
    ));
}

#[test]
fn parse_history_request_clears_a_single_site() {
    match parse_history_request("clear example.com") {
        HistoryRequest::ClearSite(host) => assert_eq!(host, "example.com"),
        _ => panic!("clear with a host must clear that site"),
    }
}

#[test]
fn parse_history_request_treats_other_text_as_a_search_query() {
    match parse_history_request("rust lang") {
        HistoryRequest::Search(query) => assert_eq!(query, "rust lang"),
        _ => panic!("a non-clear argument must search"),
    }
    // A word that merely starts with "clear" is a search, not a clear.
    assert!(matches!(
        parse_history_request("clearance"),
        HistoryRequest::Search(_)
    ));
}

#[test]
fn submitting_the_history_command_routes_to_a_list_request() {
    let mut application = terminal_app();
    let mut ui_state = UiState::new(true);
    let mut cache = None;
    let mut scroll = ScrollState::new();
    let outcome = application.submit_command_input(
        "/history".to_string(),
        Path::new("."),
        &mut ui_state,
        &mut cache,
        &mut scroll,
        0,
        Instant::now(),
    );
    assert!(matches!(
        outcome,
        CommandOutcome::History(HistoryRequest::List)
    ));
}

#[test]
fn submitting_history_clear_routes_to_a_clear_all_request() {
    let mut application = terminal_app();
    let mut ui_state = UiState::new(true);
    let mut cache = None;
    let mut scroll = ScrollState::new();
    let outcome = application.submit_command_input(
        "/history clear".to_string(),
        Path::new("."),
        &mut ui_state,
        &mut cache,
        &mut scroll,
        0,
        Instant::now(),
    );
    assert!(matches!(
        outcome,
        CommandOutcome::History(HistoryRequest::ClearAll)
    ));
}

#[tokio::test]
async fn a_history_list_request_opens_the_list_with_the_stored_entries() {
    let store = Arc::new(FakeHistoryStore::with_entries(vec![HistoryEntry::new(
        1,
        "https://a.test/".to_string(),
        Some("A".to_string()),
        100,
    )]));
    let mut application = app_with_store(store);
    let mut ui_state = UiState::new(true);

    application
        .handle_history_request(HistoryRequest::List, &mut ui_state, Instant::now())
        .await;

    assert!(ui_state.is_in_history_mode());
    assert_eq!(ui_state.history_entries().len(), 1);
}

#[tokio::test]
async fn an_empty_history_list_request_shows_a_notice_and_opens_no_list() {
    let store = Arc::new(FakeHistoryStore::default());
    let mut application = app_with_store(store);
    let mut ui_state = UiState::new(true);

    application
        .handle_history_request(HistoryRequest::List, &mut ui_state, Instant::now())
        .await;

    assert!(!ui_state.is_in_history_mode());
    assert_eq!(ui_state.transient_message(), Some("no matching history"));
}

#[tokio::test]
async fn a_history_clear_request_invokes_the_store_clear_path() {
    let store = Arc::new(FakeHistoryStore::default());
    let mut application = app_with_store(store.clone());
    let mut ui_state = UiState::new(true);

    application
        .handle_history_request(HistoryRequest::ClearAll, &mut ui_state, Instant::now())
        .await;

    assert!(store.cleared_all());
    assert_eq!(ui_state.transient_message(), Some("history cleared"));
}

#[tokio::test]
async fn a_history_clear_site_request_clears_only_that_host() {
    let store = Arc::new(FakeHistoryStore::default());
    let mut application = app_with_store(store.clone());
    let mut ui_state = UiState::new(true);

    application
        .handle_history_request(
            HistoryRequest::ClearSite("example.com".to_string()),
            &mut ui_state,
            Instant::now(),
        )
        .await;

    assert_eq!(store.cleared_sites(), vec!["example.com".to_string()]);
}

#[tokio::test]
async fn the_delete_key_removes_the_selected_history_entry_through_the_store() {
    let store = Arc::new(FakeHistoryStore::with_entries(vec![HistoryEntry::new(
        7,
        "https://a.test/".to_string(),
        None,
        100,
    )]));
    let mut application = app_with_store(store.clone());
    let mut ui_state = UiState::new(true);
    application
        .handle_history_request(HistoryRequest::List, &mut ui_state, Instant::now())
        .await;
    assert!(ui_state.is_in_history_mode());

    application
        .delete_selected_history(&mut ui_state, Instant::now())
        .await;

    assert_eq!(store.removed_ids(), vec![7]);
    // The only entry is gone, so the list closes.
    assert!(!ui_state.is_in_history_mode());
}

/// A site-policy store that records the writes made against it, standing in for SQLite so a
/// mutation command's effect can be asserted without a database.
#[derive(Default)]
struct FakeSitePolicyStore {
    writes: Mutex<Vec<(String, String)>>,
}

impl FakeSitePolicyStore {
    fn writes(&self) -> Vec<(String, String)> {
        self.writes
            .lock()
            .expect("policy store lock must not be poisoned")
            .clone()
    }
}

impl SitePolicyStore for FakeSitePolicyStore {
    fn set_site_policy(
        &self,
        domain: &str,
        policy: &str,
        _created_at: i64,
    ) -> Result<(), StorageError> {
        self.writes
            .lock()
            .expect("policy store lock must not be poisoned")
            .push((domain.to_string(), policy.to_string()));
        Ok(())
    }

    fn remove_site_policy(&self, _domain: &str) -> Result<(), StorageError> {
        Ok(())
    }

    fn site_policy(&self, _domain: &str) -> Result<Option<String>, StorageError> {
        Ok(None)
    }

    fn all_site_policies(&self) -> Result<Vec<(String, String)>, StorageError> {
        Ok(Vec::new())
    }

    fn clear_site_policies(&self) -> Result<(), StorageError> {
        Ok(())
    }
}

fn app_with_cookie_store(store: Arc<FakeSitePolicyStore>) -> TerminalApp {
    let policy_store: Option<Arc<dyn SitePolicyStore + Send + Sync>> = Some(store);
    let controller = NavigationController::new().with_cookies(
        CookiePolicyPair::default(),
        policy_store,
        Vec::new(),
    );
    TerminalApp::new(controller, ViewState::Page, test_settings())
}

#[test]
fn allow_session_writes_the_session_policy_for_the_site() {
    let store = Arc::new(FakeSitePolicyStore::default());
    let mut application = app_with_cookie_store(store.clone());
    let mut ui_state = UiState::new(true);

    application.run_cookies(
        CookiesRequest::AllowSession("example.com".to_string()),
        &mut ui_state,
        Instant::now(),
    );

    assert_eq!(
        store.writes(),
        vec![("example.com".to_string(), "session".to_string())]
    );
}

#[test]
fn reject_writes_the_reject_policy_for_the_site() {
    let store = Arc::new(FakeSitePolicyStore::default());
    let mut application = app_with_cookie_store(store.clone());
    let mut ui_state = UiState::new(true);

    application.run_cookies(
        CookiesRequest::Reject("example.com".to_string()),
        &mut ui_state,
        Instant::now(),
    );

    assert_eq!(
        store.writes(),
        vec![("example.com".to_string(), "reject".to_string())]
    );
}

#[test]
fn a_cookies_summary_with_no_records_reports_an_empty_session() {
    let mut application = terminal_app();
    let mut ui_state = UiState::new(true);

    application.run_cookies(CookiesRequest::Summary, &mut ui_state, Instant::now());

    assert!(!ui_state.is_in_cookies_mode());
    assert_eq!(
        ui_state.transient_message(),
        Some("no cookies recorded this session")
    );
}

#[test]
fn clearing_cookies_confirms_and_does_not_open_the_popup() {
    let mut application = terminal_app();
    let mut ui_state = UiState::new(true);

    application.run_cookies(CookiesRequest::Clear, &mut ui_state, Instant::now());

    assert!(!ui_state.is_in_cookies_mode());
    assert_eq!(ui_state.transient_message(), Some("cookies cleared"));
}

#[test]
fn an_unknown_cookies_subcommand_shows_the_usage_message() {
    let mut application = terminal_app();
    let mut ui_state = UiState::new(true);

    application.run_cookies(CookiesRequest::Usage, &mut ui_state, Instant::now());

    assert!(!ui_state.is_in_cookies_mode());
    assert!(ui_state
        .transient_message()
        .expect("a usage message must be set")
        .starts_with("usage: /cookies"));
}

/// A config store that records every write, standing in for SQLite so an instant-apply
/// setting's persistence can be asserted without a database.
#[derive(Default)]
struct FakeConfigStore {
    writes: Mutex<Vec<(String, String)>>,
}

impl FakeConfigStore {
    fn writes(&self) -> Vec<(String, String)> {
        self.writes
            .lock()
            .expect("config store lock must not be poisoned")
            .clone()
    }
}

impl ConfigStore for FakeConfigStore {
    fn config_value(&self, _key: &str) -> Result<Option<String>, StorageError> {
        Ok(None)
    }

    fn set_config_value(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.writes
            .lock()
            .expect("config store lock must not be poisoned")
            .push((key.to_string(), value.to_string()));
        Ok(())
    }
}

fn app_with_config_store(store: Arc<FakeConfigStore>, settings: TerminalSettings) -> TerminalApp {
    let config: Arc<dyn ConfigStore + Send + Sync> = store;
    let controller = NavigationController::new().with_config_store(config);
    TerminalApp::new(controller, ViewState::Page, settings)
}

/// A panel model built from `settings`, the given cookie policy, and the default search
/// engine, matching what the app shows when the panel opens.
fn settings_model_for(settings: &TerminalSettings, policy: CookiePolicyPair) -> SettingsModel {
    build_settings_model(settings, policy, &SearchEngine::default())
}

#[test]
fn toggling_a_checkbox_in_the_panel_applies_it_live_and_persists_it() {
    let store = Arc::new(FakeConfigStore::default());
    let settings = TerminalSettings {
        copy_on_select: true,
        force_osc52: false,
        search_enabled: true,
        unwrap_tracking: true,
        env_overridden: EnvOverrides::default(),
    };
    let mut application = app_with_config_store(store.clone(), settings);
    let mut ui_state = UiState::new(true);
    ui_state.enter_settings_mode(settings_model_for(&settings, CookiePolicyPair::default()));
    // Move focus onto the copy-on-select checkbox (the third row).
    ui_state.settings_focus_next();
    ui_state.settings_focus_next();

    application.apply_settings_mutation(SettingsMutation::Toggle, &mut ui_state, Instant::now());

    assert!(!application.settings.copy_on_select);
    assert_eq!(
        store.writes(),
        vec![("ui.copy_on_select".to_string(), "false".to_string())]
    );
}

#[test]
fn cycling_a_cookie_radio_in_the_panel_applies_it_live_and_persists_the_policy() {
    let store = Arc::new(FakeConfigStore::default());
    let settings = test_settings();
    let mut application = app_with_config_store(store.clone(), settings);
    let mut ui_state = UiState::new(true);
    ui_state.enter_settings_mode(settings_model_for(&settings, CookiePolicyPair::default()));

    // The first row is the first-party cookie radio; the default reject cycles to allow.
    application.apply_settings_mutation(SettingsMutation::CycleNext, &mut ui_state, Instant::now());

    assert_eq!(
        application.controller().cookie_policy().first_party,
        CookiePolicy::Allow
    );
    assert_eq!(
        store.writes(),
        vec![("cookies.first_party".to_string(), "allow".to_string())]
    );
}

#[test]
fn a_mutation_on_an_env_overridden_row_changes_nothing_and_explains_why() {
    let store = Arc::new(FakeConfigStore::default());
    let settings = TerminalSettings {
        copy_on_select: false,
        force_osc52: false,
        search_enabled: true,
        unwrap_tracking: true,
        env_overridden: EnvOverrides {
            force_osc52: true,
            ..EnvOverrides::default()
        },
    };
    let mut application = app_with_config_store(store.clone(), settings);
    let mut ui_state = UiState::new(true);
    ui_state.enter_settings_mode(settings_model_for(&settings, CookiePolicyPair::default()));
    // Focus the force-osc52 checkbox (the fourth row), which the environment fixes.
    for _ in 0..3 {
        ui_state.settings_focus_next();
    }

    application.apply_settings_mutation(SettingsMutation::Toggle, &mut ui_state, Instant::now());

    assert!(!application.settings.force_osc52);
    assert!(store.writes().is_empty());
    assert_eq!(ui_state.transient_message(), Some(SETTINGS_ENV_LOCKED));
}

/// Opens the panel, focuses the search base URL text row (the sixth row), and seeds its draft
/// through the reconciler, leaving the application ready for a text edit to be typed.
fn app_editing_search_base_url() -> (Arc<FakeConfigStore>, TerminalApp, UiState, Instant) {
    let store = Arc::new(FakeConfigStore::default());
    let settings = test_settings();
    let mut application = app_with_config_store(store.clone(), settings);
    let mut ui_state = UiState::new(true);
    ui_state.enter_settings_mode(settings_model_for(&settings, CookiePolicyPair::default()));
    for _ in 0..5 {
        ui_state.settings_focus_next();
    }
    let now = Instant::now();
    application.reconcile_settings_text_field(&mut ui_state, now);
    assert_eq!(
        ui_state.settings_text_edit_id(),
        Some(SettingId::SearchBaseUrl)
    );
    (store, application, ui_state, now)
}

/// Replaces the focused text field's draft with `text`, one keystroke at a time, so the edit
/// path (dirty, debounce, validation) runs exactly as it would for real input. The delete count
/// clears any seeded value the field opened with before the new text is typed.
fn retype_text_field(ui_state: &mut UiState, text: &str, now: Instant) {
    for _ in 0..256 {
        ui_state.settings_text_delete_back(now);
    }
    for character in text.chars() {
        ui_state.settings_text_input(character, now);
    }
}

#[test]
fn a_valid_text_edit_persists_through_the_controller_and_clears_dirty() {
    let (store, mut application, mut ui_state, now) = app_editing_search_base_url();
    retype_text_field(&mut ui_state, "https://search.test/", now);

    application.maybe_autosave_settings_text(&mut ui_state, now + SETTINGS_AUTOSAVE_DEBOUNCE);

    assert!(
        ui_state.settings_text_pending_save().is_none(),
        "a saved field must no longer be dirty"
    );
    assert_eq!(
        application.controller().search_engine().base_url(),
        "https://search.test/"
    );
    assert!(store.writes().contains(&(
        "search.base_url".to_string(),
        "https://search.test/".to_string()
    )));
}

#[test]
fn a_file_scheme_base_url_is_rejected_and_never_persisted() {
    let (store, mut application, mut ui_state, now) = app_editing_search_base_url();
    let default_base_url = application
        .controller()
        .search_engine()
        .base_url()
        .to_string();
    retype_text_field(&mut ui_state, "file:///etc/passwd", now);

    application.maybe_autosave_settings_text(&mut ui_state, now + SETTINGS_AUTOSAVE_DEBOUNCE);

    assert!(
        ui_state.settings_text_pending_save().is_some(),
        "a rejected value must stay dirty"
    );
    assert_eq!(ui_state.settings_text_error(), Some(SETTINGS_TEXT_INVALID));
    assert_eq!(
        application.controller().search_engine().base_url(),
        default_base_url,
        "the live engine must be unchanged after a rejected edit"
    );
    assert!(
        !store
            .writes()
            .iter()
            .any(|(key, _)| key == "search.base_url"),
        "a rejected value must not be written to the store"
    );
}

#[test]
fn an_empty_base_url_is_rejected_and_never_persisted() {
    let (store, mut application, mut ui_state, now) = app_editing_search_base_url();
    retype_text_field(&mut ui_state, "", now);

    application.maybe_autosave_settings_text(&mut ui_state, now + SETTINGS_AUTOSAVE_DEBOUNCE);

    assert!(ui_state.settings_text_pending_save().is_some());
    assert_eq!(ui_state.settings_text_error(), Some(SETTINGS_TEXT_INVALID));
    assert!(store.writes().is_empty());
}

#[test]
fn moving_focus_off_a_dirty_valid_field_saves_it_immediately() {
    let (store, mut application, mut ui_state, now) = app_editing_search_base_url();
    retype_text_field(&mut ui_state, "https://leave.test/", now);

    // Move focus to the next row and reconcile at `now`, before any debounce could elapse.
    ui_state.settings_focus_next();
    application.reconcile_settings_text_field(&mut ui_state, now);

    assert_eq!(
        application.controller().search_engine().base_url(),
        "https://leave.test/"
    );
    assert!(store.writes().contains(&(
        "search.base_url".to_string(),
        "https://leave.test/".to_string()
    )));
}

#[test]
fn a_dirty_field_does_not_save_before_the_debounce_elapses() {
    let (store, mut application, mut ui_state, now) = app_editing_search_base_url();
    retype_text_field(&mut ui_state, "https://early.test/", now);

    application.maybe_autosave_settings_text(&mut ui_state, now + Duration::from_millis(200));

    assert!(
        ui_state.settings_text_pending_save().is_some(),
        "the field must remain dirty before the debounce elapses"
    );
    assert!(!store
        .writes()
        .iter()
        .any(|(key, _)| key == "search.base_url"));
}

#[test]
fn tab_focusing_a_citation_span_sets_the_preview_and_a_hyperlink_leaves_it_unset() {
    let citation_cache = cache_with_run(citation_run("Quote", "https://example.com/source"));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let bounds = ViewportBounds {
        height: 4,
        max_offset: 0,
    };

    advance_interactive_focus(
        &mut ui_state,
        Some(&citation_cache.buffer),
        &mut scroll,
        bounds,
    );

    assert_eq!(
        ui_state.citation_preview(),
        Some("https://example.com/source")
    );

    let link_cache = cache_with_run(link_run("Link", "https://example.com/article"));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();

    advance_interactive_focus(&mut ui_state, Some(&link_cache.buffer), &mut scroll, bounds);

    assert_eq!(ui_state.citation_preview(), None);
}

#[test]
fn exiting_interactive_navigation_clears_the_citation_preview() {
    let cache = cache_with_run(citation_run("Quote", "https://example.com/source"));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let bounds = ViewportBounds {
        height: 4,
        max_offset: 0,
    };
    advance_interactive_focus(&mut ui_state, Some(&cache.buffer), &mut scroll, bounds);
    assert!(ui_state.citation_preview().is_some());

    ui_state.exit_interactive_navigation();

    assert_eq!(ui_state.citation_preview(), None);
}

#[test]
fn hovering_a_citation_span_sets_the_preview_and_moving_off_clears_it() {
    let cache = cache_with_run(citation_run("Quote", "https://example.com/source"));
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    let mut ui_state = UiState::new(true);

    handle_mouse_event(
        mouse(MouseEventKind::Moved, CONTENT_PADDING, BODY_AREA_TOP_ROW),
        Some(&cache.buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut ui_state,
        Instant::now(),
        true,
        false,
    );
    assert_eq!(
        ui_state.citation_preview(),
        Some("https://example.com/source")
    );

    handle_mouse_event(
        mouse(
            MouseEventKind::Moved,
            CONTENT_PADDING + 20,
            BODY_AREA_TOP_ROW,
        ),
        Some(&cache.buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut ui_state,
        Instant::now(),
        true,
        false,
    );
    assert_eq!(ui_state.citation_preview(), None);
}

#[test]
fn hovering_a_plain_hyperlink_span_never_sets_the_citation_preview() {
    let cache = cache_with_run(link_run("Link", "https://example.com/article"));
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    let mut ui_state = UiState::new(true);

    handle_mouse_event(
        mouse(MouseEventKind::Moved, CONTENT_PADDING, BODY_AREA_TOP_ROW),
        Some(&cache.buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut ui_state,
        Instant::now(),
        true,
        false,
    );

    assert_eq!(ui_state.citation_preview(), None);
}

#[test]
fn clicking_a_citation_span_navigates_to_the_cite_url_exactly_like_a_plain_link() {
    let cache = cache_with_run(citation_run("Quote", "https://example.com/source"));
    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    let mut ui_state = UiState::new(true);
    let steps = [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ];
    for kind in steps {
        handle_mouse_event(
            mouse(kind, CONTENT_PADDING, BODY_AREA_TOP_ROW),
            Some(&cache.buffer),
            0,
            BODY_AREA_TOP_ROW,
            &mut selection,
            &mut navigate_to_url,
            &mut ui_state,
            Instant::now(),
            true,
            false,
        );
    }

    assert_eq!(
        navigate_to_url,
        Some("https://example.com/source".to_string())
    );
}

#[test]
fn a_quote_nested_inside_an_anchor_never_previews_and_activates_the_anchor_url() {
    let cache = cache_with_run(link_and_citation_run(
        "Quote",
        "https://example.com/article",
        "https://example.com/source",
    ));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let bounds = ViewportBounds {
        height: 4,
        max_offset: 0,
    };

    advance_interactive_focus(&mut ui_state, Some(&cache.buffer), &mut scroll, bounds);
    assert_eq!(ui_state.citation_preview(), None);

    let mut selection = TextSelection::new();
    let mut navigate_to_url = None;
    let mut hover_state = UiState::new(true);
    handle_mouse_event(
        mouse(MouseEventKind::Moved, CONTENT_PADDING, BODY_AREA_TOP_ROW),
        Some(&cache.buffer),
        0,
        BODY_AREA_TOP_ROW,
        &mut selection,
        &mut navigate_to_url,
        &mut hover_state,
        Instant::now(),
        true,
        false,
    );
    assert_eq!(hover_state.citation_preview(), None);

    let steps = [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ];
    for kind in steps {
        handle_mouse_event(
            mouse(kind, CONTENT_PADDING, BODY_AREA_TOP_ROW),
            Some(&cache.buffer),
            0,
            BODY_AREA_TOP_ROW,
            &mut selection,
            &mut navigate_to_url,
            &mut hover_state,
            Instant::now(),
            true,
            false,
        );
    }
    assert_eq!(
        navigate_to_url,
        Some("https://example.com/article".to_string())
    );
}

// --- Phase 04: unified interactive focus, field editing, and submission confirmation ---

/// Loads `body` as a local file page, so a real parsed document and seeded
/// `FormFieldValues` back the controller without any network access.
async fn load_form_page(body: &str) -> NavigationController {
    let working_directory = tempdir().expect("a temporary working directory must be created");
    std::fs::write(working_directory.path().join("page.html"), body)
        .expect("the temporary HTML file must be written");
    let url = resolve_address("page.html", working_directory.path())
        .expect("an existing local HTML file must resolve to a file URL");
    let mut controller = NavigationController::new();
    controller
        .load(url, NavigationSource::AddressBar)
        .await
        .expect("loading the temporary page must succeed");
    controller
}

fn cache_for(controller: &NavigationController) -> CachedPage {
    let buffer = controller.render(80).expect("a loaded page must render");
    CachedPage { width: 80, buffer }
}

fn wide_bounds() -> ViewportBounds {
    ViewportBounds {
        height: 40,
        max_offset: 0,
    }
}

#[test]
fn unique_interactive_targets_merges_links_and_fields_in_position_order_and_dedups_each() {
    let repeated_link = InlineRun {
        text: "Again".to_string(),
        emphasis: InlineEmphasis::none(),
        link: Some("https://example.com/a".to_string()),
        citation: None,
        anchors: Vec::new(),
    };
    let first_link = SemanticNode::Paragraph {
        runs: vec![InlineRun {
            text: "Next".to_string(),
            emphasis: InlineEmphasis::none(),
            link: Some("https://example.com/a".to_string()),
            citation: None,
            anchors: Vec::new(),
        }],
        inline_style: None,
    };
    let field = SemanticNode::Input(browser_html::InputElement {
        id: NodeId::new(0),
        kind: browser_html::InputKind::Text,
        name: None,
        value: String::new(),
        checked: false,
        label: None,
        sensitive: false,
    });
    let repeated_link_paragraph = SemanticNode::Paragraph {
        runs: vec![repeated_link],
        inline_style: None,
    };
    let document = Document::new(vec![first_link, field, repeated_link_paragraph], None, 0);
    let buffer = render_document(&document, 40, &WidthConfig::default(), None)
        .expect("a link and a field must lay out");

    let targets = unique_interactive_targets(&buffer);
    assert_eq!(
        targets,
        vec![
            InteractiveTarget::Link("https://example.com/a"),
            InteractiveTarget::Field(NodeId::new(0), FieldSpanKind::Input),
        ],
        "the repeated link URL collapses into its first occurrence, not a second entry"
    );
}

#[test]
fn tab_cycles_through_a_link_and_every_form_control_in_document_order_and_wraps() {
    let link = SemanticNode::Paragraph {
        runs: vec![InlineRun {
            text: "Next".to_string(),
            emphasis: InlineEmphasis::none(),
            link: Some("https://example.com/next".to_string()),
            citation: None,
            anchors: Vec::new(),
        }],
        inline_style: None,
    };
    let form = SemanticNode::Form(browser_html::FormElement {
        id: NodeId::new(0),
        action: "https://example.com/submit".to_string(),
        method: browser_html::FormMethod::Post,
        children: vec![
            SemanticNode::Input(browser_html::InputElement {
                id: NodeId::new(1),
                kind: browser_html::InputKind::Text,
                name: None,
                value: String::new(),
                checked: false,
                label: None,
                sensitive: false,
            }),
            SemanticNode::Button(browser_html::ButtonElement {
                id: NodeId::new(2),
                kind: browser_html::ButtonKind::Submit,
                name: None,
                value: None,
                runs: vec![InlineRun::plain("Go".to_string())],
                inline_style: None,
            }),
        ],
    });
    let document = Document::new(vec![link, form], None, 0);
    let buffer = render_document(&document, 40, &WidthConfig::default(), None)
        .expect("a link and a form must lay out");
    let cache = CachedPage { width: 40, buffer };
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();

    for expected in [0, 1, 2, 0] {
        advance_interactive_focus(
            &mut ui_state,
            Some(&cache.buffer),
            &mut scroll,
            wide_bounds(),
        );
        assert_eq!(ui_state.focused_interactive_index, Some(expected));
    }
}

#[tokio::test]
async fn activating_a_checkbox_toggles_it_without_entering_a_sub_mode() {
    let mut controller = load_form_page(
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="checkbox" name="agree" />
            </form>
        </body></html>"#,
    )
    .await;
    let mut cache = Some(cache_for(&controller));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let mut view_state = ViewState::Page;

    handle_navigation_action(
        InputAction::FocusNextInteractive,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    let request = handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );

    assert!(
        request.is_none(),
        "a checkbox activation is not a navigation request"
    );
    assert!(
        ui_state.field_text_edit_target().is_none(),
        "a checkbox activation never enters the text-edit sub-mode"
    );
    assert!(controller
        .field_values()
        .expect("a page must be current")
        .is_checked(NodeId::new(1)));
}

#[tokio::test]
async fn activating_a_radio_unchecks_its_siblings() {
    let mut controller = load_form_page(
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="radio" name="color" value="red" checked />
                <input type="radio" name="color" value="blue" />
            </form>
        </body></html>"#,
    )
    .await;
    let mut cache = Some(cache_for(&controller));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let mut view_state = ViewState::Page;

    for _ in 0..2 {
        handle_navigation_action(
            InputAction::FocusNextInteractive,
            &mut controller,
            &mut ui_state,
            &mut cache,
            &mut scroll,
            &mut view_state,
            wide_bounds(),
        );
    }
    handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );

    let field_values = controller.field_values().expect("a page must be current");
    assert!(
        !field_values.is_checked(NodeId::new(1)),
        "the previously checked radio is now unchecked"
    );
    assert!(field_values.is_checked(NodeId::new(2)));
}

#[tokio::test]
async fn activating_a_text_input_enters_text_edit_and_commit_writes_it_back() {
    let mut controller = load_form_page(
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="text" name="q" value="hello" />
            </form>
        </body></html>"#,
    )
    .await;
    let mut cache = Some(cache_for(&controller));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let mut view_state = ViewState::Page;

    handle_navigation_action(
        InputAction::FocusNextInteractive,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    assert_eq!(
        ui_state.field_text_edit_target(),
        Some((NodeId::new(1), false))
    );
    assert_eq!(ui_state.field_text_edit_cursor(), Some(("hello", 5)));

    ui_state.field_text_input('!');
    handle_navigation_action(
        InputAction::FieldTextCommit,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );

    assert_eq!(
        controller
            .field_values()
            .expect("a page must be current")
            .text(NodeId::new(1)),
        Some("hello!")
    );
    assert!(ui_state.field_text_edit_target().is_none());
    assert!(ui_state.is_in_interactive_navigation());
}

#[tokio::test]
async fn cancelling_a_text_edit_discards_the_draft() {
    let mut controller = load_form_page(
        r#"<html><body>
            <form action="/submit" method="post">
                <input type="text" name="q" value="hello" />
            </form>
        </body></html>"#,
    )
    .await;
    let mut cache = Some(cache_for(&controller));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let mut view_state = ViewState::Page;

    handle_navigation_action(
        InputAction::FocusNextInteractive,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    ui_state.field_text_input('!');
    handle_navigation_action(
        InputAction::FieldTextCancel,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );

    assert_eq!(
        controller
            .field_values()
            .expect("a page must be current")
            .text(NodeId::new(1)),
        Some("hello")
    );
    assert!(ui_state.field_text_edit_target().is_none());
}

#[tokio::test]
async fn activating_a_single_select_cycles_its_option_without_a_sub_mode() {
    let mut controller = load_form_page(
        r#"<html><body>
            <form action="/submit" method="post">
                <select name="fruit">
                    <option value="apple">Apple</option>
                    <option value="banana">Banana</option>
                </select>
            </form>
        </body></html>"#,
    )
    .await;
    let mut cache = Some(cache_for(&controller));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let mut view_state = ViewState::Page;

    handle_navigation_action(
        InputAction::FocusNextInteractive,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    assert!(ui_state.field_multi_select().is_none());
    assert_eq!(
        controller
            .field_values()
            .expect("a page must be current")
            .selected_values(NodeId::new(1)),
        ["apple"]
    );

    handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    assert_eq!(
        controller
            .field_values()
            .expect("a page must be current")
            .selected_values(NodeId::new(1)),
        ["banana"]
    );
}

#[tokio::test]
async fn activating_a_multi_select_opens_the_expansion_and_space_toggles_the_moved_to_option() {
    let mut controller = load_form_page(
        r#"<html><body>
            <form action="/submit" method="post">
                <select name="tags" multiple>
                    <option value="a">Alpha</option>
                    <option value="b">Beta</option>
                </select>
            </form>
        </body></html>"#,
    )
    .await;
    let mut cache = Some(cache_for(&controller));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let mut view_state = ViewState::Page;

    handle_navigation_action(
        InputAction::FocusNextInteractive,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    let (node_id, options, cursor) = ui_state
        .field_multi_select()
        .expect("a multi-select activation must open the expansion sub-mode");
    assert_eq!(node_id, NodeId::new(1));
    assert_eq!(options.len(), 2);
    assert_eq!(cursor, 0);

    handle_navigation_action(
        InputAction::FieldMultiSelectMoveDown,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    handle_navigation_action(
        InputAction::FieldMultiSelectToggle,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );

    assert_eq!(
        controller
            .field_values()
            .expect("a page must be current")
            .selected_values(NodeId::new(1)),
        ["b"]
    );
}

#[tokio::test]
async fn activating_a_get_forms_submit_button_requests_immediate_submission() {
    let mut controller = load_form_page(
        r#"<html><body>
            <form action="/search" method="get">
                <button type="submit">Go</button>
            </form>
        </body></html>"#,
    )
    .await;
    let mut cache = Some(cache_for(&controller));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let mut view_state = ViewState::Page;

    handle_navigation_action(
        InputAction::FocusNextInteractive,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    let request = handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );

    assert!(matches!(request, Some(NavigationRequest::Submit(id)) if id == NodeId::new(1)));
    assert!(!ui_state.is_in_submit_confirmation());
}

#[tokio::test]
async fn activating_a_post_forms_submit_button_opens_confirmation_and_waits_for_submit() {
    let mut controller = load_form_page(
        r#"<html><body>
            <form action="/create" method="post">
                <button type="submit">Go</button>
            </form>
        </body></html>"#,
    )
    .await;
    let mut cache = Some(cache_for(&controller));
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    let mut view_state = ViewState::Page;

    handle_navigation_action(
        InputAction::FocusNextInteractive,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    let activation_request = handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    assert!(
        activation_request.is_none(),
        "a POST submit does not submit on activation"
    );
    assert!(ui_state.is_in_submit_confirmation());
    let (submit_button, destination, choice) = ui_state
        .submit_confirmation()
        .expect("the confirmation view must be open");
    assert_eq!(submit_button, NodeId::new(1));
    assert!(destination.ends_with("/create"));
    assert_eq!(choice, SubmitChoice::Cancel);

    let cancel_activation = handle_navigation_action(
        InputAction::SubmitConfirmActivate,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    assert!(
        cancel_activation.is_none(),
        "confirming Cancel submits nothing"
    );
    assert!(!ui_state.is_in_submit_confirmation());

    // Re-open the confirmation and this time toggle to Submit before confirming.
    handle_navigation_action(
        InputAction::ActivateFocused,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    handle_navigation_action(
        InputAction::SubmitConfirmToggle,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    let submit_request = handle_navigation_action(
        InputAction::SubmitConfirmActivate,
        &mut controller,
        &mut ui_state,
        &mut cache,
        &mut scroll,
        &mut view_state,
        wide_bounds(),
    );
    assert!(matches!(submit_request, Some(NavigationRequest::Submit(id)) if id == submit_button));
}
