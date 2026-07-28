// @file crates/browser-privacy/tests/site_identity.rs
// @description Verifies registrable-domain extraction and first/third-party classification.
// @layer privacy
// @created meerita <meerita@icloud.com>

use browser_privacy::{classify, registrable_domain, CookieScope};

#[test]
fn subdomains_of_a_com_site_share_one_registrable_domain() {
    assert_eq!(
        registrable_domain("www.example.com").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        registrable_domain("analytics.example.com").as_deref(),
        Some("example.com")
    );
}

#[test]
fn subdomains_of_a_multi_label_suffix_share_one_registrable_domain() {
    assert_eq!(
        registrable_domain("example.co.uk").as_deref(),
        Some("example.co.uk")
    );
    assert_eq!(
        registrable_domain("a.example.co.uk").as_deref(),
        Some("example.co.uk")
    );
}

#[test]
fn a_bare_public_suffix_has_no_registrable_domain() {
    assert_eq!(registrable_domain("co.uk"), None);
}

#[test]
fn same_registrable_domain_across_subdomains_is_first_party() {
    assert_eq!(
        classify("analytics.example.com", "www.example.com"),
        Some(CookieScope::FirstParty)
    );
}

#[test]
fn unrelated_host_is_third_party() {
    assert_eq!(
        classify("tracker.ads.net", "www.example.com"),
        Some(CookieScope::ThirdParty)
    );
}

#[test]
fn a_public_suffix_cookie_host_is_unclassifiable() {
    assert_eq!(classify("co.uk", "www.example.com"), None);
}
