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

/// How an emoji grapheme cluster is measured into terminal columns.
///
/// Emoji width is genuinely terminal-dependent: the same cluster renders as one or two
/// columns depending on the terminal, its fonts, and the operating system. Rather than
/// guess, the browser exposes the choice as a mode. `Replace` is the compatibility mode for
/// terminals that cannot render emoji at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmojiWidth {
    /// Measure an emoji cluster with the same `unicode-width` measurement used for other
    /// graphemes. This trusts the terminal to render emoji at that width and is the safe
    /// default; it never changes how a non-emoji grapheme is measured.
    #[default]
    Terminal,
    /// Force an emoji cluster to advance one column, whatever its scalar content.
    Narrow,
    /// Force an emoji cluster to advance two columns, whatever its scalar content.
    Wide,
    /// Substitute a neutral placeholder for an emoji cluster and measure the placeholder,
    /// so a terminal that cannot render emoji still shows a visible mark.
    Replace,
}

/// The neutral placeholder substituted for an emoji cluster in [`EmojiWidth::Replace`].
///
/// It is a single fixed grapheme with no emoji, joiner, or control content, so replacing a
/// cluster with it can never smuggle a multi-scalar sequence or control byte into the cell
/// buffer. Its column advance is measured the same way as any other grapheme.
const EMOJI_PLACEHOLDER: &str = "\u{25AF}";

/// Configuration that governs how grapheme clusters are measured into terminal columns.
///
/// The value is threaded through layout so every width measurement honours the same mode.
/// It carries no I/O; a caller constructs it and passes it in. Layout defaults it to
/// [`AmbiguousWidth::Narrow`] and [`EmojiWidth::Terminal`] when a caller has no preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WidthConfig {
    ambiguous_width: AmbiguousWidth,
    emoji_width: EmojiWidth,
}

impl WidthConfig {
    /// Build a width configuration for the given ambiguous-width mode, measuring emoji with
    /// [`EmojiWidth::Terminal`].
    pub fn new(ambiguous_width: AmbiguousWidth) -> WidthConfig {
        WidthConfig {
            ambiguous_width,
            emoji_width: EmojiWidth::Terminal,
        }
    }

    /// The same configuration measuring emoji with the given mode.
    pub fn with_emoji_width(self, emoji_width: EmojiWidth) -> WidthConfig {
        WidthConfig {
            emoji_width,
            ..self
        }
    }

    /// The ambiguous-width mode this configuration measures with.
    pub fn ambiguous_width(&self) -> AmbiguousWidth {
        self.ambiguous_width
    }

    /// The emoji-width mode this configuration measures with.
    pub fn emoji_width(&self) -> EmojiWidth {
        self.emoji_width
    }
}

/// The number of terminal columns a single grapheme cluster advances.
///
/// A combining mark contributes zero, so a base-plus-mark cluster stays the width of its
/// base; a CJK or other wide grapheme advances two. An ambiguous-width grapheme advances
/// one or two columns according to `width_config`. An emoji cluster is routed through the
/// emoji-width mode instead. This is the only place width is measured, so wrapping,
/// clipping, and column advancement always agree.
pub(crate) fn grapheme_columns(grapheme: &str, width_config: &WidthConfig) -> usize {
    if is_emoji_cluster(grapheme) {
        return emoji_columns(grapheme, width_config);
    }
    ambiguous_columns(grapheme, width_config)
}

/// Measure a non-emoji grapheme, resolving East-Asian-ambiguous width per the config.
fn ambiguous_columns(grapheme: &str, width_config: &WidthConfig) -> usize {
    match width_config.ambiguous_width() {
        AmbiguousWidth::Narrow => UnicodeWidthStr::width(grapheme),
        AmbiguousWidth::Wide => UnicodeWidthStr::width_cjk(grapheme),
    }
}

/// Measure an emoji grapheme cluster per the emoji-width mode.
///
/// `Terminal` reuses the ordinary `unicode-width` measurement, so the default configuration
/// leaves emoji width exactly as it was before this mode existed. `Narrow` and `Wide` force
/// the whole cluster to one or two columns regardless of how many scalars it joins.
/// `Replace` measures the placeholder that will be substituted for the cluster.
fn emoji_columns(grapheme: &str, width_config: &WidthConfig) -> usize {
    match width_config.emoji_width() {
        EmojiWidth::Terminal => ambiguous_columns(grapheme, width_config),
        EmojiWidth::Narrow => 1,
        EmojiWidth::Wide => 2,
        EmojiWidth::Replace => ambiguous_columns(EMOJI_PLACEHOLDER, width_config),
    }
}

/// The placeholder shown for an emoji cluster in [`EmojiWidth::Replace`], or `None` when the
/// grapheme is written to the buffer unchanged.
///
/// The width measured for the cluster already accounts for this substitution, so replacing
/// the grapheme at write time preserves the column math.
pub(crate) fn emoji_replacement(
    grapheme: &str,
    width_config: &WidthConfig,
) -> Option<&'static str> {
    if width_config.emoji_width() != EmojiWidth::Replace {
        return None;
    }
    if !is_emoji_cluster(grapheme) {
        return None;
    }
    Some(EMOJI_PLACEHOLDER)
}

/// Whether a grapheme cluster renders as emoji.
///
/// A cluster is emoji when any of its scalar values is Extended_Pictographic: an emoji base
/// such as a face, symbol, or object. Skin-tone modifiers, variation selectors, and
/// zero-width joiners that follow a base already belong to the same cluster after
/// segmentation, so testing for any pictographic scalar classifies the whole joined
/// sequence as a single emoji unit.
fn is_emoji_cluster(grapheme: &str) -> bool {
    grapheme.chars().any(is_extended_pictographic)
}

/// Whether a scalar carries the Unicode Extended_Pictographic property.
///
/// The ranges cover the emoji symbols and the emoji planes; a scalar outside them is not an
/// emoji base and leaves its cluster on the ordinary width path.
fn is_extended_pictographic(scalar: char) -> bool {
    let code_point = scalar as u32;
    EXTENDED_PICTOGRAPHIC_RANGES
        .iter()
        .any(|(start, end)| code_point >= *start && code_point <= *end)
}

/// Inclusive scalar ranges holding the Extended_Pictographic characters this browser treats
/// as emoji bases.
const EXTENDED_PICTOGRAPHIC_RANGES: &[(u32, u32)] = &[
    (0x00A9, 0x00A9),
    (0x00AE, 0x00AE),
    (0x203C, 0x203C),
    (0x2049, 0x2049),
    (0x2122, 0x2122),
    (0x2139, 0x2139),
    (0x2194, 0x2199),
    (0x21A9, 0x21AA),
    (0x231A, 0x231B),
    (0x2328, 0x2328),
    (0x23CF, 0x23CF),
    (0x23E9, 0x23F3),
    (0x23F8, 0x23FA),
    (0x24C2, 0x24C2),
    (0x25AA, 0x25AB),
    (0x25B6, 0x25B6),
    (0x25C0, 0x25C0),
    (0x25FB, 0x25FE),
    (0x2600, 0x27BF),
    (0x2934, 0x2935),
    (0x2B00, 0x2BFF),
    (0x3030, 0x3030),
    (0x303D, 0x303D),
    (0x3297, 0x3297),
    (0x3299, 0x3299),
    (0x1F000, 0x1FAFF),
];

#[cfg(test)]
#[path = "width_tests.rs"]
mod tests;
