// @file crates/browser-layout/src/width_tests.rs
// @description Unit tests for grapheme column measurement and the width configuration.
// @layer layout
// @created meerita <meerita@icloud.com>

use super::{grapheme_columns, AmbiguousWidth, WidthConfig};

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
