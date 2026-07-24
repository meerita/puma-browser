// @file crates/browser-layout/src/width.rs
// @description Width configuration and the single grapheme-cluster column measurement.
// @layer layout
// @created meerita <meerita@icloud.com>

use unicode_width::UnicodeWidthStr;

/// How a grapheme whose East Asian width is *ambiguous* is measured.
///
/// Ambiguous-width characters (for example some box-drawing and Greek letters) render as
/// one column in a Western terminal and two columns in an East Asian one. The correct
/// choice depends on the terminal, so it is a configurable mode rather than a fixed rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmbiguousWidth {
    /// An ambiguous-width grapheme advances one column. The safe default for most
    /// terminals.
    #[default]
    Narrow,
    /// An ambiguous-width grapheme advances two columns, matching East Asian terminals.
    Wide,
}

/// Configuration that governs how grapheme clusters are measured into terminal columns.
///
/// The value is threaded through layout so every width measurement honours the same mode.
/// It carries no I/O; a caller constructs it and passes it in. Layout defaults it to
/// [`AmbiguousWidth::Narrow`] when a caller has no preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WidthConfig {
    ambiguous_width: AmbiguousWidth,
}

impl WidthConfig {
    /// Build a width configuration for the given ambiguous-width mode.
    pub fn new(ambiguous_width: AmbiguousWidth) -> WidthConfig {
        WidthConfig { ambiguous_width }
    }

    /// The ambiguous-width mode this configuration measures with.
    pub fn ambiguous_width(&self) -> AmbiguousWidth {
        self.ambiguous_width
    }
}

/// The number of terminal columns a single grapheme cluster advances.
///
/// A combining mark contributes zero, so a base-plus-mark cluster stays the width of its
/// base; a CJK or other wide grapheme advances two. An ambiguous-width grapheme advances
/// one or two columns according to `width_config`. This is the only place width is
/// measured, so wrapping, clipping, and column advancement always agree.
pub(crate) fn grapheme_columns(grapheme: &str, width_config: &WidthConfig) -> usize {
    match width_config.ambiguous_width() {
        AmbiguousWidth::Narrow => UnicodeWidthStr::width(grapheme),
        AmbiguousWidth::Wide => UnicodeWidthStr::width_cjk(grapheme),
    }
}

#[cfg(test)]
#[path = "width_tests.rs"]
mod tests;
