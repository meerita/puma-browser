// @file crates/browser-core/tests/cookies.rs
// @description Behavior tests for cookie enforcement: accept, resend, reject, exceptions, redirects.
// @layer core
// @created meerita <meerita@icloud.com>

use std::sync::{Arc, Mutex};

use browser_core::{
    BrowserUrl, CookiePolicy, CookiePolicyPair, NavigationController, NavigationSource,
    RejectionReason, SitePolicyStore,
};
use browser_storage::StorageError;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

/// An in-memory [`SitePolicyStore`] fake: no SQLite, no disk, just a map behind a mutex so
/// a write-through can be observed without a real database.
#[derive(Default)]
struct InMemorySitePolicyStore {
    policies: Mutex<Vec<(String, String)>>,
}

impl SitePolicyStore for InMemorySitePolicyStore {
    fn set_site_policy(
        &self,
        domain: &str,
        policy: &str,
        _created_at: i64,
    ) -> Result<(), StorageError> {
        let mut policies = self.policies.lock().expect("policy mutex must lock");
        policies.retain(|(stored_domain, _)| stored_domain != domain);
        policies.push((domain.to_string(), policy.to_string()));
        Ok(())
    }

    fn remove_site_policy(&self, domain: &str) -> Result<(), StorageError> {
        let mut policies = self.policies.lock().expect("policy mutex must lock");
        policies.retain(|(stored_domain, _)| stored_domain != domain);
        Ok(())
    }

    fn site_policy(&self, domain: &str) -> Result<Option<String>, StorageError> {
        let policies = self.policies.lock().expect("policy mutex must lock");
        Ok(policies
            .iter()
            .find(|(stored_domain, _)| stored_domain == domain)
            .map(|(_, policy)| policy.clone()))
    }

    fn all_site_policies(&self) -> Result<Vec<(String, String)>, StorageError> {
        let policies = self.policies.lock().expect("policy mutex must lock");
        Ok(policies.clone())
    }

    fn clear_site_policies(&self) -> Result<(), StorageError> {
        let mut policies = self.policies.lock().expect("policy mutex must lock");
        policies.clear();
        Ok(())
    }
}

fn url_for(server: &MockServer, suffix: &str) -> BrowserUrl {
    let full = format!("{}{}", server.uri(), suffix);
    BrowserUrl::parse(&full).expect("mock server URL must parse")
}

/// The bare host of the mock server, so a per-site exception is set for the same host the
/// controller then resolves to a registrable domain when it classifies cookies.
fn server_host(server: &MockServer) -> String {
    url_for(server, "/")
        .host_str()
        .expect("the mock server URL must carry a host")
        .to_string()
}

/// The value of the `Cookie` request header on `request`, if it carried one.
fn cookie_header(request: &Request) -> Option<String> {
    request
        .headers
        .get("cookie")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

/// Every request the server received whose path equals `suffix`, in arrival order.
async fn requests_to(server: &MockServer, suffix: &str) -> Vec<Request> {
    server
        .received_requests()
        .await
        .expect("request recording must be enabled on the mock server")
        .into_iter()
        .filter(|request| request.url.path() == suffix)
        .collect()
}

async fn mount_page_with_set_cookie(server: &MockServer, page_path: &str, set_cookie: &str) {
    Mock::given(method("GET"))
        .and(path(page_path))
        .respond_with(
            ResponseTemplate::new(200)
                .append_header("set-cookie", set_cookie)
                .set_body_raw("<html><body><p>page</p></body></html>", "text/html"),
        )
        .mount(server)
        .await;
}

async fn mount_plain_page(server: &MockServer, page_path: &str) {
    Mock::given(method("GET"))
        .and(path(page_path))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_raw("<html><body><p>ok</p></body></html>", "text/html"),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_first_party_session_cookie_is_accepted_and_resent() {
    let server = MockServer::start().await;
    mount_page_with_set_cookie(&server, "/set", "sid=abc").await;
    mount_plain_page(&server, "/next").await;
    let mut controller =
        NavigationController::new().with_cookies(CookiePolicyPair::default(), None, Vec::new());
    controller
        .set_site_cookie_policy(&server_host(&server), CookiePolicy::Session)
        .expect("setting a session exception must succeed");

    controller
        .load(url_for(&server, "/set"), NavigationSource::AddressBar)
        .await
        .expect("the first page must load and accept the cookie");
    controller
        .load(url_for(&server, "/next"), NavigationSource::AddressBar)
        .await
        .expect("the second page must load");

    let set_request = requests_to(&server, "/set").await;
    assert_eq!(
        cookie_header(&set_request[0]),
        None,
        "the first request sends no cookie"
    );
    let next_request = requests_to(&server, "/next").await;
    assert_eq!(
        cookie_header(&next_request[0]).as_deref(),
        Some("sid=abc"),
        "the accepted cookie is resent on the next same-origin request"
    );
}

#[tokio::test]
async fn a_third_party_cookie_is_rejected_by_default() {
    let server = MockServer::start().await;
    mount_page_with_set_cookie(&server, "/", "tid=xyz; Domain=tracker.example.com").await;
    let mut controller = NavigationController::new();

    controller
        .load(url_for(&server, "/"), NavigationSource::AddressBar)
        .await
        .expect("the page must load even when its cookie is rejected");

    let records = controller.cookie_records();
    assert_eq!(records.len(), 1);
    assert!(!records[0].accepted());
    assert_eq!(records[0].reason(), Some(RejectionReason::ThirdParty));
    assert!(!records[0].first_party());
    assert_eq!(controller.cookie_counts(), (0, 1));
}

#[tokio::test]
async fn a_first_party_cookie_is_rejected_by_the_default_policy() {
    let server = MockServer::start().await;
    mount_page_with_set_cookie(&server, "/", "sid=abc").await;
    let mut controller = NavigationController::new();

    controller
        .load(url_for(&server, "/"), NavigationSource::AddressBar)
        .await
        .expect("the page must load even when its cookie is rejected");

    let records = controller.cookie_records();
    assert_eq!(records.len(), 1);
    assert!(!records[0].accepted());
    assert_eq!(records[0].reason(), Some(RejectionReason::Policy));
    assert!(records[0].first_party());
}

#[tokio::test]
async fn an_allow_exception_stores_a_cookie_that_reject_then_drops() {
    let server = MockServer::start().await;
    mount_page_with_set_cookie(&server, "/set", "sid=abc; Max-Age=86400").await;
    mount_plain_page(&server, "/echo").await;
    let mut controller = NavigationController::new();
    controller
        .set_site_cookie_policy(&server_host(&server), CookiePolicy::Allow)
        .expect("setting an allow exception must succeed");
    controller
        .load(url_for(&server, "/set"), NavigationSource::AddressBar)
        .await
        .expect("the persistent cookie must be accepted under allow");
    controller
        .load(url_for(&server, "/echo"), NavigationSource::AddressBar)
        .await
        .expect("the echo page must load while the cookie is held");
    let before = requests_to(&server, "/echo").await;
    assert_eq!(
        cookie_header(&before[0]).as_deref(),
        Some("sid=abc"),
        "an allowed cookie is resent while it is held"
    );

    controller
        .set_site_cookie_policy(&server_host(&server), CookiePolicy::Reject)
        .expect("switching to reject must succeed");
    controller
        .load(url_for(&server, "/echo"), NavigationSource::AddressBar)
        .await
        .expect("the echo page must still load after the cookie is dropped");

    let after = requests_to(&server, "/echo").await;
    assert_eq!(after.len(), 2, "a second echo request was made");
    assert_eq!(
        cookie_header(&after[1]),
        None,
        "rejecting the site drops its cookie from the jar, so nothing is resent"
    );
}

#[tokio::test]
async fn a_cookie_set_on_a_redirect_hop_is_sent_on_the_next_hop() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/login"))
        .respond_with(
            ResponseTemplate::new(302)
                .append_header("location", "/dashboard")
                .append_header("set-cookie", "sid=abc"),
        )
        .mount(&server)
        .await;
    mount_plain_page(&server, "/dashboard").await;
    let mut controller = NavigationController::new();
    controller
        .set_site_cookie_policy(&server_host(&server), CookiePolicy::Session)
        .expect("setting a session exception must succeed");

    controller
        .load(url_for(&server, "/login"), NavigationSource::AddressBar)
        .await
        .expect("the redirect chain must resolve to the dashboard");

    let dashboard_request = requests_to(&server, "/dashboard").await;
    assert_eq!(
        cookie_header(&dashboard_request[0]).as_deref(),
        Some("sid=abc"),
        "a cookie set on the redirect hop is carried on the redirected request"
    );
    assert_eq!(
        controller
            .current_url()
            .expect("a page must be current")
            .as_str(),
        url_for(&server, "/dashboard").as_str()
    );
}

#[tokio::test]
async fn a_public_suffix_cookie_is_rejected() {
    let server = MockServer::start().await;
    mount_page_with_set_cookie(&server, "/", "x=y; Domain=co.uk").await;
    let mut controller = NavigationController::new();
    controller
        .set_site_cookie_policy(&server_host(&server), CookiePolicy::Allow)
        .expect("an allow exception must not rescue a public-suffix cookie");

    controller
        .load(url_for(&server, "/"), NavigationSource::AddressBar)
        .await
        .expect("the page must load even when its cookie targets a public suffix");

    let records = controller.cookie_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].reason(), Some(RejectionReason::PublicSuffix));
}

#[tokio::test]
async fn a_secure_cookie_over_an_insecure_request_is_rejected() {
    let server = MockServer::start().await;
    mount_page_with_set_cookie(&server, "/", "sid=abc; Secure").await;
    let mut controller = NavigationController::new();
    controller
        .set_site_cookie_policy(&server_host(&server), CookiePolicy::Session)
        .expect("setting a session exception must succeed");

    controller
        .load(url_for(&server, "/"), NavigationSource::AddressBar)
        .await
        .expect("the page must load even when its Secure cookie is refused over http");

    let records = controller.cookie_records();
    assert_eq!(records.len(), 1);
    assert!(!records[0].accepted());
    assert_eq!(
        records[0].reason(),
        Some(RejectionReason::SecureOverInsecure)
    );
}

#[tokio::test]
async fn a_malformed_set_cookie_line_is_recorded_and_dropped() {
    let server = MockServer::start().await;
    // A header with no name=value pair does not parse as a cookie.
    mount_page_with_set_cookie(&server, "/", "no-pair-here").await;
    let mut controller = NavigationController::new();

    controller
        .load(url_for(&server, "/"), NavigationSource::AddressBar)
        .await
        .expect("the page must load even when a Set-Cookie line is malformed");

    let records = controller.cookie_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].reason(), Some(RejectionReason::Malformed));
}

#[tokio::test]
async fn clear_cookies_empties_the_jar_and_the_records() {
    let server = MockServer::start().await;
    mount_page_with_set_cookie(&server, "/set", "sid=abc").await;
    mount_plain_page(&server, "/check").await;
    let mut controller = NavigationController::new();
    controller
        .set_site_cookie_policy(&server_host(&server), CookiePolicy::Session)
        .expect("setting a session exception must succeed");
    controller
        .load(url_for(&server, "/set"), NavigationSource::AddressBar)
        .await
        .expect("the cookie must be accepted");
    assert_eq!(controller.cookie_records().len(), 1);

    controller.clear_cookies();

    assert!(controller.cookie_records().is_empty());
    controller
        .load(url_for(&server, "/check"), NavigationSource::AddressBar)
        .await
        .expect("the page must load with an empty jar");
    let check_request = requests_to(&server, "/check").await;
    assert_eq!(
        cookie_header(&check_request[0]),
        None,
        "the cleared jar sends no cookie on a later request"
    );
}

#[tokio::test]
async fn an_mcp_shape_controller_rejects_every_cookie_by_default() {
    let server = MockServer::start().await;
    mount_page_with_set_cookie(&server, "/", "sid=abc").await;
    // new() is the MCP path: reject pair, empty jar, no store.
    let mut controller = NavigationController::new();

    controller
        .load(url_for(&server, "/"), NavigationSource::AddressBar)
        .await
        .expect("the page must load with cookies rejected");

    let records = controller.cookie_records();
    assert_eq!(records.len(), 1);
    assert!(!records[0].accepted());
    assert_eq!(controller.cookie_counts(), (0, 1));
}

#[tokio::test]
async fn a_site_exception_is_written_through_to_the_store() {
    let store = Arc::new(InMemorySitePolicyStore::default());
    let mut controller = NavigationController::new().with_cookies(
        CookiePolicyPair::default(),
        Some(store.clone()),
        Vec::new(),
    );

    controller
        .set_site_cookie_policy("www.example.com", CookiePolicy::Session)
        .expect("a write-through must succeed");

    let persisted = store
        .site_policy("example.com")
        .expect("the store read must succeed");
    assert_eq!(persisted.as_deref(), Some("session"));
}
