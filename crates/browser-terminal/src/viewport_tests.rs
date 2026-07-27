// @file crates/browser-terminal/src/viewport_tests.rs
// @description Unit tests for scroll clamping, offset limits, and scroll percentage.
// @layer terminal
// @created meerita <meerita@icloud.com>

use super::{max_scroll_offset, scroll_percentage, ScrollState};

#[test]
fn scrolling_up_at_the_top_stays_at_zero() {
    let mut scroll = ScrollState::new();
    scroll.line_up();
    assert_eq!(scroll.offset(), 0);
}

#[test]
fn scrolling_down_past_the_bottom_clamps_to_the_max_offset() {
    let mut scroll = ScrollState::new();
    let max_offset = 5;
    for _ in 0..20 {
        scroll.line_down(max_offset);
    }
    assert_eq!(scroll.offset(), max_offset);
}

#[test]
fn paging_down_clamps_to_the_max_offset() {
    let mut scroll = ScrollState::new();
    scroll.page_down(10, 4);
    assert_eq!(scroll.offset(), 4);
}

#[test]
fn a_resize_that_shrinks_content_reclamps_the_offset() {
    let mut scroll = ScrollState::new();
    scroll.move_to_bottom(80);
    assert_eq!(scroll.offset(), 80);
    scroll.clamp(20);
    assert_eq!(scroll.offset(), 20);
}

#[test]
fn max_scroll_offset_is_zero_when_content_fits_the_viewport() {
    assert_eq!(max_scroll_offset(10, 24), 0);
}

#[test]
fn scroll_to_brings_the_target_row_to_the_top() {
    let mut scroll = ScrollState::new();
    scroll.scroll_to(12, 40);
    assert_eq!(scroll.offset(), 12);
}

#[test]
fn scroll_to_a_row_past_the_end_clamps_to_the_max_offset() {
    let mut scroll = ScrollState::new();
    scroll.scroll_to(200, 30);
    assert_eq!(scroll.offset(), 30);
}

#[test]
fn max_scroll_offset_is_the_overflow_when_content_exceeds_the_viewport() {
    assert_eq!(max_scroll_offset(100, 24), 76);
}

#[test]
fn scroll_percentage_is_zero_at_the_top_and_full_at_the_bottom() {
    assert_eq!(scroll_percentage(0, 80), 0);
    assert_eq!(scroll_percentage(80, 80), 100);
}

#[test]
fn scroll_percentage_is_full_when_nothing_can_scroll() {
    assert_eq!(scroll_percentage(0, 0), 100);
}

#[test]
fn revealing_a_row_above_the_window_brings_it_to_the_top() {
    let mut scroll = ScrollState::new();
    scroll.scroll_to(30, 100);
    scroll.reveal_row(10, 20, 100);
    assert_eq!(scroll.offset(), 10);
}

#[test]
fn revealing_a_row_below_the_window_brings_it_to_the_bottom_edge() {
    let mut scroll = ScrollState::new();
    let viewport_height = 20;
    scroll.reveal_row(50, viewport_height, 100);
    assert_eq!(scroll.offset(), 50 - (viewport_height - 1));
}

#[test]
fn revealing_a_row_already_visible_leaves_the_offset_unchanged() {
    let mut scroll = ScrollState::new();
    scroll.scroll_to(10, 100);
    scroll.reveal_row(15, 20, 100);
    assert_eq!(scroll.offset(), 10);
}

#[test]
fn revealing_a_row_near_the_end_clamps_to_the_max_offset() {
    let mut scroll = ScrollState::new();
    scroll.reveal_row(95, 20, 30);
    assert_eq!(scroll.offset(), 30);
}

#[test]
fn revealing_a_row_never_scrolls_when_the_page_fits() {
    let mut scroll = ScrollState::new();
    scroll.reveal_row(40, 20, 0);
    assert_eq!(scroll.offset(), 0);
}
