// @file crates/browser-css/tests/text_style.rs
// @description Behavior tests for the TextStyle default reduced style.
// @layer css
// @created meerita <meerita@icloud.com>

use browser_css::{DisplayMode, Emphasis, TextStyle, WhiteSpace};

#[test]
fn default_text_style_is_visible_block() {
    let style = TextStyle::default();
    assert!(style.visible);
    assert_eq!(style.display_mode, DisplayMode::Block);
}

#[test]
fn default_text_style_has_no_emphasis_or_decoration() {
    let style = TextStyle::default();
    assert_eq!(style.emphasis, Emphasis::None);
    assert!(!style.underline);
    assert!(!style.strike);
    assert_eq!(style.foreground, None);
    assert_eq!(style.background, None);
    assert_eq!(style.list_marker, None);
}

#[test]
fn text_style_default_has_normal_white_space() {
    let style = TextStyle::default();
    assert_eq!(style.white_space, WhiteSpace::Normal);
}
