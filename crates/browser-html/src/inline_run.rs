// @file crates/browser-html/src/inline_run.rs
// @description Inline styled text run and its semantic emphasis flags for text-bearing nodes.
// @layer html
// @created meerita <meerita@icloud.com>

/// Which semantic emphases apply to an inline run.
///
/// Emphasis here is a parse-time property derived from the source tag (`<em>`,
/// `<strong>`, `<code>`), not a computed style. A run can carry several at once, so
/// each is an independent flag rather than a single enum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineEmphasis {
    pub strong: bool,
    pub emphasis: bool,
    pub code: bool,
}

impl InlineEmphasis {
    /// The emphasis of a run with no emphasis applied.
    pub fn none() -> InlineEmphasis {
        InlineEmphasis {
            strong: false,
            emphasis: false,
            code: false,
        }
    }
}

/// A contiguous span of text within a text-bearing node, with its own emphasis and link.
///
/// `text` is sanitized by the parser before the run is constructed, exactly as block
/// text was sanitized before. `link`, when present, is a reference resolved against the
/// document's base URL; it is a plain `String` because URL validation belongs to the
/// network layer, not here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineRun {
    pub text: String,
    pub emphasis: InlineEmphasis,
    pub link: Option<String>,
}

impl InlineRun {
    /// Build a run of plain text with no emphasis and no link.
    pub fn plain(text: String) -> InlineRun {
        InlineRun {
            text,
            emphasis: InlineEmphasis::none(),
            link: None,
        }
    }
}
