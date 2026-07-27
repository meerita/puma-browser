// @file crates/browser-terminal/src/viewport.rs
// @description Vertical scroll state and window math for the read-only page viewport.
// @layer terminal
// @created meerita <meerita@icloud.com>

/// The vertical scroll position of the viewport, measured in rows from the top of the
/// content.
///
/// The offset is only ever changed through these methods, each of which keeps it within
/// the scrollable range so the draw path can trust it without re-checking.
#[derive(Debug, Default)]
pub(crate) struct ScrollState {
    offset: u16,
}

impl ScrollState {
    pub(crate) fn new() -> ScrollState {
        ScrollState { offset: 0 }
    }

    pub(crate) fn offset(&self) -> u16 {
        self.offset
    }

    pub(crate) fn line_down(&mut self, max_offset: u16) {
        self.offset = self.offset.saturating_add(1).min(max_offset);
    }

    pub(crate) fn line_up(&mut self) {
        self.offset = self.offset.saturating_sub(1);
    }

    pub(crate) fn page_down(&mut self, viewport_height: u16, max_offset: u16) {
        self.offset = self.offset.saturating_add(viewport_height).min(max_offset);
    }

    pub(crate) fn page_up(&mut self, viewport_height: u16) {
        self.offset = self.offset.saturating_sub(viewport_height);
    }

    pub(crate) fn move_to_top(&mut self) {
        self.offset = 0;
    }

    pub(crate) fn move_to_bottom(&mut self, max_offset: u16) {
        self.offset = max_offset;
    }

    /// Brings `row` to the top of the viewport, clamped so the last content row never
    /// scrolls past the bottom. Used to land the viewport on a fragment's anchor.
    pub(crate) fn scroll_to(&mut self, row: u16, max_offset: u16) {
        self.offset = row.min(max_offset);
    }

    /// Scrolls the minimum needed so `row` is visible, then clamps to range. A row already
    /// inside the window leaves the offset unchanged; a row above moves to it, a row below
    /// moves just far enough to bring it to the bottom edge.
    pub(crate) fn reveal_row(&mut self, row: u16, viewport_height: u16, max_offset: u16) {
        if row < self.offset {
            self.offset = row;
        } else if row >= self.offset.saturating_add(viewport_height) {
            self.offset = row.saturating_sub(viewport_height.saturating_sub(1));
        }
        self.offset = self.offset.min(max_offset);
    }

    /// Pulls the offset back within range after the content or viewport shrank.
    pub(crate) fn clamp(&mut self, max_offset: u16) {
        self.offset = self.offset.min(max_offset);
    }
}

/// The largest valid scroll offset: how far the top of the viewport can move down
/// before the last content row sits at the bottom. Zero when the content fits.
pub(crate) fn max_scroll_offset(content_rows: u16, viewport_height: u16) -> u16 {
    content_rows.saturating_sub(viewport_height)
}

/// The scroll position as a whole-number percentage of the scrollable range.
///
/// When nothing can scroll the whole document is already visible, reported as 100.
pub(crate) fn scroll_percentage(offset: u16, max_offset: u16) -> u16 {
    if max_offset == 0 {
        return 100;
    }
    let percent = u32::from(offset) * 100 / u32::from(max_offset);
    u16::try_from(percent).unwrap_or(100)
}

#[cfg(test)]
#[path = "viewport_tests.rs"]
mod tests;
