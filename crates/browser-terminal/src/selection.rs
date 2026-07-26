// @file crates/browser-terminal/src/selection.rs
// @description Mouse-drag text selection state and click-versus-drag gesture disambiguation.
// @layer terminal
// @created meerita <meerita@icloud.com>

use browser_layout::CellPosition;

/// A text selection driven by a left-button mouse drag over the page body.
///
/// A press starts a pending selection at the anchor cell. Movement while the button is
/// held sets the cursor and marks the gesture as moved, which is how the event loop tells
/// a drag (selects text) from a clean click (activates a link). Once marked as moved the
/// gesture stays a drag even if the pointer returns to the anchor cell, so a press that
/// wandered and came back never activates a link by accident.
///
/// This is a plain UI-state value. It records the two document coordinates and whether the
/// pointer has moved; it holds no clipboard or layout logic and never reads the buffer.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TextSelection {
    anchor: Option<CellPosition>,
    cursor: Option<CellPosition>,
    moved: bool,
}

impl TextSelection {
    /// An empty selection with nothing pressed or highlighted.
    pub fn new() -> TextSelection {
        TextSelection::default()
    }

    /// Starts a pending selection at the press cell, discarding any previous selection so a
    /// new press clears the old highlight.
    pub fn begin(&mut self, anchor: CellPosition) {
        self.anchor = Some(anchor);
        self.cursor = Some(anchor);
        self.moved = false;
    }

    /// Sets the cursor cell during a drag and marks the gesture as moved once the cursor
    /// leaves the anchor cell. Does nothing when no press has begun a selection.
    pub fn update(&mut self, cursor: CellPosition) {
        let Some(anchor) = self.anchor else {
            return;
        };
        self.cursor = Some(cursor);
        if cursor != anchor {
            self.moved = true;
        }
    }

    /// Whether a selection is active: a press has begun one and it has not been cleared.
    pub fn is_dragging(&self) -> bool {
        self.anchor.is_some()
    }

    /// Whether the pointer has moved off the anchor cell since the press. A moved gesture is
    /// a drag (selects text); an unmoved one is a click (activates a link).
    pub fn has_moved(&self) -> bool {
        self.moved
    }

    /// The selected span as an inclusive `(start, end)` pair in reading order (earlier row
    /// first, then smaller column). `None` until the pointer has moved, so a plain click
    /// never highlights anything.
    pub fn range(&self) -> Option<(CellPosition, CellPosition)> {
        if !self.moved {
            return None;
        }
        let anchor = self.anchor?;
        let cursor = self.cursor?;
        Some(reading_order(anchor, cursor))
    }

    /// Discards the selection so no cells stay highlighted.
    pub fn clear(&mut self) {
        self.anchor = None;
        self.cursor = None;
        self.moved = false;
    }
}

/// Orders two positions so the first is the earlier cell in row-major reading order:
/// earlier row first, then smaller column on the same row.
fn reading_order(anchor: CellPosition, cursor: CellPosition) -> (CellPosition, CellPosition) {
    let anchor_is_first =
        anchor.row < cursor.row || (anchor.row == cursor.row && anchor.column <= cursor.column);
    if anchor_is_first {
        return (anchor, cursor);
    }
    (cursor, anchor)
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
