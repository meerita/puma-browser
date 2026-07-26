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
///
/// `anchors` names the fragment targets that land on this run: the `id` of an element and
/// the `name` of an `<a>` are captured and attached to the next run so a link to
/// `#name` can position the viewport on it. It is empty for a run that is not a target,
/// and holds several names when more than one anchor precedes the same run. Names are
/// stored decoded and control-stripped so they compare cleanly against a decoded fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineRun {
    pub text: String,
    pub emphasis: InlineEmphasis,
    pub link: Option<String>,
    pub anchors: Vec<String>,
}

impl InlineRun {
    /// Build a run of plain text with no emphasis, link, or anchor.
    pub fn plain(text: String) -> InlineRun {
        InlineRun {
            text,
            emphasis: InlineEmphasis::none(),
            link: None,
            anchors: Vec::new(),
        }
    }
}
