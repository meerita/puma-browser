// @file crates/browser-css/tests/computed_run_style.rs
// @description Verifies computed_run_style folds a run's emphasis and link onto the base style.
// @layer css
// @created meerita <meerita@icloud.com>

use browser_css::{computed_run_style, Color, Emphasis, TextStyle};
use browser_html::{InlineEmphasis, InlineRun};

fn run_with(emphasis: InlineEmphasis, link: Option<String>) -> InlineRun {
    InlineRun {
        text: String::from("word"),
        emphasis,
        link,
    }
}

#[test]
fn a_strong_run_renders_bold() {
    let run = run_with(
        InlineEmphasis {
            strong: true,
            emphasis: false,
            code: false,
        },
        None,
    );

    let style = computed_run_style(TextStyle::default(), &run);

    assert_eq!(style.emphasis, Emphasis::Bold);
}

#[test]
fn an_emphasis_run_renders_italic() {
    let run = run_with(
        InlineEmphasis {
            strong: false,
            emphasis: true,
            code: false,
        },
        None,
    );

    let style = computed_run_style(TextStyle::default(), &run);

    assert_eq!(style.emphasis, Emphasis::Italic);
}

#[test]
fn an_inline_code_run_renders_bold() {
    let run = run_with(
        InlineEmphasis {
            strong: false,
            emphasis: false,
            code: true,
        },
        None,
    );

    let style = computed_run_style(TextStyle::default(), &run);

    assert_eq!(style.emphasis, Emphasis::Bold);
}

#[test]
fn a_strong_emphasis_run_resolves_to_bold_by_precedence() {
    let run = run_with(
        InlineEmphasis {
            strong: true,
            emphasis: true,
            code: false,
        },
        None,
    );

    let style = computed_run_style(TextStyle::default(), &run);

    assert_eq!(style.emphasis, Emphasis::Bold);
}

#[test]
fn a_linked_run_is_underlined() {
    let run = run_with(InlineEmphasis::none(), Some(String::from("/path")));

    let style = computed_run_style(TextStyle::default(), &run);

    assert!(style.underline, "a linked run must be underlined");
}

#[test]
fn a_strong_run_foreground_is_bright_white() {
    let run = run_with(
        InlineEmphasis {
            strong: true,
            emphasis: false,
            code: false,
        },
        None,
    );

    let style = computed_run_style(TextStyle::default(), &run);

    assert_eq!(style.foreground, Some(Color::BrightWhite));
}

#[test]
fn a_linked_run_foreground_is_yellow() {
    let run = run_with(InlineEmphasis::none(), Some(String::from("/path")));

    let style = computed_run_style(TextStyle::default(), &run);

    assert_eq!(style.foreground, Some(Color::Yellow));
}

#[test]
fn a_plain_run_keeps_the_base_emphasis() {
    let base = TextStyle {
        emphasis: Emphasis::Bold,
        ..TextStyle::default()
    };
    let run = run_with(InlineEmphasis::none(), None);

    let style = computed_run_style(base, &run);

    assert_eq!(
        style.emphasis,
        Emphasis::Bold,
        "plain text in a bold heading stays bold"
    );
    assert!(!style.underline, "a run with no link is not underlined");
}
