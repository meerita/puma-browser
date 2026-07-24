// @file crates/browser-html/src/landmark_role.rs
// @description The page-region role a landmark node establishes, mapped from element or ARIA role.
// @layer html
// @created meerita <meerita@icloud.com>

/// The kind of page region a landmark element establishes.
///
/// A landmark is a structural region of the page (navigation, main content, and the
/// like). The role is derived at parse time from the element name or an explicit ARIA
/// `role` attribute, so downstream layers read a stable region kind rather than the raw
/// tag or attribute string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LandmarkRole {
    Navigation,
    Main,
    Complementary,
    ContentInfo,
    Banner,
    Region,
    Search,
    Form,
}

impl LandmarkRole {
    /// The landmark role a sectioning element establishes, if it is a landmark element.
    ///
    /// `<section>` maps to `Region` even though it is only a landmark when named; the
    /// reduced model treats every mapped element as its region kind.
    pub fn from_tag(tag: &str) -> Option<LandmarkRole> {
        match tag {
            "nav" => Some(LandmarkRole::Navigation),
            "main" => Some(LandmarkRole::Main),
            "aside" => Some(LandmarkRole::Complementary),
            "footer" => Some(LandmarkRole::ContentInfo),
            "header" => Some(LandmarkRole::Banner),
            "section" => Some(LandmarkRole::Region),
            _ => None,
        }
    }

    /// The landmark role named by an ARIA `role` attribute, if it names one.
    ///
    /// An explicit ARIA role takes precedence over the element name so an authored
    /// override is honored.
    pub fn from_aria_role(role: &str) -> Option<LandmarkRole> {
        match role.trim().to_ascii_lowercase().as_str() {
            "navigation" => Some(LandmarkRole::Navigation),
            "main" => Some(LandmarkRole::Main),
            "complementary" => Some(LandmarkRole::Complementary),
            "contentinfo" => Some(LandmarkRole::ContentInfo),
            "banner" => Some(LandmarkRole::Banner),
            "region" => Some(LandmarkRole::Region),
            "search" => Some(LandmarkRole::Search),
            "form" => Some(LandmarkRole::Form),
            _ => None,
        }
    }
}
