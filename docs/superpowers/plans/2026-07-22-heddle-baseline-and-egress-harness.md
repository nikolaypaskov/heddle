# Heddle: Baseline Build & Egress Verification Harness — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove the OSS build compiles, build a harness that *observes* its network egress, use that harness to demonstrate the phone-home leak, then close the leaks that are knowable without compiler feedback.

**Architecture:** Test-first at the system level. Task 2 builds an egress observer and runs it against the *unmodified* OSS build, where it must FAIL — that failure is the proof the leak exists. Tasks 3–6 close leaks at their source-of-truth choke points until it passes. Every override is gated on `Channel::Oss` so the diff stays additive and upstream's other channels keep their tests green.

**Tech Stack:** Rust 1.92.0, cargo, reqwest, Docker (Linux container for egress observation), strace.

## Scope

This plan covers spec Phase 1 (baseline build) and Phase 3 (verification harness), plus the parts
of Phase 2 that are knowable today.

**Deliberately excluded:** the `ChannelConfig::{server_config,oz_config}` → `Option` migration.
That change's blast radius is a compile-error inventory that cannot exist until the workspace has
compiled once (Task 1). It gets its own plan, written against the real inventory produced by
Task 7.

Spec: `docs/superpowers/specs/2026-07-22-foss-fork-design.md`

## Global Constraints

- **Rust toolchain: 1.92.0**, pinned by `rust-toolchain.toml`. Do not change it.
- **License: AGPL-3.0.** Do not add dependencies incompatible with AGPL-3.0.
- **Never edit telemetry or entitlement call sites.** ~340 files reference telemetry and ~240
  reference entitlements. Changes go in source-of-truth functions ONLY, so call sites stay
  byte-identical to upstream and rebase cleanly.
- **No inline test modules.** `script/check_no_inline_test_modules` enforces this. Tests live in a
  sibling `<name>_tests.rs` file, wired with:
  ```rust
  #[cfg(all(test, not(target_family = "wasm")))]
  #[path = "<name>_tests.rs"]
  mod tests;
  ```
- **`./script/presubmit` must pass** before every commit: `./script/format --check`, the inline
  test module check, and `cargo clippy --workspace --all-targets --tests -- -D warnings`.
- **Gate overrides on `Channel::Oss`**, via `warp_core::channel::ChannelState::channel()`, not on
  `cfg!` features. Runtime gating keeps a single binary honest and works in both front-ends.
- **Commit after every task** using conventional commits.
- Work on branch `foss-fork-design` (already checked out).

---

### Task 1: Rust toolchain and baseline OSS build

Establishes that 1.58M lines compile here *before* any fork changes. Nothing else in this plan is
meaningful until this passes.

**Files:**
- Create: none
- Modify: none

**Interfaces:**
- Consumes: nothing
- Produces: a built `warp-tui-oss` binary at `target/debug/warp-tui-oss`, used as the egress
  target by every later task.

- [ ] **Step 1: Install rustup and the pinned toolchain**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
rustup show
```

Expected: `rustup show` reports active toolchain `1.92.0`, read from `rust-toolchain.toml`.

- [ ] **Step 2: Install build prerequisites**

```bash
brew install jq clang-format pkgconf llvm
```

Skip `sentry-cli`, `create-dmg`, `multitime`, `powershell`, and Docker Desktop for now — they are
needed for bundling and linting, not for `cargo build`. Do NOT run `./script/bootstrap`: it
requires sudo, switches your active Xcode, and installs a large toolchain we do not yet need.

- [ ] **Step 3: Build the OSS TUI binary**

```bash
cd /Users/npaskov/Development/warp
cargo build --bin warp-tui-oss 2>&1 | tail -30
```

Expected: `Finished` with exit 0. This is a cold build of a very large workspace — allow 20–60
minutes. If it fails, STOP and record the exact error; the whole plan depends on this.

- [ ] **Step 4: Verify the binary runs and identifies as the OSS channel**

```bash
ls -la target/debug/warp-tui-oss
./target/debug/warp-tui-oss --help 2>&1 | head -20
```

Expected: the binary exists and prints usage without panicking.

- [ ] **Step 5: Record the baseline**

```bash
cargo --version > docs/superpowers/plans/baseline-build.txt
rustc --version >> docs/superpowers/plans/baseline-build.txt
echo "warp-tui-oss built OK: $(date -u +%Y-%m-%dT%H:%M:%SZ)" >> docs/superpowers/plans/baseline-build.txt
git add docs/superpowers/plans/baseline-build.txt
git commit -m "chore: record baseline OSS build environment"
```

---

### Task 2: Egress observation harness (must FAIL against unmodified OSS)

Observes `connect()` syscalls directly, so it catches every egress path — HTTP, websockets,
Sentry, Firebase — not just `http_client`. Runs in a Linux container so it works identically on
your Mac and in CI.

**Files:**
- Create: `script/heddle/egress-test`
- Create: `docker/heddle-egress/Dockerfile`
- Test: the script IS the test.

**Interfaces:**
- Consumes: `target/debug/warp-tui-oss` from Task 1 (rebuilt inside the container for Linux).
- Produces: `script/heddle/egress-test`, exit 0 when zero non-loopback connections are observed,
  exit 1 otherwise. Writes observed destinations to `target/heddle-egress.log`.

- [ ] **Step 1: Write the container that observes syscalls**

Create `docker/heddle-egress/Dockerfile`:

```dockerfile
FROM rust:1.92.0-bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
        strace ca-certificates pkg-config libssl-dev cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
ENTRYPOINT ["/bin/bash", "-c"]
```

- [ ] **Step 2: Write the egress test script**

Create `script/heddle/egress-test`:

```bash
#!/bin/bash
#
# Observes every outbound connect() the OSS build attempts on a cold start with
# no user configuration. Exits non-zero if any non-loopback destination is
# contacted. This is Heddle's core privacy guarantee, enforced mechanically.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../.. && pwd)"
cd "${REPO_ROOT}"

IMAGE="heddle-egress:local"
LOG="target/heddle-egress.log"
RUN_SECONDS="${HEDDLE_EGRESS_RUN_SECONDS:-20}"

mkdir -p target

echo "Building observation image..."
docker build -q -t "${IMAGE}" docker/heddle-egress >/dev/null

echo "Building and tracing warp-tui-oss (cold start, empty HOME)..."
docker run --rm \
    --cap-add=SYS_PTRACE \
    -v "${REPO_ROOT}:/src" \
    -v heddle-cargo-registry:/usr/local/cargo/registry \
    -v heddle-target-linux:/src/target-linux \
    "${IMAGE}" \
    "set -e
     export CARGO_TARGET_DIR=/src/target-linux
     cargo build --bin warp-tui-oss
     export HOME=/tmp/heddle-empty-home
     mkdir -p \$HOME
     timeout ${RUN_SECONDS} strace -f -qq -e trace=connect \
        -o /src/${LOG} \
        \$CARGO_TARGET_DIR/debug/warp-tui-oss || true" \
    2>&1 | tail -20

echo
echo "Analysing observed connections..."

# strace renders AF_INET/AF_INET6 destinations with an inet_addr/inet_pton field.
# Loopback and AF_UNIX are expected and ignored; anything else is egress.
VIOLATIONS="$(grep -E 'connect\(.*(AF_INET|AF_INET6)' "${LOG}" 2>/dev/null \
    | grep -vE 'inet_addr\("127\.|inet_pton\(AF_INET6, "::1"' \
    || true)"

if [[ -n "${VIOLATIONS}" ]]; then
    echo "FAIL: the OSS build attempted outbound connections:"
    echo "${VIOLATIONS}" | sed 's/^/  /' | head -40
    echo
    echo "Full trace: ${LOG}"
    exit 1
fi

echo "PASS: zero non-loopback connections observed in ${RUN_SECONDS}s cold start."
```

Then:

```bash
chmod +x script/heddle/egress-test
```

- [ ] **Step 3: Run it against the UNMODIFIED build and confirm it FAILS**

```bash
./script/heddle/egress-test
```

Expected: **exit 1**, listing connections to Warp infrastructure. This failure is the deliverable —
it is the evidence that the leak is real and that the observer detects it. If it PASSES here, the
observer is broken (most likely the trace file is empty or the binary exited immediately); debug
that before continuing, because a test that cannot fail proves nothing.

- [ ] **Step 4: Record the observed baseline leak**

```bash
cp target/heddle-egress.log docs/superpowers/plans/egress-baseline.log
git add script/heddle/egress-test docker/heddle-egress/Dockerfile docs/superpowers/plans/egress-baseline.log
git commit -m "test: add egress observation harness, capture baseline leak

Observes connect() syscalls of a cold-start warp-tui-oss in a Linux
container. Currently FAILS against the unmodified OSS build, which is
the point: it documents the phone-home behaviour we intend to remove."
```

---

### Task 3: Deny non-allowlisted egress in `http_client` under `Channel::Oss`

Closes the largest single egress path. This is defence in depth — the hard guarantee comes from the
`Option` migration in the follow-up plan — but it stops the common case immediately.

**Files:**
- Modify: `crates/http_client/src/lib.rs` (add guard; call it from the five verb methods at
  lines 216–244)
- Test: `crates/http_client/src/lib_tests.rs` (exists; append)

**Interfaces:**
- Consumes: `warp_core::channel::{Channel, ChannelState}` (already imported at `lib.rs:23`).
- Produces: `pub fn is_egress_permitted(url: &reqwest::Url) -> bool` in `http_client`, used by
  later tasks and by the CI gate.

- [ ] **Step 1: Write the failing test**

Append to `crates/http_client/src/lib_tests.rs`:

```rust
#[test]
fn oss_channel_denies_warp_infrastructure() {
    let url = reqwest::Url::parse("https://app.warp.dev/graphql/v2").unwrap();
    assert!(
        !crate::is_egress_permitted_for_channel(&url, warp_core::channel::Channel::Oss),
        "OSS builds must not contact Warp infrastructure"
    );
}

#[test]
fn oss_channel_permits_loopback() {
    let url = reqwest::Url::parse("http://127.0.0.1:8080/health").unwrap();
    assert!(crate::is_egress_permitted_for_channel(
        &url,
        warp_core::channel::Channel::Oss
    ));
}

#[test]
fn oss_channel_permits_user_configured_third_party() {
    // A user's own model provider is their choice, not a phone-home.
    let url = reqwest::Url::parse("https://api.anthropic.com/v1/messages").unwrap();
    assert!(crate::is_egress_permitted_for_channel(
        &url,
        warp_core::channel::Channel::Oss
    ));
}

#[test]
fn non_oss_channels_are_unaffected() {
    let url = reqwest::Url::parse("https://app.warp.dev/graphql/v2").unwrap();
    assert!(crate::is_egress_permitted_for_channel(
        &url,
        warp_core::channel::Channel::Stable
    ));
}
```

- [ ] **Step 2: Run it and verify it fails**

```bash
cargo test -p http_client is_egress 2>&1 | tail -20
```

Expected: FAIL — `cannot find function `is_egress_permitted_for_channel` in this scope`.

- [ ] **Step 3: Implement the guard**

Add to `crates/http_client/src/lib.rs`, immediately before `impl Client {` at line 138:

```rust
/// Hosts that Heddle's OSS build must never contact. These are Warp's own
/// first-party services; contacting any of them would defeat the point of the
/// fork. Third-party hosts are permitted because they are only ever reached as
/// a result of explicit user configuration (e.g. the user's own model provider).
const DENIED_OSS_HOST_SUFFIXES: &[&str] = &[
    "warp.dev",
    "rudderstack.com",
    "rudderlabs.com",
    "sentry.io",
    "firebaseio.com",
    "googleapis.com",
    "identitytoolkit.googleapis.com",
];

/// Whether `url` may be contacted from `channel`.
///
/// Only [`Channel::Oss`] is restricted; all other channels are upstream's
/// concern and are left untouched so their tests keep passing.
pub fn is_egress_permitted_for_channel(url: &reqwest::Url, channel: Channel) -> bool {
    if channel != Channel::Oss {
        return true;
    }
    let Some(host) = url.host_str() else {
        return true;
    };
    !DENIED_OSS_HOST_SUFFIXES
        .iter()
        .any(|denied| host == *denied || host.ends_with(&format!(".{denied}")))
}

/// Whether `url` may be contacted from the running channel.
pub fn is_egress_permitted(url: &reqwest::Url) -> bool {
    is_egress_permitted_for_channel(url, ChannelState::channel())
}
```

- [ ] **Step 4: Run the tests and verify they pass**

```bash
cargo test -p http_client is_egress 2>&1 | tail -20
```

Expected: PASS, 4 tests.

- [ ] **Step 5: Enforce the guard on every request verb**

In `crates/http_client/src/lib.rs`, add this helper inside `impl Client` (immediately before
`pub fn get` at line 216):

```rust
    /// Panics in debug builds and logs in release builds when a request would
    /// violate the OSS egress policy. Returns whether the request may proceed.
    fn check_egress<U: IntoUrl + Clone>(url: U) -> bool {
        let Ok(parsed) = url.into_url() else {
            return true;
        };
        if is_egress_permitted(&parsed) {
            return true;
        }
        debug_assert!(
            false,
            "OSS egress policy violation: {}",
            parsed.host_str().unwrap_or("<no host>")
        );
        log::error!(
            "Blocked OSS egress to {}",
            parsed.host_str().unwrap_or("<no host>")
        );
        false
    }
```

Then in each of `get`, `post`, `put`, `patch`, and `delete` (lines 216–244), insert this as the
first line of the body. Shown for `get`; repeat verbatim in the other four:

```rust
    pub fn get<U: IntoUrl + Clone>(&self, url: U) -> RequestBuilder<'_> {
        let _permitted = Self::check_egress(url.clone());
        let include_warp_headers = Self::include_warp_http_headers(url.clone());
        let iap_token = self.iap_token_for(url.clone());
        self.builder(self.wrapped.get(url), include_warp_headers, iap_token)
    }
```

Note: this logs and asserts but does not yet abort the request, because `RequestBuilder` has no
error variant. Hard blocking arrives with the `Option` migration, which removes the URLs entirely.
This step exists so violations are *loud* during development.

- [ ] **Step 6: Verify the workspace still builds and lints clean**

```bash
cargo build --bin warp-tui-oss 2>&1 | tail -5
cargo clippy -p http_client --all-targets --tests -- -D warnings 2>&1 | tail -5
./script/format
```

Expected: all succeed.

- [ ] **Step 7: Commit**

```bash
git add crates/http_client/src/lib.rs crates/http_client/src/lib_tests.rs
git commit -m "feat(http_client): deny Warp infrastructure egress on OSS channel

Adds is_egress_permitted_for_channel() and wires it into all five request
verbs. Logs and debug-asserts on violation; hard blocking comes with the
ChannelConfig Option migration."
```

---

### Task 4: Telemetry policy always disabled on OSS

Removes the server-controlled override of the user's telemetry opt-out.

**Files:**
- Modify: `app/src/settings/privacy.rs:202`
- Create: `app/src/settings/privacy_tests.rs`

**Interfaces:**
- Consumes: `warp_core::channel::{Channel, ChannelState}`.
- Produces: `PrivacySettingsSnapshot::should_disable_telemetry()` returning `true` on OSS
  regardless of flags, settings, or server state. Signature unchanged: `fn(&self) -> bool`.

- [ ] **Step 1: Write the failing test**

Create `app/src/settings/privacy_tests.rs`:

```rust
use warp_core::channel::Channel;

use super::PrivacySettingsSnapshot;

#[test]
fn oss_disables_telemetry_even_when_force_enabled() {
    // `mock()` sets is_telemetry_enabled, is_telemetry_force_enabled and
    // should_collect_ai_ugc_telemetry all to true — the worst case.
    let snapshot = PrivacySettingsSnapshot::mock();
    assert!(
        snapshot.should_disable_telemetry_for_channel(Channel::Oss),
        "OSS builds must disable telemetry even when the server force-enables it"
    );
}

#[test]
fn non_oss_channels_retain_upstream_behaviour() {
    let snapshot = PrivacySettingsSnapshot::mock();
    assert!(
        !snapshot.should_disable_telemetry_for_channel(Channel::Stable),
        "upstream behaviour must be unchanged on non-OSS channels"
    );
}
```

Wire it up by appending to the end of `app/src/settings/privacy.rs`:

```rust
#[cfg(all(test, not(target_family = "wasm")))]
#[path = "privacy_tests.rs"]
mod tests;
```

- [ ] **Step 2: Run it and verify it fails**

```bash
cargo test -p warp --lib settings::privacy 2>&1 | tail -20
```

Expected: FAIL — no method `should_disable_telemetry_for_channel`.

- [ ] **Step 3: Implement**

In `app/src/settings/privacy.rs`, replace `should_disable_telemetry` (lines 202–207) with:

```rust
    pub fn should_disable_telemetry(&self) -> bool {
        self.should_disable_telemetry_for_channel(warp_core::channel::ChannelState::channel())
    }

    /// Heddle: OSS builds never send telemetry. Upstream allows a user's opt-out
    /// to be overridden by `is_telemetry_force_enabled` (set from team/server
    /// data) or by the server-driven `AgentModeAnalytics` experiment. On the OSS
    /// channel neither override applies.
    pub fn should_disable_telemetry_for_channel(
        &self,
        channel: warp_core::channel::Channel,
    ) -> bool {
        if channel == warp_core::channel::Channel::Oss {
            return true;
        }
        !self.is_telemetry_enabled
            && !self.is_telemetry_force_enabled
            && !FeatureFlag::AgentModeAnalytics.is_enabled()
    }
```

`PrivacySettingsSnapshot::mock()` is currently `#[cfg(test)]` only, which is what these tests need
— no change required.

- [ ] **Step 4: Run the tests and verify they pass**

```bash
cargo test -p warp --lib settings::privacy 2>&1 | tail -20
```

Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
./script/format
git add app/src/settings/privacy.rs app/src/settings/privacy_tests.rs
git commit -m "feat(privacy): always disable telemetry on OSS channel

Upstream lets is_telemetry_force_enabled or the server-driven
AgentModeAnalytics experiment override a user's telemetry opt-out. OSS
builds now short-circuit both."
```

---

### Task 5: Never initialize the telemetry collector on OSS

Stops the background flush threads and the disk-persisted event replay.

**Files:**
- Modify: `app/src/server/telemetry/collector.rs` (`initialize_telemetry_collection`, line ~45)
- Test: `app/src/server/telemetry/collector_tests.rs` (create)

**Interfaces:**
- Consumes: `warp_core::channel::{Channel, ChannelState}` (`ChannelState` already imported in this
  file).
- Produces: `TelemetryCollector::should_collect_for_channel(channel: Channel) -> bool`.

- [ ] **Step 1: Write the failing test**

Create `app/src/server/telemetry/collector_tests.rs`:

```rust
use warp_core::channel::Channel;

use super::TelemetryCollector;

#[test]
fn oss_never_collects() {
    assert!(!TelemetryCollector::should_collect_for_channel(Channel::Oss));
}

#[test]
fn stable_still_collects() {
    assert!(TelemetryCollector::should_collect_for_channel(
        Channel::Stable
    ));
}
```

Append to `app/src/server/telemetry/collector.rs`:

```rust
#[cfg(all(test, not(target_family = "wasm")))]
#[path = "collector_tests.rs"]
mod tests;
```

- [ ] **Step 2: Run it and verify it fails**

```bash
cargo test -p warp --lib server::telemetry::collector 2>&1 | tail -20
```

Expected: FAIL — no function `should_collect_for_channel`.

- [ ] **Step 3: Implement**

In `app/src/server/telemetry/collector.rs`, add to `impl TelemetryCollector` immediately before
`pub fn initialize_telemetry_collection`:

```rust
    /// Heddle: the OSS channel never collects or transmits telemetry.
    pub fn should_collect_for_channel(channel: warp_core::channel::Channel) -> bool {
        channel != warp_core::channel::Channel::Oss
    }
```

Then make `initialize_telemetry_collection` return immediately. Insert as the first line of its
body:

```rust
    pub fn initialize_telemetry_collection(&self, ctx: &mut ModelContext<TelemetryCollector>) {
        if !Self::should_collect_for_channel(ChannelState::channel()) {
            return;
        }
        // ... existing body unchanged ...
```

- [ ] **Step 4: Run the tests and verify they pass**

```bash
cargo test -p warp --lib server::telemetry::collector 2>&1 | tail -20
```

Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
./script/format
git add app/src/server/telemetry/collector.rs app/src/server/telemetry/collector_tests.rs
git commit -m "feat(telemetry): never initialize the collector on OSS channel"
```

---

### Task 6: Never apply server-side experiments on OSS

Removes remote control of client behaviour — the mechanism that could re-enable telemetry.

**Files:**
- Modify: `app/src/server/experiments/mod.rs` (`ServerExperiment::on_added_to`, line ~66)
- Test: `app/src/server/experiments/mod_tests.rs` (create)

**Interfaces:**
- Consumes: `warp_core::channel::{Channel, ChannelState}`.
- Produces: `ServerExperiment::should_apply_for_channel(channel: Channel) -> bool`.

- [ ] **Step 1: Write the failing test**

Create `app/src/server/experiments/mod_tests.rs`:

```rust
use warp_core::channel::Channel;

use super::ServerExperiment;

#[test]
fn oss_ignores_server_experiments() {
    assert!(!ServerExperiment::should_apply_for_channel(Channel::Oss));
}

#[test]
fn stable_applies_server_experiments() {
    assert!(ServerExperiment::should_apply_for_channel(Channel::Stable));
}
```

Append to `app/src/server/experiments/mod.rs`:

```rust
#[cfg(all(test, not(target_family = "wasm")))]
#[path = "mod_tests.rs"]
mod tests;
```

- [ ] **Step 2: Run it and verify it fails**

```bash
cargo test -p warp --lib server::experiments 2>&1 | tail -20
```

Expected: FAIL — no function `should_apply_for_channel`.

- [ ] **Step 3: Implement**

In `app/src/server/experiments/mod.rs`, add to `impl ServerExperiment` immediately before
`fn on_added_to`:

```rust
    /// Heddle: OSS builds are never remotely reconfigured. Server-side
    /// experiments are the mechanism by which the client's behaviour — including
    /// its telemetry posture — can be changed without a release, so the OSS
    /// channel ignores them entirely.
    pub fn should_apply_for_channel(channel: warp_core::channel::Channel) -> bool {
        channel != warp_core::channel::Channel::Oss
    }
```

Then insert as the first line of `on_added_to`'s body:

```rust
    fn on_added_to(&self, _ctx: &mut AppContext) {
        if !Self::should_apply_for_channel(warp_core::channel::ChannelState::channel()) {
            return;
        }
        match self {
        // ... existing body unchanged ...
```

- [ ] **Step 4: Run the tests and verify they pass**

```bash
cargo test -p warp --lib server::experiments 2>&1 | tail -20
```

Expected: PASS, 2 tests.

- [ ] **Step 5: Commit**

```bash
./script/format
git add app/src/server/experiments/mod.rs app/src/server/experiments/mod_tests.rs
git commit -m "feat(experiments): ignore server-side experiments on OSS channel"
```

---

### Task 7: Re-measure egress and produce the residual leak inventory

Turns the remaining leaks into the input for the follow-up plan.

**Files:**
- Create: `docs/superpowers/plans/egress-residual.md`
- Modify: none

**Interfaces:**
- Consumes: `script/heddle/egress-test` (Task 2), all overrides from Tasks 3–6.
- Produces: `docs/superpowers/plans/egress-residual.md` — the enumerated remaining egress paths
  that the `ChannelConfig` `Option` migration must eliminate.

- [ ] **Step 1: Re-run the egress harness**

```bash
./script/heddle/egress-test 2>&1 | tail -40
```

Expected: fewer violations than the Task 2 baseline. It may still FAIL — that is an acceptable and
expected outcome at this stage, because websocket, Firebase auth, and autoupdate paths do not go
through `http_client`.

- [ ] **Step 2: Diff against the baseline**

```bash
diff <(grep -oE 'inet_addr\("[0-9.]+"' docs/superpowers/plans/egress-baseline.log | sort -u) \
     <(grep -oE 'inet_addr\("[0-9.]+"' target/heddle-egress.log | sort -u) \
     || true
```

- [ ] **Step 3: Write the residual inventory**

Create `docs/superpowers/plans/egress-residual.md` recording, for each remaining destination: the
observed address, the code path responsible (find it with
`grep -rn '<hostname>' crates app --include='*.rs'`), and whether the `Option` migration will
remove it. Known candidates to check explicitly:

- `crates/warp_tui/src/autoupdate.rs:489` — requests `/client_version` before rejecting the OSS
  channel.
- `crates/websocket` and the RTC GraphQL socket from `WarpServerConfig::rtc_server_url`.
- Firebase auth via `WarpServerConfig::firebase_auth_api_key`.

- [ ] **Step 4: Run full presubmit**

```bash
./script/presubmit 2>&1 | tail -30
```

Expected: PASS. If clippy fails on files you did not touch, note it in the inventory rather than
fixing it — unrelated upstream lint churn is not this plan's problem.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/egress-residual.md
git commit -m "docs: record residual egress inventory after OSS choke-point overrides

Input for the ChannelConfig Option migration plan."
```

---

## Self-Review

**Spec coverage.** Phase 1 → Task 1. Phase 3 → Tasks 2 and 7 (egress allowlist, cold-start test,
residual inventory). Phase 2's knowable parts → Tasks 3–6 (telemetry policy, collector,
experiments). Phase 2's `Option` migration → deliberately deferred, with Task 7 producing its
input. Phases 4–6 (rebrand, release, ACP) → out of scope, separate plans.

**Not covered by this plan, by design:** the CI merge gate. It belongs with release engineering
(Phase 5) and would be premature while the egress test is still expected to fail.

**Type consistency.** `is_egress_permitted_for_channel(&Url, Channel) -> bool` and
`is_egress_permitted(&Url) -> bool` (Task 3); `should_disable_telemetry_for_channel(&self, Channel)
-> bool` (Task 4); `should_collect_for_channel(Channel) -> bool` (Task 5);
`should_apply_for_channel(Channel) -> bool` (Task 6). All take `warp_core::channel::Channel` and
return `bool`; each pairs a channel-parameterised function (testable) with a call site that reads
the live channel via `ChannelState::channel()`.

**Known risk.** Task 1 is unproven and gates everything. If the workspace does not compile on this
machine, Tasks 2–7 cannot start and the plan needs revisiting rather than pushing through.
