// @file crates/browser-layout/src/width_tests.rs
// @description Unit tests for grapheme column measurement and the width configuration.
// @layer layout
// @created meerita <meerita@icloud.com>

use super::{emoji_replacement, grapheme_columns, AmbiguousWidth, EmojiWidth, WidthConfig};

/// A zero-width-joiner family sequence: four emoji joined into one grapheme cluster.
const FAMILY: &str = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";

/// A warning sign followed by the emoji variation selector U+FE0F.
const WARNING_WITH_VARIATION_SELECTOR: &str = "\u{26A0}\u{FE0F}";

#[test]
fn the_default_width_config_measures_ambiguous_width_as_narrow() {
    assert_eq!(
        WidthConfig::default().ambiguous_width(),
        AmbiguousWidth::Narrow
    );
}

#[test]
fn an_ascii_grapheme_is_one_column_in_both_modes() {
    assert_eq!(
        grapheme_columns("a", &WidthConfig::new(AmbiguousWidth::Narrow)),
        1
    );
    assert_eq!(
        grapheme_columns("a", &WidthConfig::new(AmbiguousWidth::Wide)),
        1
    );
}

#[test]
fn a_cjk_grapheme_is_two_columns_in_both_modes() {
    assert_eq!(
        grapheme_columns("界", &WidthConfig::new(AmbiguousWidth::Narrow)),
        2
    );
    assert_eq!(
        grapheme_columns("界", &WidthConfig::new(AmbiguousWidth::Wide)),
        2
    );
}

#[test]
fn a_base_plus_combining_mark_cluster_measures_as_its_base() {
    assert_eq!(grapheme_columns("e\u{0301}", &WidthConfig::default()), 1);
}

#[test]
fn an_ambiguous_width_grapheme_is_one_column_narrow_and_two_wide() {
    assert_eq!(
        grapheme_columns("\u{00a7}", &WidthConfig::new(AmbiguousWidth::Narrow)),
        1
    );
    assert_eq!(
        grapheme_columns("\u{00a7}", &WidthConfig::new(AmbiguousWidth::Wide)),
        2
    );
}

#[test]
fn the_default_width_config_measures_emoji_with_terminal_mode() {
    assert_eq!(WidthConfig::default().emoji_width(), EmojiWidth::Terminal);
}

#[test]
fn a_zero_width_joiner_family_stays_one_cluster_measured_terminal() {
    // unicode-width measures the whole joined sequence as a single two-column emoji, so
    // terminal mode leaves the family width exactly as the ordinary measurement reports it.
    assert_eq!(FAMILY.chars().count(), 7);
    assert_eq!(grapheme_columns(FAMILY, &WidthConfig::default()), 2);
}

#[test]
fn a_family_measures_one_column_narrow_and_two_columns_wide() {
    assert_eq!(
        grapheme_columns(
            FAMILY,
            &WidthConfig::default().with_emoji_width(EmojiWidth::Narrow)
        ),
        1
    );
    assert_eq!(
        grapheme_columns(
            FAMILY,
            &WidthConfig::default().with_emoji_width(EmojiWidth::Wide)
        ),
        2
    );
}

#[test]
fn a_family_measures_the_placeholder_width_in_replace_mode() {
    assert_eq!(
        grapheme_columns(
            FAMILY,
            &WidthConfig::default().with_emoji_width(EmojiWidth::Replace)
        ),
        1
    );
}

#[test]
fn an_emoji_with_a_variation_selector_is_measured_as_emoji() {
    assert_eq!(WARNING_WITH_VARIATION_SELECTOR.chars().count(), 2);
    assert_eq!(
        grapheme_columns(
            WARNING_WITH_VARIATION_SELECTOR,
            &WidthConfig::default().with_emoji_width(EmojiWidth::Narrow)
        ),
        1
    );
}

#[test]
fn replace_mode_substitutes_the_placeholder_for_an_emoji_cluster() {
    let config = WidthConfig::default().with_emoji_width(EmojiWidth::Replace);
    assert_eq!(emoji_replacement(FAMILY, &config), Some("\u{25AF}"));
}

#[test]
fn replace_mode_leaves_a_non_emoji_grapheme_unsubstituted() {
    let config = WidthConfig::default().with_emoji_width(EmojiWidth::Replace);
    assert_eq!(emoji_replacement("a", &config), None);
    assert_eq!(emoji_replacement("界", &config), None);
}

#[test]
fn an_emoji_is_only_substituted_in_replace_mode() {
    assert_eq!(emoji_replacement(FAMILY, &WidthConfig::default()), None);
    assert_eq!(
        emoji_replacement(
            FAMILY,
            &WidthConfig::default().with_emoji_width(EmojiWidth::Wide)
        ),
        None
    );
}

#[test]
fn a_non_emoji_grapheme_width_is_unchanged_by_the_emoji_mode() {
    // Only emoji clusters are routed through the emoji path; a CJK ideograph measures the
    // same in every emoji mode.
    for mode in [
        EmojiWidth::Terminal,
        EmojiWidth::Narrow,
        EmojiWidth::Wide,
        EmojiWidth::Replace,
    ] {
        assert_eq!(
            grapheme_columns("界", &WidthConfig::default().with_emoji_width(mode)),
            2
        );
    }
}
