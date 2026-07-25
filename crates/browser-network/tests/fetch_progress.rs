// @file crates/browser-network/tests/fetch_progress.rs
// @description Tests for fetch_with_progress: byte-count channel updates and dropped-receiver safety.
// @layer network
// @created meerita <meerita@icloud.com>

use browser_network::{fetch_with_progress, BrowserUrl};
use tokio::sync::watch;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn url_for(server: &MockServer, suffix: &str) -> BrowserUrl {
    let full = format!("{}{}", server.uri(), suffix);
    BrowserUrl::parse(&full).expect("mock server URL must parse")
}

#[tokio::test]
async fn progress_channel_receives_increasing_byte_counts() {
    let server = MockServer::start().await;
    let body = vec![b'x'; 512];
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(body.clone(), "text/html"))
        .mount(&server)
        .await;

    let url = url_for(&server, "/");
    let (progress_tx, mut observer_rx) = watch::channel(0usize);

    // Collect every distinct value the sender emits; the loop exits when the sender
    // is dropped (fetch_with_progress returns and drops progress_tx).
    let observer = tokio::spawn(async move {
        let mut samples = vec![*observer_rx.borrow()];
        while observer_rx.changed().await.is_ok() {
            samples.push(*observer_rx.borrow());
        }
        samples
    });

    fetch_with_progress(&url, progress_tx)
        .await
        .expect("fetch must succeed");

    let samples = observer.await.expect("observer task must complete");
    assert!(
        !samples.is_empty(),
        "at least one progress sample must be observed"
    );
    assert_eq!(
        *samples.last().expect("samples must be non-empty"),
        body.len(),
        "final progress value must equal body length"
    );
    for window in samples.windows(2) {
        assert!(
            window[1] >= window[0],
            "progress values must be non-decreasing"
        );
    }
}

#[tokio::test]
async fn fetch_with_no_receivers_does_not_panic() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/"))
        .respond_with(ResponseTemplate::new(200).set_body_raw("<html></html>", "text/html"))
        .mount(&server)
        .await;

    let url = url_for(&server, "/");
    let (progress_tx, progress_rx) = watch::channel(0usize);
    drop(progress_rx);

    fetch_with_progress(&url, progress_tx)
        .await
        .expect("fetch must succeed even when the receiver is dropped before the call");
}
