# Update Notification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Heddle.app tells the user when a newer version exists and can install it, after
asking permission once, without ever installing something unsigned, un-notarized, or older.

**Architecture:** Almost everything already exists in `app/src/autoupdate/` (~4,600 lines,
inert because `autoupdate` is not a default feature). Four things are new: a semver
comparison that can actually order Heddle's tags, a plain-HTTPS manifest fetch replacing
one that went through Warp's removed `ServerApi`, a one-time consent gate, and two extra
checks (notarization, monotonicity) in front of install.

**Tech Stack:** Rust edition 2024, toolchain 1.92.0 (pinned by `rust-toolchain.toml`),
`serde`/`serde_json`, `reqwest` (already a workspace dependency), `cargo nextest`.

## Global Constraints

- **Toolchain is 1.92.0**, pinned in `rust-toolchain.toml`. Do not bump it.
- **`cargo test` covers only workspace default members, and `warp_core` is NOT one.** Always
  name crates: `cargo nextest run --locked -p warp -p warp_core -p warp_tui -p onboarding`.
- **No network in unit tests.** The manifest fetch is tested through
  `WARP_CHANNEL_VERSIONS_PATH`, which reads a local file and already exists.
- **Team ID is `4STAAHTNCN`** (`warp_core::macos::APPLE_TEAM_ID`). `2BBY89MBSN` is Warp's and
  appears only in `LEGACY_WARP_APP_GROUP_TEAM_ID` for data migration. Never use it here.
- **Manifest URL:** `https://github.com/nikolaypaskov/heddle/releases/latest/download/channel_versions.json`
- **No `curl … | sh` anywhere.** `.claudeconf/rules/heddle.yaml` fails the build on it.
- **Run the gate before pushing:** `lefthook run gate`. Run `script/heddle/codex-review`
  before opening the PR.
- Every failure mode except verification failure is **silent and non-blocking**. The app
  must behave exactly as if the feature were off.

---

## File Structure

| File | Responsibility |
| --- | --- |
| `app/src/autoupdate/heddle_version.rs` **(new)** | Parse and order `vMAJOR.MINOR.PATCH`. Nothing else. |
| `app/src/autoupdate/heddle_version_tests.rs` **(new)** | Its tests. |
| `app/src/autoupdate/channel_versions.rs` **(modify)** | Replace the `ServerApi` fetch with a plain HTTPS GET. |
| `app/src/autoupdate/channel_versions_tests.rs` **(new)** | Fetch tests, driven by `WARP_CHANNEL_VERSIONS_PATH`. |
| `app/src/settings/update_consent.rs` **(new)** | The tri-state consent setting and its accessor. |
| `app/src/settings/update_consent_tests.rs` **(new)** | Its tests. |
| `app/src/autoupdate/mac.rs` **(modify)** | Add the notarization assertion beside the existing signature check. |
| `app/src/autoupdate/mod.rs` **(modify)** | Gate the check on consent; gate install on monotonicity. |
| `.github/workflows/heddle-release.yml` **(modify)** | Publish `channel_versions.json`; assert it matches the tag. |
| `app/Cargo.toml` **(modify)** | Add `autoupdate` to the default feature list. |

Why `heddle_version.rs` is its own file: version ordering is the load-bearing part of the
downgrade protection, it is pure (no I/O, no context), and it is the one piece that must be
exhaustively tested. Keeping it separate from the 1,163-line `mod.rs` means a reviewer can
hold all of it in their head.

---

## Task 1: Version ordering that works on Heddle's tags

**Files:**
- Create: `app/src/autoupdate/heddle_version.rs`
- Create: `app/src/autoupdate/heddle_version_tests.rs`
- Modify: `app/src/autoupdate/mod.rs` (add the module declaration)

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct HeddleVersion { major: u64, minor: u64, patch: u64 }`,
  `HeddleVersion::parse(&str) -> Option<HeddleVersion>`, and `Ord`/`PartialOrd` impls.

**Why this task exists first.** `crates/channel_versions` already has a `ParsedVersion` with
an `Ord` impl, and it is a trap. Its regex is `v(\d+)\.(.+)\.(.+)_(\d+)` — Warp's dated
scheme, `v0.2026.07.26.18.00.stable_01`. Heddle tags `v0.3.1`, which **does not match**.
Reusing it would leave monotonicity resting on a parser that rejects our own versions. The
same mismatch already caused a real bug: `script/update_plist` only rewrote
`CFBundleShortVersionString` for the dated format, so every release before v0.3.1 shipped
claiming to be `0.1.0`.

- [ ] **Step 1: Write the failing tests**

Create `app/src/autoupdate/heddle_version_tests.rs`:

```rust
use super::heddle_version::HeddleVersion;

#[test]
fn parses_a_heddle_tag() {
    let v = HeddleVersion::parse("v0.3.1").expect("v0.3.1 must parse");
    assert_eq!((v.major(), v.minor(), v.patch()), (0, 3, 1));
}

#[test]
fn parses_without_the_v_prefix() {
    // CFBundleShortVersionString has no `v`; the manifest tag does. Accept both so the
    // caller never has to remember which side it is holding.
    let v = HeddleVersion::parse("0.3.1").expect("0.3.1 must parse");
    assert_eq!((v.major(), v.minor(), v.patch()), (0, 3, 1));
}

#[test]
fn orders_by_component_not_lexically() {
    let older = HeddleVersion::parse("v0.9.0").unwrap();
    let newer = HeddleVersion::parse("v0.10.0").unwrap();
    // Lexically "0.10.0" < "0.9.0"; numerically it is not. A string comparison here would
    // silently refuse every update after 0.9.
    assert!(newer > older, "0.10.0 must be newer than 0.9.0");
}

#[test]
fn equal_versions_are_not_newer() {
    let a = HeddleVersion::parse("v0.3.1").unwrap();
    let b = HeddleVersion::parse("v0.3.1").unwrap();
    assert!(!(a > b) && !(b > a), "identical versions must not order");
}

#[test]
fn rejects_warps_dated_scheme() {
    // Not an error to guard against an attacker -- a guard against US. If a manifest ever
    // carries upstream's format, this must refuse to parse rather than produce a number
    // that happens to compare.
    assert!(HeddleVersion::parse("v0.2026.07.26.18.00.stable_01").is_none());
}

#[test]
fn rejects_junk() {
    for junk in ["", "v", "v1", "v1.2", "v1.2.3.4", "va.b.c", "1.2.3-beta", "vX.Y.Z"] {
        assert!(
            HeddleVersion::parse(junk).is_none(),
            "{junk:?} must not parse: an unparseable version has to be treated as \
             'do not update', never as version zero"
        );
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run --locked -p warp heddle_version`
Expected: FAIL — `heddle_version` module does not exist.

- [ ] **Step 3: Write the implementation**

Create `app/src/autoupdate/heddle_version.rs`:

```rust
//! Ordering for Heddle's own version tags.
//!
//! `crates/channel_versions::ParsedVersion` exists and looks like it would do this job. It
//! will not: its regex is `v(\d+)\.(.+)\.(.+)_(\d+)`, which matches upstream's dated scheme
//! (`v0.2026.07.26.18.00.stable_01`) and rejects `v0.3.1`. Building the downgrade guard on
//! a parser that cannot read our own tags would defeat the guard.

/// A parsed `MAJOR.MINOR.PATCH` version, with or without a leading `v`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HeddleVersion {
    // Field order matters: the derived `Ord` compares these in declaration order, which is
    // exactly major-then-minor-then-patch.
    major: u64,
    minor: u64,
    patch: u64,
}

impl HeddleVersion {
    /// Parse `v0.3.1` or `0.3.1`. Returns `None` for anything else.
    ///
    /// Deliberately strict. A version that cannot be parsed must become "do not update",
    /// and a lenient parser that returns 0.0.0 for junk would turn every malformed manifest
    /// into an offer to "upgrade" from a real version to nothing.
    pub fn parse(raw: &str) -> Option<Self> {
        let raw = raw.strip_prefix('v').unwrap_or(raw);
        let mut parts = raw.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next()?.parse().ok()?;
        let patch = parts.next()?.parse().ok()?;
        // A fourth component means this is not our scheme.
        if parts.next().is_some() {
            return None;
        }
        Some(Self { major, minor, patch })
    }

    pub fn major(&self) -> u64 { self.major }
    pub fn minor(&self) -> u64 { self.minor }
    pub fn patch(&self) -> u64 { self.patch }
}

#[cfg(test)]
#[path = "heddle_version_tests.rs"]
mod tests;
```

Add to `app/src/autoupdate/mod.rs`, beside the other `mod` declarations near the top:

```rust
pub mod heddle_version;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo nextest run --locked -p warp heddle_version`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add app/src/autoupdate/heddle_version.rs app/src/autoupdate/heddle_version_tests.rs app/src/autoupdate/mod.rs
git commit -m "feat(autoupdate): version ordering that works on Heddle's semver tags"
```

---

## Task 2: The consent setting

**Files:**
- Create: `app/src/settings/update_consent.rs`
- Create: `app/src/settings/update_consent_tests.rs`
- Modify: `app/src/settings/mod.rs` (add the module declaration)

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `pub enum UpdateConsent { Unanswered, Enabled, Disabled }` and
  `UpdateConsent::should_check(&self) -> bool`.

**Why tri-state.** A boolean cannot distinguish "the user said no" from "we have not asked
yet", and those must behave the same (no network) but *look* different (one shows the
prompt, one never does again). Collapsing them is how a declined setting turns into a
prompt that reappears on every launch.

- [ ] **Step 1: Write the failing tests**

Create `app/src/settings/update_consent_tests.rs`:

```rust
use super::update_consent::UpdateConsent;

#[test]
fn the_default_is_unanswered() {
    // Critically NOT Enabled. The spec's whole premise is that no network request happens
    // until the user has answered.
    assert_eq!(UpdateConsent::default(), UpdateConsent::Unanswered);
}

#[test]
fn only_an_explicit_yes_permits_a_network_request() {
    assert!(!UpdateConsent::Unanswered.should_check());
    assert!(!UpdateConsent::Disabled.should_check());
    assert!(UpdateConsent::Enabled.should_check());
}

#[test]
fn round_trips_through_serde() {
    // The setting is persisted; a serialisation change that silently reset it to the
    // default would re-prompt everyone and re-enable nobody.
    for value in [UpdateConsent::Unanswered, UpdateConsent::Enabled, UpdateConsent::Disabled] {
        let json = serde_json::to_string(&value).expect("serialises");
        let back: UpdateConsent = serde_json::from_str(&json).expect("deserialises");
        assert_eq!(value, back, "{value:?} must survive a round trip");
    }
}

#[test]
fn an_unknown_persisted_value_falls_back_to_unanswered() {
    // Never fall back to Enabled: an unreadable setting must not become consent.
    let back: UpdateConsent = serde_json::from_str("\"nonsense\"").unwrap_or_default();
    assert_eq!(back, UpdateConsent::Unanswered);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run --locked -p warp update_consent`
Expected: FAIL — module does not exist.

- [ ] **Step 3: Write the implementation**

Create `app/src/settings/update_consent.rs`:

```rust
//! Whether the user has agreed to Heddle checking for new releases.
//!
//! Three states, not two. "Not asked yet" and "said no" both mean no network request, but
//! only the first should ever show the prompt.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum UpdateConsent {
    /// Never asked. Show the prompt once; make no network request until answered.
    #[default]
    Unanswered,
    /// The user agreed. Check on launch.
    Enabled,
    /// The user declined. Never check, never ask again.
    Disabled,
}

impl UpdateConsent {
    /// The single place that decides whether a network request is permitted.
    pub fn should_check(&self) -> bool {
        matches!(self, Self::Enabled)
    }
}

#[cfg(test)]
#[path = "update_consent_tests.rs"]
mod tests;
```

Add to `app/src/settings/mod.rs`, beside the other `mod` declarations:

```rust
pub mod update_consent;
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo nextest run --locked -p warp update_consent`
Expected: PASS, 4 tests.

- [ ] **Step 5: Commit**

```bash
git add app/src/settings/update_consent.rs app/src/settings/update_consent_tests.rs app/src/settings/mod.rs
git commit -m "feat(settings): tri-state update-check consent, defaulting to unanswered"
```

---

## Task 3: Fetch the manifest over plain HTTPS

**Files:**
- Modify: `app/src/autoupdate/channel_versions.rs`
- Create: `app/src/autoupdate/channel_versions_tests.rs`

**Interfaces:**
- Consumes: `UpdateConsent::should_check` (Task 2).
- Produces: `pub async fn fetch_channel_versions(consent: UpdateConsent) -> Result<Option<ChannelVersions>>`
  — `Ok(None)` means "no check was permitted or none was available", which is not an error.

**What is being replaced.** The current body calls
`server_api.fetch_channel_versions(...)`, i.e. Warp's `ServerApi`, which this fork removed.
The `WARP_CHANNEL_VERSIONS_PATH` early-return at the top of the function already exists —
keep it exactly as it is; it is what makes this testable without network.

**Before writing the request, confirm the reqwest API in this workspace** rather than
assuming: `grep -rn "reqwest::Client" --include='*.rs' app/src crates | head`. Match the
surrounding code's client construction and timeout handling.

- [ ] **Step 1: Write the failing tests**

Create `app/src/autoupdate/channel_versions_tests.rs`:

```rust
use super::*;
use crate::settings::update_consent::UpdateConsent;

/// Write a manifest to a temp file and point WARP_CHANNEL_VERSIONS_PATH at it.
fn with_manifest<T>(json: &str, body: impl FnOnce() -> T) -> T {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("channel_versions.json");
    std::fs::write(&path, json).expect("write manifest");
    // SAFETY: sets one process-wide variable and removes it immediately. Tests share a
    // process, hence the removal rather than leaving it set.
    unsafe { std::env::set_var("WARP_CHANNEL_VERSIONS_PATH", &path) };
    let out = body();
    unsafe { std::env::remove_var("WARP_CHANNEL_VERSIONS_PATH") };
    out
}

const MANIFEST: &str = r#"{
  "dev":     { "version": "v0.3.2" },
  "preview": { "version": "v0.3.2" },
  "stable":  { "version": "v0.3.2" }
}"#;

#[tokio::test]
async fn unanswered_consent_makes_no_request_and_returns_none() {
    // No WARP_CHANNEL_VERSIONS_PATH is set, so if this DID attempt a fetch it would try the
    // network -- and the unit tier forbids that. Returning None without touching anything
    // is the whole point.
    let got = fetch_channel_versions(UpdateConsent::Unanswered).await.expect("must not error");
    assert!(got.is_none(), "an unanswered consent must not produce a manifest");
}

#[tokio::test]
async fn declined_consent_makes_no_request_and_returns_none() {
    let got = fetch_channel_versions(UpdateConsent::Disabled).await.expect("must not error");
    assert!(got.is_none(), "a declined consent must not produce a manifest");
}

#[tokio::test]
async fn enabled_consent_reads_the_manifest() {
    let got = with_manifest(MANIFEST, || {
        futures::executor::block_on(fetch_channel_versions(UpdateConsent::Enabled))
    })
    .expect("must not error");
    let versions = got.expect("a manifest must be returned");
    assert_eq!(versions.stable.version_info().version, "v0.3.2");
}

#[tokio::test]
async fn a_malformed_manifest_is_an_error_not_a_panic() {
    let got = with_manifest("{ not json", || {
        futures::executor::block_on(fetch_channel_versions(UpdateConsent::Enabled))
    });
    assert!(got.is_err(), "malformed JSON must surface as an error the caller can swallow");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run --locked -p warp channel_versions`
Expected: FAIL — the signature takes `(nonce, server_api, include_changelogs, is_daily)`.

- [ ] **Step 3: Rewrite the function**

Replace the body of `fetch_channel_versions` in `app/src/autoupdate/channel_versions.rs`:

```rust
/// Where Heddle publishes its release manifest.
///
/// `releases/latest/download/<asset>` is resolved by GitHub to the asset on the current
/// release, so publishing one extra file per release is the entire infrastructure -- no
/// server, no hostname to operate, no secret to hold.
const MANIFEST_URL: &str =
    "https://github.com/nikolaypaskov/heddle/releases/latest/download/channel_versions.json";

/// Fetch the release manifest, if the user has agreed to that.
///
/// `Ok(None)` means no check was permitted -- not a failure. The caller treats every error
/// as "carry on as if the feature were off"; see the spec's error-handling section.
pub async fn fetch_channel_versions(
    consent: UpdateConsent,
) -> Result<Option<ChannelVersions>> {
    // Local override, for tests and for anyone wanting to point at a staging manifest.
    // Checked BEFORE consent so a test never needs to fake consent to exercise parsing.
    if let Ok(path) = env::var("WARP_CHANNEL_VERSIONS_PATH") {
        let path = shellexpand::tilde(&path);
        let raw = read_to_string::<&str>(&path)?;
        return Ok(Some(
            serde_json::from_str(&raw).context("Failed to parse channel versions JSON")?,
        ));
    }

    if !consent.should_check() {
        return Ok(None);
    }

    let response = reqwest::Client::new()
        .get(MANIFEST_URL)
        .timeout(FETCH_CHANNEL_VERSIONS_TIMEOUT)
        .send()
        .await
        .context("Failed to fetch the release manifest")?;

    let body = response
        .error_for_status()
        .context("Release manifest request returned an error status")?
        .text()
        .await
        .context("Failed to read the release manifest body")?;

    Ok(Some(
        serde_json::from_str(&body).context("Failed to parse channel versions JSON")?,
    ))
}
```

Remove the now-unused `ServerApi` import and the `nonce` / `include_changelogs` /
`is_daily` parameters. Fix the call sites the compiler names — do not guess at them; run
`cargo check --locked -p warp` and work the list.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo nextest run --locked -p warp channel_versions`
Expected: PASS, 4 tests.

- [ ] **Step 5: Confirm no network is reachable from the unit tier**

Run: `cargo nextest run --locked -p warp channel_versions -- --nocapture`
Expected: no DNS or connection errors in output. If any appear, a test is escaping to the
network and the consent gate is wrong.

- [ ] **Step 6: Commit**

```bash
git add app/src/autoupdate/channel_versions.rs app/src/autoupdate/channel_versions_tests.rs
git commit -m "feat(autoupdate): fetch the manifest over plain HTTPS, gated on consent"
```

---

## Task 4: Refuse anything unsigned, un-notarized, or older

**Files:**
- Modify: `app/src/autoupdate/mac.rs` (beside `verify_code_signature`, around line 309)
- Modify: `app/src/autoupdate/mod.rs` (the install path)
- Modify: `app/src/autoupdate/mod_tests.rs`

**Interfaces:**
- Consumes: `HeddleVersion` (Task 1).
- Produces: `async fn verify_notarization(component: &str, path: &Path) -> Result<()>` and
  `fn is_upgrade(running: &str, offered: &str) -> bool`.

**Why all three checks.** `codesign -R="certificate leaf[subject.OU] = 4STAAHTNCN"` proves
the bundle carries the project's Developer ID. It does **not** prove the build was
notarized (a separate Apple gate that would catch a leaked signing key never submitted to
Apple), and it does **not** prove the build is newer — an *older* Heddle release is validly
signed, so both signature checks pass on a downgrade.

This check deserves the scrutiny for a concrete reason: until 2026-07-26 it compared
against Warp's team identifier, so it would have accepted a Warp-signed update and rejected
a Heddle-signed one. It was dormant, so nothing broke and nothing caught it.

- [ ] **Step 1: Write the failing tests**

Add to `app/src/autoupdate/mod_tests.rs`:

```rust
use super::heddle_version::HeddleVersion;
use super::is_upgrade;

#[test]
fn only_a_strictly_greater_version_is_an_upgrade() {
    assert!(is_upgrade("v0.3.1", "v0.3.2"), "0.3.2 over 0.3.1 is an upgrade");
    assert!(is_upgrade("v0.3.1", "v0.4.0"), "0.4.0 over 0.3.1 is an upgrade");
    assert!(is_upgrade("v0.9.0", "v0.10.0"), "0.10.0 over 0.9.0 is an upgrade");
}

#[test]
fn the_same_version_is_not_an_upgrade() {
    assert!(!is_upgrade("v0.3.1", "v0.3.1"));
}

#[test]
fn an_older_version_is_never_an_upgrade() {
    // The downgrade guard. An older release is VALIDLY SIGNED, so both signature checks
    // pass on it; this is the only thing standing between a user and being walked back to
    // a build with a known bug.
    assert!(!is_upgrade("v0.3.2", "v0.3.1"));
    assert!(!is_upgrade("v0.10.0", "v0.9.0"));
}

#[test]
fn an_unparseable_version_on_either_side_is_never_an_upgrade() {
    // Fail closed. If we cannot tell which is newer, we do not install.
    assert!(!is_upgrade("v0.3.1", "garbage"));
    assert!(!is_upgrade("garbage", "v0.3.2"));
    assert!(!is_upgrade("v0.2026.07.26.18.00.stable_01", "v0.3.2"));
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo nextest run --locked -p warp is_upgrade`
Expected: FAIL — `is_upgrade` does not exist.

- [ ] **Step 3: Implement `is_upgrade`**

Add to `app/src/autoupdate/mod.rs`:

```rust
use crate::autoupdate::heddle_version::HeddleVersion;

/// Whether `offered` is strictly newer than `running`.
///
/// Fails closed: if either side cannot be parsed, the answer is `false`. An unknown version
/// must never be treated as an upgrade, because the signature checks cannot tell old from
/// new -- an older Heddle release carries a perfectly valid signature.
pub(crate) fn is_upgrade(running: &str, offered: &str) -> bool {
    match (HeddleVersion::parse(running), HeddleVersion::parse(offered)) {
        (Some(running), Some(offered)) => offered > running,
        _ => false,
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo nextest run --locked -p warp is_upgrade`
Expected: PASS, 4 tests.

- [ ] **Step 5: Add the notarization assertion**

Add to `app/src/autoupdate/mac.rs`, directly below `verify_code_signature`:

```rust
/// Assert that Apple notarized this build, not merely that it is signed.
///
/// `codesign` proves the bundle carries our Developer ID. Notarization is a SEPARATE Apple
/// gate: a build signed with a leaked key that was never submitted to Apple passes
/// `codesign` and fails here. `spctl` is what Gatekeeper itself consults, so this asks the
/// same question the user's Mac asks on first launch.
async fn verify_notarization(component: &str, path: &Path) -> Result<()> {
    let output = Command::new("/usr/sbin/spctl")
        .arg("-a")
        .arg("-vv")
        .arg("-t")
        .arg("exec")
        .arg(path)
        .output()
        .await?;

    // spctl writes its assessment to stderr.
    let assessment = String::from_utf8_lossy(&output.stderr);
    ensure!(
        output.status.success() && assessment.contains("source=Notarized Developer ID"),
        "Staged update for {component} is not notarized: {assessment}"
    );

    safe_info!(
        safe: ("Notarization is valid for {component}"),
        full: ("Notarization is valid for {}", path.display())
    );
    Ok(())
}
```

Call it immediately after the existing `verify_code_signature(...)` call in the same file.
Find that call with `grep -n "verify_code_signature(" app/src/autoupdate/mac.rs` — both
must pass before anything is installed.

- [ ] **Step 6: Gate the install on `is_upgrade`**

In `app/src/autoupdate/mod.rs`, find where a fetched version becomes an available update
(`grep -n "UpdateAvailable" app/src/autoupdate/mod.rs`). Before emitting it:

```rust
let running = warp_core::channel::ChannelState::app_version().unwrap_or("");
if !is_upgrade(running, &offered_version) {
    // Not newer, not parseable, or a downgrade. Silently do nothing -- this is the
    // ordinary "you are up to date" path as well as the guard.
    return Ok(());
}
```

- [ ] **Step 7: Run the full suite**

Run: `cargo nextest run --locked -p warp -p warp_core -p warp_tui -p onboarding`
Expected: PASS. The suite was 5,607/0 before this work; it must still be.

- [ ] **Step 8: Commit**

```bash
git add app/src/autoupdate/mac.rs app/src/autoupdate/mod.rs app/src/autoupdate/mod_tests.rs
git commit -m "feat(autoupdate): require notarization and refuse downgrades"
```

---

## Task 5: Publish the manifest, and turn the feature on

**Files:**
- Modify: `.github/workflows/heddle-release.yml`
- Modify: `app/Cargo.toml` (default features)

**Interfaces:**
- Consumes: everything above.
- Produces: a `channel_versions.json` asset on every release.

- [ ] **Step 1: Add the manifest generation step**

In `.github/workflows/heddle-release.yml`, in the `publish` job, before the `gh release`
commands:

```yaml
      - name: Generate the release manifest
        env:
          # Interpolate through the environment, never directly into a run: block.
          RELEASE_TAG: ${{ inputs.tag || github.ref_name }}
        run: |
          set -euo pipefail
          # Heddle ships one channel. All three are published pointing at the same release
          # so the manifest keeps the shape crates/channel_versions expects.
          jq -n --arg v "$RELEASE_TAG" '{
            dev:     { version: $v },
            preview: { version: $v },
            stable:  { version: $v }
          }' > dist/channel_versions.json
          cat dist/channel_versions.json

      - name: Assert the manifest matches the tag
        env:
          RELEASE_TAG: ${{ inputs.tag || github.ref_name }}
        run: |
          set -euo pipefail
          # A manifest that disagrees with the tag would offer users a version that is not
          # the one attached to this release.
          published="$(jq -r .stable.version dist/channel_versions.json)"
          if [ "$published" != "$RELEASE_TAG" ]; then
            echo "::error::manifest says $published, tag is $RELEASE_TAG"
            exit 1
          fi
```

Add `dist/channel_versions.json` to the file list in both the `gh release upload` and
`gh release create` commands.

- [ ] **Step 2: Verify the workflow still audits clean**

Run: `actionlint && zizmor --persona=regular .github/workflows/`
Expected: actionlint exits 0; zizmor reports no findings.

- [ ] **Step 3: Enable the feature**

In `app/Cargo.toml`, add `"autoupdate"` to the `default` feature list.

- [ ] **Step 4: Confirm the GUI still builds with it on**

Run: `cargo check --locked -p warp --features gui`
Expected: 0 errors. This is the first time `autoupdate` has been compiled in this fork —
expect to fix references to removed server types, and fix them by deleting the dead path
rather than by reintroducing a server call.

- [ ] **Step 5: Run the full gate**

Run: `lefthook run gate`
Expected: all jobs pass. If the surface gate reports additions, they are new Warp strings
compiled in by the feature — read each one before recording it.

- [ ] **Step 6: Independent review before the PR**

Run: `script/heddle/codex-review`

This feature is the reason that gate exists: it makes a signature check load-bearing for
the first time, and that check trusted the wrong team ID until recently.

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/heddle-release.yml app/Cargo.toml
git commit -m "feat(release): publish the version manifest and enable update checks"
```

---

## Self-Review

**Spec coverage.** Ask once on first launch → Task 2 (the tri-state setting). *The prompt UI
itself is NOT in this plan* — see the gap below. One-click update → Tasks 3–5. GUI only →
no TUI file is touched. Trust root → Task 4 (all three checks). Manifest hosting → Task 5.
Error handling → Task 3 returns `Ok(None)` rather than erroring, and Task 4 fails closed.
Testing → every listed test in the spec's table appears in Tasks 1–4.

**Known gap, stated rather than hidden.** The spec's first-run notice needs a UI component,
and this plan does not build one. It builds everything the notice needs — a setting to
write, and a fetch that stays silent until it is written — but the notice itself depends on
where it lands in the workspace view, which was not settled in the design. **Do not mark
this feature complete without it:** with the setting stuck at `Unanswered`, every task above
is correct and the feature does nothing. Settle the placement, then extend this plan.

**Type consistency.** `HeddleVersion::parse` returns `Option<HeddleVersion>` in Task 1 and is
consumed that way in Task 4. `UpdateConsent::should_check(&self) -> bool` in Task 2 is called
that way in Task 3. `fetch_channel_versions(consent) -> Result<Option<ChannelVersions>>` in
Task 3 matches the `Ok(None)` handling described in Task 4.

**Placeholders.** None. Every code step contains the code.
