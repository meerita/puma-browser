// @file crates/browser-html/src/input_kind.rs
// @description The control type of a form input, mapped from its type attribute.
// @layer html
// @created meerita <meerita@icloud.com>

/// The control type of a form `<input>`.
///
/// The kind is derived at parse time from the `type` attribute. An absent `type`
/// defaults to `Text`, matching HTML; an unrecognized value maps to `Other` so an
/// unknown control still renders as an inert placeholder rather than being dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    Text,
    Password,
    Email,
    Search,
    Url,
    Telephone,
    Number,
    Range,
    Checkbox,
    Radio,
    File,
    Color,
    Date,
    Time,
    Submit,
    Reset,
    Button,
    Hidden,
    Other,
}

impl InputKind {
    /// The control kind named by an input's `type` attribute.
    ///
    /// An absent attribute yields `Text`; an unrecognized value yields `Other`.
    pub fn from_type_attribute(value: Option<&str>) -> InputKind {
        let Some(value) = value else {
            return InputKind::Text;
        };
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "text" => InputKind::Text,
            "password" => InputKind::Password,
            "email" => InputKind::Email,
            "search" => InputKind::Search,
            "url" => InputKind::Url,
            "tel" => InputKind::Telephone,
            "number" => InputKind::Number,
            "range" => InputKind::Range,
            "checkbox" => InputKind::Checkbox,
            "radio" => InputKind::Radio,
            "file" => InputKind::File,
            "color" => InputKind::Color,
            "date" => InputKind::Date,
            "time" => InputKind::Time,
            "submit" => InputKind::Submit,
            "reset" => InputKind::Reset,
            "button" => InputKind::Button,
            "hidden" => InputKind::Hidden,
            _ => InputKind::Other,
        }
    }

    /// Whether a control of this kind holds a value that must never be captured.
    ///
    /// A password field's value is never read, stored, or rendered, so it is marked
    /// sensitive at parse time.
    pub fn is_sensitive(self) -> bool {
        matches!(self, InputKind::Password)
    }
}
