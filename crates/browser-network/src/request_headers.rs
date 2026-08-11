// @file crates/browser-network/src/request_headers.rs
// @description Outbound request identity: User-Agent, Accept-Language, and the fixed
//   Accept/DNT/Sec-GPC headers applied to every HTTP request.
// @layer network
// @created meerita <meerita@icloud.com>

/// The fixed `Accept` header value sent on every request.
const ACCEPT: &str = "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8";

/// Every outbound HTTP request's identity: `User-Agent` and `Accept-Language`, built once
/// from the app version, OS/arch/locale detection, and applied to every request alongside
/// the fixed `Accept`, `DNT`, and `Sec-GPC` headers.
#[derive(Debug)]
pub struct RequestHeaders {
    user_agent: String,
    accept_language: String,
}

impl RequestHeaders {
    /// Build a `User-Agent` and `Accept-Language` from the app version and, when
    /// available, OS family, OS version, and locale. Any missing detail is omitted
    /// rather than guessed.
    pub fn new(
        app_version: &str,
        os_family: Option<&str>,
        os_version: Option<&str>,
        arch: &str,
        locale: Option<&str>,
    ) -> Self {
        Self {
            user_agent: build_user_agent(app_version, os_family, os_version, arch),
            accept_language: build_accept_language(locale),
        }
    }

    /// Apply this identity, plus the fixed `Accept`, `DNT`, and `Sec-GPC` headers, to an
    /// outgoing request builder.
    pub(crate) fn apply(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .header(reqwest::header::USER_AGENT, self.user_agent.as_str())
            .header(reqwest::header::ACCEPT, ACCEPT)
            .header(
                reqwest::header::ACCEPT_LANGUAGE,
                self.accept_language.as_str(),
            )
            .header(reqwest::header::HeaderName::from_static("dnt"), "1")
            .header(reqwest::header::HeaderName::from_static("sec-gpc"), "1")
    }
}

impl Default for RequestHeaders {
    fn default() -> Self {
        Self::new(
            env!("CARGO_PKG_VERSION"),
            None,
            None,
            std::env::consts::ARCH,
            None,
        )
    }
}

fn build_user_agent(
    app_version: &str,
    os_family: Option<&str>,
    os_version: Option<&str>,
    arch: &str,
) -> String {
    let mut tokens = Vec::new();
    if let Some(os_family) = os_family {
        let platform = os_version
            .map(|version| format!("{os_family} {version}"))
            .unwrap_or_else(|| os_family.to_string());
        tokens.push(platform);
    }
    tokens.push(normalize_arch(arch).to_string());
    format!(
        "Puma/{app_version} ({}; +https://github.com/meerita/puma-browser)",
        tokens.join("; ")
    )
}

/// Map an architecture identifier to the conventional token used in a User-Agent
/// string. `"aarch64"` reads as `"arm64"` there; every other value passes through.
fn normalize_arch(arch: &str) -> &str {
    match arch {
        "aarch64" => "arm64",
        other => other,
    }
}

fn build_accept_language(locale: Option<&str>) -> String {
    let locale = locale.unwrap_or("en-US");
    let language = locale.split(['-', '_']).next().unwrap_or(locale);
    if language.eq_ignore_ascii_case("en") {
        return locale.to_string();
    }
    format!("{locale}, {language}, en;q=0.5")
}

#[cfg(test)]
#[path = "request_headers_tests.rs"]
mod tests;
