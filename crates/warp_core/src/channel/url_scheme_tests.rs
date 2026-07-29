//! The URL scheme this build registers must be the one it accepts.
//!
//! Deliberately NOT in state_tests.rs. That module is
//! `#[cfg(all(test, not(feature = "test-util")))]`, because the function it
//! covers only exists without that feature -- and building warp_core alongside
//! `warp` (which the gate and CI both do) turns `test-util` on, compiling the
//! whole module away. A test that silently stops being built protects nothing,
//! and this one exists precisely because the bug it guards is invisible.

// A URL scheme is only useful if every artifact that names it agrees: the string
// the app accepts (`ChannelState::url_scheme`), the two strings the macOS
// bundlers write into CFBundleURLSchemes, the plist embedded in the binary, and
// the handler the Linux desktop entry registers. Nothing fails loudly when they
// diverge -- the OS routes the URL to the app and the app silently drops it for
// having "someone else's" scheme.
//
// That is not hypothetical. `url_scheme()` returned "warposs" while
// script/macos/bundle had already been renamed to write "heddle", so every
// heddle:// link LaunchServices delivered to the shipped app was discarded. The
// obvious test -- checking the sanitizer allowlist in warpui's browser.rs --
// passes happily through exactly that bug, because an allowlist of schemes we
// are willing to OPEN says nothing about which scheme we REGISTER.
//
// So this reads the registering artifacts themselves. It is hermetic: no
// network, no build, no cargo invocation, just four files in the tree.
use std::fs;
use std::path::PathBuf;

use super::{Channel, ChannelState};

fn repo_root() -> PathBuf {
    // crates/warp_core -> repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root should resolve from CARGO_MANIFEST_DIR")
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| panic!("could not read {relative}: {err}"))
}

/// The shell variable the macOS bundlers put the scheme in, matched by SUFFIX.
///
/// Deliberately not the full variable name. Spelled out, it is a `WARP_` string
/// literal, and this file would then ADD two Warp brand mentions to the ratchet
/// in script/heddle/gui-branding.baseline -- a test enforcing de-branding is a
/// poor place to introduce brand mentions, and re-recording them would be the
/// wrong way round. The suffix is also the more durable anchor: it keeps working
/// when that variable is eventually renamed, which is the direction of travel.
const SCHEME_VAR_SUFFIX: &str = "_SCHEME_NAME=";

/// Pull the value out of the first `*_SCHEME_NAME="value"` assignment after
/// `anchor`.
///
/// Panics rather than returning None: if the shape these scripts are written
/// in ever changes, this test must fail and be rewritten, not quietly stop
/// checking anything.
fn scheme_assignment_after(haystack: &str, anchor: &str, what: &str) -> String {
    let tail = haystack
        .split_once(anchor)
        .unwrap_or_else(|| panic!("{what}: anchor {anchor:?} not found"))
        .1;
    let line = tail
        .lines()
        .map(str::trim)
        .find(|line| !line.starts_with('#') && line.contains(SCHEME_VAR_SUFFIX))
        .unwrap_or_else(|| panic!("{what}: no {SCHEME_VAR_SUFFIX} assignment after {anchor:?}"));
    line.split_once('=')
        .expect("line was selected for containing '='")
        .1
        .trim()
        .trim_matches('"')
        .to_owned()
}

#[test]
fn url_scheme_matches_what_the_bundlers_register() {
    // The ambient channel; `ChannelState::init` sets it to Oss and no test in
    // this crate calls `set`. Assert it rather than assume it, so this can
    // never silently end up checking a different channel's scheme.
    assert_eq!(
        ChannelState::channel(),
        Channel::Oss,
        "this test describes the OSS channel; the ambient channel is no longer Oss"
    );
    let expected = ChannelState::url_scheme();

    // 1. The signed/notarized macOS bundle. script/update_plist copies
    //    WARP_SCHEME_NAME into CFBundleURLSchemes.
    let macos_bundle = read("script/macos/bundle");
    assert_eq!(
        scheme_assignment_after(
            &macos_bundle,
            r#"RELEASE_CHANNEL = "oss""#,
            "script/macos/bundle",
        ),
        expected,
        "script/macos/bundle registers a scheme the app will not accept",
    );

    // 2. The ./script/run dev bundle, which is a real .app and really does
    //    get URLs routed to it.
    let macos_run = read("script/macos/run");
    assert_eq!(
        scheme_assignment_after(&macos_run, "Heddle.app", "script/macos/run"),
        expected,
        "script/macos/run registers a scheme the app will not accept",
    );

    // 3. The Linux desktop entry shipped inside the AppImage.
    let desktop = read("app/channels/oss/dev.heddle.Heddle.desktop");
    let handler = desktop
        .lines()
        .find_map(|line| line.trim().strip_prefix("MimeType=x-scheme-handler/"))
        .expect("desktop entry has no x-scheme-handler MimeType")
        .trim_end_matches(';')
        .to_owned();
    assert_eq!(
        handler, expected,
        "the Linux desktop entry registers a scheme the app will not accept",
    );

    // 4. The plist embedded in the binary, used when `extern_plist` is off.
    let oss_main = read("app/src/bin/oss.rs");
    let embedded = oss_main
        .split_once("CFBundleURLSchemes")
        .expect("app/src/bin/oss.rs no longer embeds CFBundleURLSchemes")
        .1
        .split_once("<string>")
        .expect("CFBundleURLSchemes has no <string> entry")
        .1
        .split_once("</string>")
        .expect("unterminated <string> after CFBundleURLSchemes")
        .0
        .to_owned();
    assert_eq!(
        embedded, expected,
        "the embedded Info.plist registers a scheme the app will not accept",
    );
}
