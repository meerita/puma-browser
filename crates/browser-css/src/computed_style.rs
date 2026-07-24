// @file crates/browser-css/src/computed_style.rs
// @description Folds a text block's inline runs onto their node's computed base style.
// @layer css
// @created meerita <meerita@icloud.com>

use browser_html::{InlineEmphasis, InlineRun};

use crate::style_properties::Emphasis;
use crate::text_style::TextStyle;

/// Compute the reduced text style for a single inline run within a text block.
///
/// The run starts from its containing node's `base` style and folds in the run's own
/// semantic emphasis and link: inline `<code>` and `<strong>` render bold-equivalent,
/// `<em>` italic-equivalent, and a linked run is underlined so it stays distinguishable
/// without relying on color. A run with no emphasis of its own keeps the node's emphasis,
/// so plain text inside a bold heading stays bold.
pub fn computed_run_style(base: TextStyle, run: &InlineRun) -> TextStyle {
    TextStyle {
        emphasis: run_emphasis(base.emphasis, &run.emphasis),
        underline: base.underline || run.link.is_some(),
        ..base
    }
}

/// Fold a run's semantic emphasis onto the node's base emphasis with a deterministic
/// precedence: bold-equivalent wins over italic, so `<strong><em>` renders bold.
fn run_emphasis(base: Emphasis, inline: &InlineEmphasis) -> Emphasis {
    if inline.strong || inline.code {
        return Emphasis::Bold;
    }
    if inline.emphasis {
        return Emphasis::Italic;
    }
    base
}
