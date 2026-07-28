// @file crates/browser-privacy/tests/cookie_decision.rs
// @description Verifies every branch of the resolved-policy cookie decision.
// @layer privacy
// @created meerita <meerita@icloud.com>

use browser_privacy::{
    decide, parse, CookieContext, CookieDecision, CookiePolicy, CookieScope, RejectionReason,
};

fn context() -> CookieContext {
    CookieContext {
        scope: CookieScope::FirstParty,
        request_is_secure: true,
    }
}

fn persistent_cookie() -> browser_privacy::ParsedCookie {
    parse("sid=abc123; Max-Age=3600").expect("a Max-Age cookie must parse")
}

fn session_cookie() -> browser_privacy::ParsedCookie {
    parse("sid=abc123").expect("a bare name=value cookie must parse")
}

#[test]
fn reject_policy_rejects_with_policy_reason() {
    let decision = decide(CookiePolicy::Reject, &persistent_cookie(), &context());

    assert_eq!(decision, CookieDecision::Reject(RejectionReason::Policy));
}

#[test]
fn ask_policy_rejects_with_ask_reason() {
    let decision = decide(CookiePolicy::Ask, &persistent_cookie(), &context());

    assert_eq!(decision, CookieDecision::Reject(RejectionReason::Ask));
}

#[test]
fn session_policy_accepts_a_persistent_cookie_as_session_only() {
    let decision = decide(CookiePolicy::Session, &persistent_cookie(), &context());

    assert_eq!(decision, CookieDecision::Accept { session_only: true });
}

#[test]
fn allow_policy_accepts_a_persistent_cookie_as_persistent() {
    let decision = decide(CookiePolicy::Allow, &persistent_cookie(), &context());

    assert_eq!(
        decision,
        CookieDecision::Accept {
            session_only: false
        }
    );
}

#[test]
fn allow_policy_accepts_a_session_cookie_as_session_only() {
    let decision = decide(CookiePolicy::Allow, &session_cookie(), &context());

    assert_eq!(decision, CookieDecision::Accept { session_only: true });
}

#[test]
fn a_secure_cookie_over_an_insecure_request_is_rejected() {
    let insecure_context = CookieContext {
        scope: CookieScope::FirstParty,
        request_is_secure: false,
    };
    let secure_cookie = parse("sid=abc123; Secure").expect("a Secure cookie must parse");

    let decision = decide(CookiePolicy::Allow, &secure_cookie, &insecure_context);

    assert_eq!(
        decision,
        CookieDecision::Reject(RejectionReason::SecureOverInsecure)
    );
}
