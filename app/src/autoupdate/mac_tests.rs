//! Tests for the checks that stand between a download and an install.
//!
//! Codex's review of the first cut noted, correctly, that there was "no test exercising
//! code-signature rejection, notarization rejection, or manifest-versus-bundle version
//! mismatch" -- the three things the feature advertises. These are those tests.
//!
//! They run real `codesign`, `spctl` and `plutil`, which is the point: the checks are
//! shell-outs, so a test that mocked them would verify the mock. macOS-only by construction.

use std::path::{Path, PathBuf};

use super::{staged_bundle_version, verify_bundle_is_newer, verify_code_signature};
use crate::channel::ChannelState;

/// Build a minimal `.app` on disk carrying `version`. Unsigned, which is exactly what the
/// signature test needs and irrelevant to the version tests.
fn fake_bundle(dir: &Path, version: &str) -> PathBuf {
    let app = dir.join("Heddle.app");
    std::fs::create_dir_all(app.join("Contents/MacOS")).expect("create bundle");
    std::fs::write(
        app.join("Contents/Info.plist"),
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundleIdentifier</key>
    <string>dev.heddle.Heddle</string>
</dict>
</plist>
"#
        ),
    )
    .expect("write Info.plist");
    app
}

#[tokio::test]
async fn the_bundle_version_is_read_from_the_payload_not_the_manifest() {
    let dir = tempfile::tempdir().expect("temp dir");
    let app = fake_bundle(dir.path(), "0.3.2");
    let version = staged_bundle_version(&app).await.expect("must read");
    assert_eq!(
        version, "0.3.2",
        "the version must come from the bundle's own Info.plist"
    );
}

#[tokio::test]
async fn an_older_staged_bundle_is_refused() {
    // The defect this closes: the manifest is a file on a host we do not control, and a
    // signature check cannot tell an old Heddle release from a new one -- an older release is
    // validly signed AND validly notarized. Before this check, a manifest naming v999.0.0
    // beside a genuinely-signed older payload passed everything and installed a downgrade.
    ChannelState::set_app_version(Some("v0.3.2"));
    let dir = tempfile::tempdir().expect("temp dir");
    let app = fake_bundle(dir.path(), "0.3.1");

    let result = verify_bundle_is_newer(&app).await;
    assert!(
        result.is_err(),
        "a staged bundle older than the running build must be refused, however well signed"
    );
}

#[tokio::test]
async fn an_identical_staged_bundle_is_refused() {
    ChannelState::set_app_version(Some("v0.3.2"));
    let dir = tempfile::tempdir().expect("temp dir");
    let app = fake_bundle(dir.path(), "0.3.2");

    assert!(
        verify_bundle_is_newer(&app).await.is_err(),
        "reinstalling the running version is not an update"
    );
}

#[tokio::test]
async fn a_newer_staged_bundle_is_accepted() {
    ChannelState::set_app_version(Some("v0.3.1"));
    let dir = tempfile::tempdir().expect("temp dir");
    let app = fake_bundle(dir.path(), "0.3.2");

    verify_bundle_is_newer(&app)
        .await
        .expect("a newer bundle must be accepted, or no update could ever install");
}

#[tokio::test]
async fn a_bundle_with_an_unreadable_version_is_refused() {
    // Fail closed. A payload whose version we cannot establish is one we cannot prove is
    // newer, and "cannot prove" must mean "do not install".
    ChannelState::set_app_version(Some("v0.3.1"));
    let dir = tempfile::tempdir().expect("temp dir");
    let app = dir.path().join("Heddle.app");
    std::fs::create_dir_all(app.join("Contents")).expect("create bundle");
    // No Info.plist at all.
    assert!(
        verify_bundle_is_newer(&app).await.is_err(),
        "a bundle with no readable version must be refused"
    );
}

#[tokio::test]
async fn a_bundle_carrying_an_unparseable_version_is_refused() {
    // Upstream's dated scheme, which `HeddleVersion::parse` refuses. A payload claiming it
    // must not be installed on the strength of a signature alone.
    ChannelState::set_app_version(Some("v0.3.1"));
    let dir = tempfile::tempdir().expect("temp dir");
    let app = fake_bundle(dir.path(), "v0.2026.07.26.18.00.stable_01");

    assert!(
        verify_bundle_is_newer(&app).await.is_err(),
        "an unparseable bundle version must be refused, not assumed newer"
    );
}

#[tokio::test]
async fn an_unsigned_bundle_fails_the_signature_check() {
    // Proves the signature check actually rejects, rather than passing everything handed to
    // it. `codesign -v` on an unsigned directory exits non-zero, so a check that ignored the
    // exit status -- or that was never wired up -- fails here.
    let dir = tempfile::tempdir().expect("temp dir");
    let app = fake_bundle(dir.path(), "0.3.2");

    assert!(
        verify_code_signature("bundle", &app).await.is_err(),
        "an unsigned bundle must not pass the Developer ID check"
    );
}

#[tokio::test]
async fn an_unsigned_bundle_fails_the_notarization_check() {
    // Notarization is a SEPARATE Apple gate from the signature: a build signed with a leaked
    // key that was never submitted to Apple passes `codesign` and must fail here. An unsigned
    // bundle is the case we can construct without credentials, and it exercises the same
    // assertion.
    let dir = tempfile::tempdir().expect("temp dir");
    let app = fake_bundle(dir.path(), "0.3.2");

    assert!(
        super::verify_notarization("bundle", &app).await.is_err(),
        "an un-notarized bundle must not pass the Gatekeeper check"
    );
}

/// The download path must run all three checks before reporting the payload ready.
///
/// The tests above prove each check rejects what it should. They do NOT prove the download
/// path calls them: deleting `verify_bundle_is_newer(&target).await?` from
/// `download_and_extract_app_zip` still compiles and still leaves every test above passing,
/// because they call the function directly. That gap is exactly how a guard becomes
/// decorative.
///
/// A source-level ratchet is coarse, but it fails on the edit that matters. Testing it
/// properly would need a signed, notarized bundle served over HTTPS, which is a release
/// rehearsal rather than a unit test.
#[test]
fn the_download_path_runs_all_three_checks() {
    let source = include_str!("mac.rs");
    let start = source
        .find("async fn download_and_extract_app_zip")
        .expect("the zip download path must exist");
    let body = &source[start..];
    let end = body
        .find("\n/// Apply the downloaded update.")
        .unwrap_or(body.len());
    let body = &body[..end];

    for required in [
        "verify_code_signature",
        "verify_notarization",
        "verify_bundle_is_newer",
    ] {
        assert!(
            body.contains(required),
            "download_and_extract_app_zip must call {required} before returning \
             DownloadReady::Yes; without it the check exists but nothing runs it"
        );
    }
}
