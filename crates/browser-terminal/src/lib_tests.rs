// @file crates/browser-terminal/src/lib_tests.rs
// @description Tests for mouse gesture dispatch, coordinate mapping, and selection highlight.
// @layer terminal
// @created meerita <meerita@icloud.com>

use std::time::Instant;

use super::{
    advance_link_focus, cell_is_selected, clamped_document_coordinate, copied_message,
    decode_fragment, document_coordinate, handle_mouse_event, max_scroll_offset,
    resolve_anchor_row, retreat_link_focus, sanitize_fragment_for_display, CachedPage,
    CommandOutcome, LoadState, ScrollState, TerminalApp, TerminalSettings, TextSelection, UiState,
    ViewState, ViewportBounds, BODY_AREA_TOP_ROW, CONTENT_PADDING,
};
use browser_core::NavigationController;
use browser_html::{Document, InlineEmphasis, InlineRun, SemanticNode};
use browser_layout::{render_document, AnchorSpan, CellBuffer, CellPosition, WidthConfig};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

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
            anchors: vec![anchor_name.to_string()],
        }],
        inline_style: None,
    };
    let document = Document::new(vec![first, target], None, 0);
    let buffer = render_document(&document, 40, &WidthConfig::default())
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
                anchors: Vec::new(),
            }],
            inline_style: None,
        })
        .collect();
    let document = Document::new(nodes, None, 0);
    let buffer = render_document(&document, 40, &WidthConfig::default())
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
    advance_link_focus(
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
        advance_link_focus(
            &mut ui_state,
            Some(&cache.buffer),
            &mut scroll,
            ViewportBounds {
                height: viewport_height,
                max_offset,
            },
        );
    }
    assert_eq!(ui_state.focused_link_index, Some(3));
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
    ui_state.enter_link_navigation(4);
    scroll.scroll_to(8, max_offset);

    // Shift+Tab up to the fourth link (row 6) sits above the window and scrolls up to it.
    retreat_link_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(ui_state.focused_link_index, Some(3));
    assert_eq!(scroll.offset(), 6);
}

#[test]
fn tabbing_forward_off_the_last_link_wraps_and_scrolls_to_the_top() {
    let cache = cache_with_links(6);
    let viewport_height = 4;
    let max_offset = max_scroll_offset(cache.buffer.height(), viewport_height);
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    ui_state.enter_link_navigation(5);
    scroll.scroll_to(8, max_offset);

    advance_link_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(ui_state.focused_link_index, Some(0));
    assert_eq!(scroll.offset(), 0);
}

#[test]
fn shift_tabbing_backward_off_the_first_link_wraps_and_reveals_the_last() {
    let cache = cache_with_links(6);
    let viewport_height = 4;
    let max_offset = max_scroll_offset(cache.buffer.height(), viewport_height);
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    ui_state.enter_link_navigation(0);

    // The last link sits on row 10; the bottom edge lands it at offset 10 - (4 - 1) = 7.
    retreat_link_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(ui_state.focused_link_index, Some(5));
    assert_eq!(scroll.offset(), 10 - (viewport_height - 1));
}

#[test]
fn tabbing_onto_an_already_visible_link_leaves_the_offset_unchanged() {
    let cache = cache_with_links(6);
    let viewport_height = 6;
    let max_offset = max_scroll_offset(cache.buffer.height(), viewport_height);
    let mut ui_state = UiState::new(true);
    let mut scroll = ScrollState::new();
    ui_state.enter_link_navigation(0);

    // The second link (row 2) is already inside the window that starts at the top.
    advance_link_focus(
        &mut ui_state,
        Some(&cache.buffer),
        &mut scroll,
        ViewportBounds {
            height: viewport_height,
            max_offset,
        },
    );
    assert_eq!(ui_state.focused_link_index, Some(1));
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
