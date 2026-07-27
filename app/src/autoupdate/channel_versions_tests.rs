//! Tests for the manifest fetch.
//!
//! Every case runs with NO network. The two "no consent" cases deliberately leave
//! `HEDDLE_CHANNEL_VERSIONS_PATH` unset: if the consent gate were removed, they would fall
//! through to a real HTTPS request to github.com and fail (or hang) rather than passing
//! quietly. That is the point -- the assertion is backed by the absence of a local fallback,
//! not just by an `is_none()` check.

use serial_test::serial;

use super::fetch_channel_versions;
use crate::settings::UpdateConsent;

const MANIFEST: &str = r#"{
  "dev":     { "version": "v0.3.2" },
  "preview": { "version": "v0.3.2" },
  "stable":  { "version": "v0.3.2" }
}"#;

/// Write a manifest to a temp file and point `HEDDLE_CHANNEL_VERSIONS_PATH` at it for the
/// duration of `body`.
///
/// The env var is process-wide, so these tests are `#[serial]`. `cargo-nextest` runs each
/// test in its own process, which makes this safe under nextest regardless -- but the suite
/// must also behave under plain `cargo test`, where they share one.
fn with_manifest<T>(json: &str, body: impl FnOnce() -> T) -> T {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("channel_versions.json");
    std::fs::write(&path, json).expect("write manifest");
    // SAFETY: sets one process-wide variable and removes it before returning. Guarded by
    // `#[serial]` so no other test observes it.
    unsafe { std::env::set_var("HEDDLE_CHANNEL_VERSIONS_PATH", &path) };
    let out = body();
    unsafe { std::env::remove_var("HEDDLE_CHANNEL_VERSIONS_PATH") };
    out
}

/// Asserts the env var is not set, so a fetch would have to reach the network.
fn assert_no_local_manifest_configured() {
    assert!(
        std::env::var("HEDDLE_CHANNEL_VERSIONS_PATH").is_err(),
        "this test proves nothing if a local manifest is configured: the fetch would read \
         the file instead of attempting a request"
    );
}

#[tokio::test]
#[serial]
async fn unanswered_consent_makes_no_request_and_returns_none() {
    assert_no_local_manifest_configured();
    let got = fetch_channel_versions(UpdateConsent::Unanswered)
        .await
        .expect("not having asked must not be an error");
    assert!(
        got.is_none(),
        "an unanswered consent must not produce a manifest"
    );
}

#[tokio::test]
#[serial]
async fn declined_consent_makes_no_request_and_returns_none() {
    assert_no_local_manifest_configured();
    let got = fetch_channel_versions(UpdateConsent::Disabled)
        .await
        .expect("declining must not be an error");
    assert!(
        got.is_none(),
        "a declined consent must not produce a manifest"
    );
}

#[tokio::test]
#[serial]
async fn the_local_path_override_does_not_bypass_consent() {
    // The env var chooses WHERE the manifest comes from, never WHETHER we may look. Without
    // this ordering a staging override -- or a stray variable in someone's shell -- would
    // produce an update offer for a user who was never asked.
    let got = with_manifest(MANIFEST, || {
        futures::executor::block_on(fetch_channel_versions(UpdateConsent::Unanswered))
    })
    .expect("must not error");
    assert!(
        got.is_none(),
        "a local manifest must not override an unanswered consent"
    );
}

#[tokio::test]
#[serial]
async fn enabled_consent_reads_the_manifest() {
    let got = with_manifest(MANIFEST, || {
        futures::executor::block_on(fetch_channel_versions(UpdateConsent::Enabled))
    })
    .expect("must not error");
    let versions = got.expect("a manifest must be returned");
    assert_eq!(versions.stable.version_info().version, "v0.3.2");
}

#[tokio::test]
#[serial]
async fn a_malformed_manifest_is_an_error_not_a_panic() {
    let got = with_manifest("{ not json", || {
        futures::executor::block_on(fetch_channel_versions(UpdateConsent::Enabled))
    });
    assert!(
        got.is_err(),
        "malformed JSON must surface as an error the caller can swallow, not a panic"
    );
}

#[tokio::test]
#[serial]
async fn a_missing_manifest_file_is_an_error_not_a_panic() {
    // SAFETY: process-wide, removed immediately; guarded by `#[serial]`.
    unsafe {
        std::env::set_var(
            "HEDDLE_CHANNEL_VERSIONS_PATH",
            "/nonexistent/heddle/channel_versions.json",
        )
    };
    let got = fetch_channel_versions(UpdateConsent::Enabled).await;
    unsafe { std::env::remove_var("HEDDLE_CHANNEL_VERSIONS_PATH") };
    assert!(got.is_err(), "an unreadable manifest must be an error");
}
