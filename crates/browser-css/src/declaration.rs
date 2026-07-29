// @file crates/browser-css/src/declaration.rs
// @description Parses a raw inline style string into the reduced set of style declarations.
// @layer css
// @created meerita <meerita@icloud.com>

use cssparser::{
    match_ignore_ascii_case, AtRuleParser, CowRcStr, DeclarationParser, ParseError, Parser,
    ParserInput, ParserState, QualifiedRuleParser, RuleBodyItemParser, RuleBodyParser,
};

use crate::style_properties::{
    Color, DisplayMode, Emphasis, ListMarker, ReadingOrder, TextTransform, WhiteSpace,
};

/// A set of optional style properties: one layer of the cascade.
///
/// Every field is optional so a layer can tell an absent property from one set to a
/// value, which is what lets an inner layer override an outer one property by property.
/// The same type carries both the user-agent layer and the properties parsed from an
/// inline `style` attribute. The inline parser only ever sets the subset a `style`
/// attribute can express; `spacing_before`, `spacing_after`, and `reading_order` are set
/// only by the user-agent layer, since no supported inline property maps to them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Declarations {
    pub display: Option<DisplayMode>,
    pub visible: Option<bool>,
    pub white_space: Option<WhiteSpace>,
    pub emphasis: Option<Emphasis>,
    pub text_transform: Option<TextTransform>,
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub list_marker: Option<ListMarker>,
    pub spacing_before: Option<u16>,
    pub spacing_after: Option<u16>,
    pub reading_order: Option<ReadingOrder>,
}

/// Parse a raw inline style string into its reduced declarations.
///
/// Unknown properties, unknown values, and malformed declarations are ignored: each is
/// dropped and parsing continues with the next declaration, so adversarial or partial
/// input degrades to the properties it could understand rather than an error. Later
/// declarations of the same property override earlier ones, matching source order.
pub(crate) fn parse_inline_style(raw: &str) -> Declarations {
    let mut input = ParserInput::new(raw);
    let mut parser = Parser::new(&mut input);
    let mut collector = DeclarationReader;
    let mut declarations = Declarations::default();
    let body = RuleBodyParser::new(&mut parser, &mut collector);
    for declaration in body.flatten() {
        declaration.apply_to(&mut declarations);
    }
    declarations
}

/// One recognized declaration, before it is folded into the [`Declarations`] set.
///
/// `TextDecoration` carries two independent flags because a single `text-decoration`
/// value can set both underline and line-through at once.
enum Declaration {
    Display(DisplayMode),
    Visible(bool),
    WhiteSpace(WhiteSpace),
    Emphasis(Emphasis),
    TextTransform(TextTransform),
    Foreground(Color),
    Background(Color),
    TextDecoration { underline: bool, strike: bool },
    ListMarker(ListMarker),
}

impl Declaration {
    fn apply_to(self, declarations: &mut Declarations) {
        match self {
            Declaration::Display(display) => declarations.display = Some(display),
            Declaration::Visible(visible) => declarations.visible = Some(visible),
            Declaration::WhiteSpace(white_space) => declarations.white_space = Some(white_space),
            Declaration::Emphasis(emphasis) => declarations.emphasis = Some(emphasis),
            Declaration::TextTransform(transform) => declarations.text_transform = Some(transform),
            Declaration::Foreground(color) => declarations.foreground = Some(color),
            Declaration::Background(color) => declarations.background = Some(color),
            Declaration::TextDecoration { underline, strike } => {
                declarations.underline = Some(underline);
                declarations.strike = Some(strike);
            }
            Declaration::ListMarker(marker) => declarations.list_marker = Some(marker),
        }
    }
}

/// A `cssparser` declaration reader that recognizes only the reduced property subset.
///
/// It parses declarations and never qualified rules or at-rules, since an inline `style`
/// attribute is a bare declaration list. An unrecognized property or value returns an
/// error for that one declaration, which the caller drops.
struct DeclarationReader;

impl<'i> DeclarationParser<'i> for DeclarationReader {
    type Declaration = Declaration;
    type Error = ();

    fn parse_value<'t>(
        &mut self,
        name: CowRcStr<'i>,
        input: &mut Parser<'i, 't>,
        _declaration_start: &ParserState,
    ) -> Result<Self::Declaration, ParseError<'i, Self::Error>> {
        parse_declaration(&name, input)
    }
}

impl<'i> AtRuleParser<'i> for DeclarationReader {
    type Prelude = ();
    type AtRule = Declaration;
    type Error = ();
}

impl<'i> QualifiedRuleParser<'i> for DeclarationReader {
    type Prelude = ();
    type QualifiedRule = Declaration;
    type Error = ();
}

impl<'i> RuleBodyItemParser<'i, Declaration, ()> for DeclarationReader {
    fn parse_declarations(&self) -> bool {
        true
    }

    fn parse_qualified(&self) -> bool {
        false
    }
}

fn parse_declaration<'i>(
    name: &str,
    input: &mut Parser<'i, '_>,
) -> Result<Declaration, ParseError<'i, ()>> {
    match_ignore_ascii_case! { name,
        "display" => parse_ident_value(input, display_value).map(Declaration::Display),
        "visibility" => parse_ident_value(input, visibility_value).map(Declaration::Visible),
        "white-space" => parse_ident_value(input, white_space_value).map(Declaration::WhiteSpace),
        "text-transform" => {
            parse_ident_value(input, text_transform_value).map(Declaration::TextTransform)
        },
        "font-weight" => parse_font_weight(input).map(Declaration::Emphasis),
        "color" => parse_ident_value(input, color_keyword).map(Declaration::Foreground),
        "background-color" | "background" => {
            parse_ident_value(input, color_keyword).map(Declaration::Background)
        },
        "text-decoration" | "text-decoration-line" => parse_text_decoration(input),
        "list-style-type" | "list-style" => {
            parse_ident_value(input, list_marker_value).map(Declaration::ListMarker)
        },
        _ => Err(input.new_custom_error(())),
    }
}

/// Read the declaration's value as one identifier and map it with `map`.
///
/// A value that is not a single recognized identifier fails, so the whole declaration is
/// dropped and the node keeps its user-agent style for that property.
fn parse_ident_value<'i, Value>(
    input: &mut Parser<'i, '_>,
    map: impl Fn(&str) -> Option<Value>,
) -> Result<Value, ParseError<'i, ()>> {
    let identifier = input.expect_ident().map_err(ParseError::from)?;
    map(identifier).ok_or_else(|| input.new_custom_error(()))
}

/// Read a `font-weight` value as bold-or-not: a numeric weight of 600 or more, or a
/// bold-ish keyword, folds to bold; every other recognized value folds to no emphasis.
fn parse_font_weight<'i>(input: &mut Parser<'i, '_>) -> Result<Emphasis, ParseError<'i, ()>> {
    if let Ok(identifier) = input.try_parse(|parser| parser.expect_ident_cloned()) {
        return font_weight_keyword(&identifier).ok_or_else(|| input.new_custom_error(()));
    }
    let weight = input.expect_number().map_err(ParseError::from)?;
    Ok(emphasis_for_weight(weight))
}

fn parse_text_decoration<'i>(
    input: &mut Parser<'i, '_>,
) -> Result<Declaration, ParseError<'i, ()>> {
    let mut underline = false;
    let mut strike = false;
    let mut recognized = false;
    while let Ok(identifier) = input.try_parse(|parser| parser.expect_ident_cloned()) {
        match_ignore_ascii_case! { &*identifier,
            "underline" => {
                underline = true;
                recognized = true;
            },
            "line-through" => {
                strike = true;
                recognized = true;
            },
            "none" => recognized = true,
            _ => {},
        }
    }
    if !recognized {
        return Err(input.new_custom_error(()));
    }
    Ok(Declaration::TextDecoration { underline, strike })
}

fn display_value(value: &str) -> Option<DisplayMode> {
    match_ignore_ascii_case! { value,
        "none" => Some(DisplayMode::Hidden),
        "block" => Some(DisplayMode::Block),
        "inline" | "inline-block" => Some(DisplayMode::Inline),
        "list-item" => Some(DisplayMode::ListItem),
        _ => None,
    }
}

fn visibility_value(value: &str) -> Option<bool> {
    match_ignore_ascii_case! { value,
        "visible" => Some(true),
        "hidden" | "collapse" => Some(false),
        _ => None,
    }
}

fn white_space_value(value: &str) -> Option<WhiteSpace> {
    match_ignore_ascii_case! { value,
        "normal" => Some(WhiteSpace::Normal),
        "nowrap" => Some(WhiteSpace::NoWrap),
        "pre" | "pre-wrap" | "pre-line" => Some(WhiteSpace::Pre),
        _ => None,
    }
}

fn text_transform_value(value: &str) -> Option<TextTransform> {
    match_ignore_ascii_case! { value,
        "none" => Some(TextTransform::None),
        "uppercase" => Some(TextTransform::Uppercase),
        "lowercase" => Some(TextTransform::Lowercase),
        "capitalize" => Some(TextTransform::Capitalize),
        _ => None,
    }
}

fn list_marker_value(value: &str) -> Option<ListMarker> {
    match_ignore_ascii_case! { value,
        "none" => Some(ListMarker::None),
        "decimal" => Some(ListMarker::Decimal),
        "disc" | "circle" | "square" => Some(ListMarker::Disc),
        _ => None,
    }
}

fn font_weight_keyword(value: &str) -> Option<Emphasis> {
    match_ignore_ascii_case! { value,
        "bold" | "bolder" => Some(Emphasis::Bold),
        "normal" | "lighter" => Some(Emphasis::None),
        _ => None,
    }
}

/// Weights of 600 and above render bold; lighter weights carry no bold emphasis.
fn emphasis_for_weight(weight: f32) -> Emphasis {
    if weight >= 600.0 {
        return Emphasis::Bold;
    }
    Emphasis::None
}

/// Map a CSS color keyword to the nearest terminal palette color.
///
/// Only named colors are understood; numeric and functional colors (hex, `rgb()`) map to
/// no palette entry and leave the property at its user-agent value, since the reduced
/// palette cannot represent an arbitrary color faithfully.
fn color_keyword(value: &str) -> Option<Color> {
    match_ignore_ascii_case! { value,
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" | "fuchsia" => Some(Color::Magenta),
        "cyan" | "aqua" => Some(Color::Cyan),
        "white" => Some(Color::White),
        "gray" | "grey" => Some(Color::BrightBlack),
        "silver" => Some(Color::White),
        "maroon" => Some(Color::Red),
        "olive" => Some(Color::Yellow),
        "lime" => Some(Color::BrightGreen),
        "teal" => Some(Color::Cyan),
        "navy" => Some(Color::Blue),
        "purple" => Some(Color::Magenta),
        _ => None,
    }
}
