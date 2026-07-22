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
brew install jq clang-format pkgconf llvm protobuf
```

`protobuf` is REQUIRED on the host too, not just in the container:
`crates/remote_server/build.rs:4` calls `prost_build::compile_protos()` unconditionally, and
`warp_tui` pulls `remote_server` in through the `warp` crate.

Skip `sentry-cli`, `create-dmg`, `multitime`, `powershell`, and Docker Desktop for now — they are
needed for bundling and linting, not for `cargo build`. Do NOT run `./script/bootstrap`: it
requires sudo and switches your active Xcode.

- [ ] **Step 2b: Install the Xcode Metal Toolchain (macOS only)**

```bash
xcodebuild -downloadComponent MetalToolchain
```

VERIFIED REQUIRED on 2026-07-22 with Xcode 26.6: without it the build fails at
`crates/warpui/build.rs:113` with

```
error compiling metal shaders to .air; error: cannot execute tool 'metal' due to
missing Metal Toolchain; use: xcodebuild -downloadComponent MetalToolchain
```

Note that `xcrun -f metal` still resolves to a binary even when the component is absent, so
checking for the tool's existence is not a valid precondition test — only a build proves it.

This is needed even though `warp-tui-oss` is the *headless* TUI: `warp_tui` depends on the `warp`
app crate, which depends on `warpui`, whose build script compiles Metal shaders on macOS. The TUI
is not GPU-free at build time on this platform. (The Linux container in Task 2 is unaffected —
`warpui`'s build script takes a non-Metal path there.)

- [ ] **Step 3: Build the OSS TUI binary**

```bash
cd /Users/npaskov/Development/warp
set -o pipefail
cargo build -p warp_tui --bin warp-tui-oss 2>&1 | tail -30
```

`-p warp_tui` is REQUIRED. `crates/warp_tui` is not in the workspace's `default-members`
(`Cargo.toml:11`), so a bare `cargo build --bin warp-tui-oss` does not select the package and
fails to find the binary.

`set -o pipefail` is REQUIRED wherever a build is piped into `tail`, or a failing build exits 0 and
you will believe it succeeded.

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

Observes network syscalls (`connect`/`sendto`/`sendmsg`) directly, so it catches every egress path
— HTTP, websockets, Sentry, Firebase, UDP — below any client abstraction that could bypass an
in-process check. Runs in a Linux container so it behaves identically on your Mac and in CI.

**Files:**
- Create: `script/heddle/egress-test`
- Create: `docker/heddle-egress/Dockerfile`
- Test: the script IS the test.

**Interfaces:**
- Consumes: Task 1's proof that the workspace compiles (the container rebuilds for Linux itself).
- Produces: `script/heddle/egress-test`, exiting **0** when no non-loopback egress is observed,
  **1** when egress is observed, and **2** when the run was inconclusive. An inconclusive run must
  never be read as a pass. Writes the subject trace to `target/heddle-egress.log`, the positive
  control to `target/heddle-egress-control.log`, and run markers to
  `target/heddle-egress-markers.txt`.

- [ ] **Step 1: Write the container that observes syscalls**

Create `docker/heddle-egress/Dockerfile`:

```dockerfile
FROM rust:1.92.0-bookworm

# Mirrors script/linux/install_build_deps.
#
# strace is the observation instrument; curl backs the positive control.
RUN apt-get update && apt-get install -y --no-install-recommends \
        strace curl git ca-certificates unzip \
        build-essential cmake pkg-config \
        libssl-dev libfreetype-dev libexpat1-dev libgit2-dev \
        libfontconfig1-dev libasound2-dev libclang-dev \
    && rm -rf /var/lib/apt/lists/*

# VERIFIED REQUIRED 2026-07-22: protoc must come from the official release,
# NOT from apt. Debian's protobuf-compiler ships the binary WITHOUT the
# well-known type definitions, so the build fails with:
#   protoc failed: google/protobuf/descriptor.proto: File not found.
# Both crates/remote_server/build.rs and the warp_multi_agent_api git
# dependency compile protos, and the latter imports descriptor.proto.
# Upstream extracts 'include/*' for exactly this reason
# (script/linux/install_build_deps:60). Note macOS is unaffected because
# `brew install protobuf` bundles the includes — a platform difference that
# only showed up on the first container run.
ARG PROTOC_VERSION=25.1
RUN set -eux; \
    arch="$(uname -m)"; \
    case "$arch" in \
      x86_64)  zip="protoc-${PROTOC_VERSION}-linux-x86_64.zip" ;; \
      aarch64) zip="protoc-${PROTOC_VERSION}-linux-aarch_64.zip" ;; \
      *) echo "unsupported arch: $arch" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /tmp/protoc.zip \
      "https://github.com/protocolbuffers/protobuf/releases/download/v${PROTOC_VERSION}/${zip}"; \
    unzip -o /tmp/protoc.zip -d /usr/local bin/protoc 'include/*'; \
    rm /tmp/protoc.zip; \
    protoc --version

WORKDIR /src
ENTRYPOINT ["/bin/bash", "-c"]
```

- [ ] **Step 2: Write the egress test script**

Create `script/heddle/egress-test`:

```bash
#!/bin/bash
#
# Observes every outbound network syscall the OSS build attempts on a cold start
# with no user configuration, in the configuration we actually ship. Exits
# non-zero if any non-loopback destination is contacted. This is Heddle's core
# privacy guarantee, enforced mechanically rather than asserted.
#
# Exit codes: 0 = no egress, 1 = egress observed, 2 = inconclusive (do not
# interpret an inconclusive run as a pass).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")"/../.. && pwd)"
cd "${REPO_ROOT}"

IMAGE="heddle-egress:local"
LOG="target/heddle-egress.log"
CONTROL_LOG="target/heddle-egress-control.log"
MARKER="target/heddle-egress-markers.txt"
RUN_SECONDS="${HEDDLE_EGRESS_RUN_SECONDS:-20}"

mkdir -p target
rm -f "${LOG}" "${CONTROL_LOG}" "${MARKER}"

echo "Building observation image..."
docker build -q -t "${IMAGE}" docker/heddle-egress >/dev/null

echo "Building and tracing warp-tui-oss in SHIPPED configuration..."
# Three things make this test the real artifact rather than a convenient one:
#
#  1. GIT_RELEASE_TAG is baked in. ChannelState::app_version() reads
#     option_env!("GIT_RELEASE_TAG") (channel/state.rs:346). Without it the TUI
#     autoupdater disables itself with "no release version tag baked into this
#     build" (autoupdate.rs:210) and its egress silently vanishes from the test.
#  2. The binary is staged into <root>/versions/<ver>/ so InstallLayout::detect()
#     recognises a managed install (autoupdate.rs:101). Otherwise the updater
#     disables itself with "not running from a managed install".
#  3. -t allocates a TTY. The TUI uses an alt screen and exits immediately
#     without one, producing an empty trace and a meaningless PASS.
#
# Skipping any of these yields a test that passes while the shipped build
# phones home. That is the failure mode this harness exists to prevent.
docker run --rm -t \
    --cap-add=SYS_PTRACE \
    -v "${REPO_ROOT}:/src" \
    -v heddle-cargo-registry:/usr/local/cargo/registry \
    -v heddle-target-linux:/src/target-linux \
    "${IMAGE}" \
    "set -euo pipefail
     export CARGO_TARGET_DIR=/src/target-linux
     export GIT_RELEASE_TAG=v0.0.0-heddle-egress-test
     cargo build -p warp_tui --bin warp-tui-oss

     # Stage into a managed install layout: <root>/versions/<version>/<binary>
     STAGE=/tmp/heddle-install/versions/v0.0.0-heddle-egress-test
     mkdir -p \$STAGE
     cp \$CARGO_TARGET_DIR/debug/warp-tui-oss \$STAGE/

     # option_env! is evaluated at COMPILE time, and cargo does not invalidate
     # its cache when a plain environment variable changes. A build cached from
     # an earlier run would silently lack the tag, disabling the autoupdater and
     # restoring the false PASS this staging exists to prevent. Verify the tag
     # is actually baked into the artifact.
     if ! grep -aq 'v0.0.0-heddle-egress-test' \$STAGE/warp-tui-oss; then
       echo 'TAG_BAKED=no' >> /src/${MARKER}
     else
       echo 'TAG_BAKED=yes' >> /src/${MARKER}
     fi

     export HOME=/tmp/heddle-empty-home
     mkdir -p \$HOME

     # Positive control: prove the tracer actually observes egress in this
     # container before we trust it to report the absence of egress.
     RC=0
     strace -f -qq -e trace=network -o /src/${CONTROL_LOG} \
        curl -s -m 5 https://example.com >/dev/null 2>&1 || RC=\$?
     echo \"CONTROL_EXIT=\$RC\" >> /src/${MARKER}

     # Subject under test. 'set -e' would abort on timeout's non-zero exit
     # before the marker is written, so capture the code explicitly. 124 means
     # it was still running when the timeout fired.
     RC=0
     timeout ${RUN_SECONDS} strace -f -qq -e trace=network \
        -o /src/${LOG} \
        \$STAGE/warp-tui-oss || RC=\$?
     echo \"SUBJECT_EXIT=\$RC\" >> /src/${MARKER}"

echo
echo "Validating that the observation is meaningful..."

# A test that cannot fail proves nothing. Validate the instrument before the
# measurement. Markers live in their own file so an empty trace stays empty.
if ! grep -qE 'AF_INET' "${CONTROL_LOG}" 2>/dev/null; then
    echo "INCONCLUSIVE: the positive control observed no network syscalls."
    echo "strace is not attached correctly, so a PASS would be meaningless."
    exit 2
fi

if ! grep -q 'TAG_BAKED=yes' "${MARKER}"; then
    echo "INCONCLUSIVE: GIT_RELEASE_TAG is not baked into the binary, so the"
    echo "autoupdater disabled itself and its egress is invisible to this test."
    echo "Force a rebuild: docker volume rm heddle-target-linux"
    exit 2
fi

# NOTE: an empty subject trace is NOT inconclusive. Once the fork is clean that
# is precisely the expected result, and treating it as a failure would make the
# target state unreachable. The SUBJECT_EXIT=124 marker below is what proves the
# process actually ran; liveness and emptiness are separate questions.

if ! grep -q 'SUBJECT_EXIT=124' "${MARKER}"; then
    echo "INCONCLUSIVE: the process exited before the ${RUN_SECONDS}s timeout."
    echo "A short-lived process cannot demonstrate absence of egress."
    echo "Markers: $(tr '\n' ' ' < "${MARKER}")"
    exit 2
fi

echo "Analysing observed egress..."

# trace=network covers connect/sendto/sendmsg, so UDP egress is caught too.
# Anchor on the syscalls that carry a DESTINATION. Matching AF_INET anywhere
# would flag socket(AF_INET, SOCK_STREAM, ...) — merely creating a socket, with
# nowhere to send — as egress, producing false FAILs.
# Loopback and AF_UNIX are expected; anything else is egress.
VIOLATIONS="$(grep -E '(^|[0-9] +)(connect|sendto|sendmsg)\(' "${LOG}" 2>/dev/null \
    | grep -E 'AF_INET' \
    | grep -vE 'inet_addr\("127\.|inet_pton\(AF_INET6, "::1"' \
    || true)"

if [[ -n "${VIOLATIONS}" ]]; then
    echo "FAIL: the OSS build attempted outbound network activity:"
    echo "${VIOLATIONS}" | sed 's/^/  /' | head -40
    echo
    echo "Full trace: ${LOG}"
    exit 1
fi

echo "PASS: zero non-loopback egress in ${RUN_SECONDS}s, shipped configuration."
```

Then:

```bash
chmod +x script/heddle/egress-test
```

- [ ] **Step 3: Run it against the UNMODIFIED build and confirm it FAILS**

```bash
./script/heddle/egress-test
```

The script has three distinct outcomes, and only one of them is acceptable here:

| Exit | Meaning | What to do |
|---|---|---|
| **1** | Egress observed — connections to Warp infrastructure listed | **This is the goal.** Proceed to Step 4. |
| 2 | Inconclusive — empty trace, process died early, or strace not attached | Fix the harness. Do NOT proceed. |
| 0 | "No egress" from an unmodified build | Disbelieve it. The observer is broken. |

Exit 1 is the deliverable: it is the evidence that the leak is real *and* that the observer can
detect it. A test that has never failed proves nothing, so do not continue past this step until you
have seen it fail for the right reason.

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
fn egress_oss_denies_warp_infrastructure() {
    for host in [
        "https://app.warp.dev/graphql/v2",
        "https://oz.warp.dev/api",
        "wss://rtc.app.warp.dev/graphql/v2",
    ] {
        let url = reqwest::Url::parse(host).unwrap();
        assert!(
            !crate::is_egress_permitted_for_channel(&url, warp_core::channel::Channel::Oss),
            "OSS builds must not contact Warp infrastructure: {host}"
        );
    }
}

#[test]
fn egress_oss_permits_loopback() {
    let url = reqwest::Url::parse("http://127.0.0.1:8080/health").unwrap();
    assert!(crate::is_egress_permitted_for_channel(
        &url,
        warp_core::channel::Channel::Oss
    ));
}

#[test]
fn egress_oss_permits_unrelated_third_party() {
    // The tripwire targets Warp's own infrastructure. It must NOT break
    // legitimate user-directed traffic such as LSP server downloads from
    // GitHub (crates/lsp/src/install.rs:59) or a user's own model provider.
    for host in [
        "https://github.com/some/lsp-release.tar.gz",
        "https://api.example-provider.test/v1/messages",
    ] {
        let url = reqwest::Url::parse(host).unwrap();
        assert!(
            crate::is_egress_permitted_for_channel(&url, warp_core::channel::Channel::Oss),
            "tripwire must not block unrelated traffic: {host}"
        );
    }
}

#[test]
fn egress_non_oss_channels_are_unaffected() {
    let url = reqwest::Url::parse("https://app.warp.dev/graphql/v2").unwrap();
    assert!(crate::is_egress_permitted_for_channel(
        &url,
        warp_core::channel::Channel::Stable
    ));
}
```

- [ ] **Step 2: Run it and verify it fails**

```bash
set -o pipefail
cargo test -p http_client egress 2>&1 | tail -20
```

Expected: FAIL — `cannot find function `is_egress_permitted_for_channel` in this scope`.

- [ ] **Step 3: Implement the guard**

Add to `crates/http_client/src/lib.rs`, immediately before `impl Client {` at line 138:

```rust
/// Warp's own first-party infrastructure. Contacting any of it from an OSS
/// build defeats the point of the fork.
const WARP_INFRASTRUCTURE_SUFFIXES: &[&str] = &[
    "warp.dev",
    "rudderstack.com",
    "rudderlabs.com",
    "sentry.io",
    "firebaseio.com",
    "identitytoolkit.googleapis.com",
];

/// Whether `url` may be contacted from `channel`.
///
/// This is a **tripwire, not the guarantee.** A deny-by-default allowlist here
/// would be both leaky and harmful: leaky because `crates/mcp/src/runtime.rs:428`
/// builds a raw `reqwest` client that never passes through this type, and
/// harmful because it would block legitimate user-directed traffic such as LSP
/// server downloads (`crates/lsp/src/install.rs:59`). Narrowly targeting Warp's
/// own hosts is what this layer can actually do correctly.
///
/// The real guarantees live elsewhere: the `ChannelConfig` `Option` migration
/// removes the endpoints at compile time, and `script/heddle/egress-test`
/// observes the process at the syscall level, below every bypass.
///
/// Only [`Channel::Oss`] is restricted; other channels are upstream's concern.
pub fn is_egress_permitted_for_channel(url: &reqwest::Url, channel: Channel) -> bool {
    if channel != Channel::Oss {
        return true;
    }
    let Some(host) = url.host_str() else {
        // No host means no network egress (e.g. a data: URL).
        return true;
    };
    !WARP_INFRASTRUCTURE_SUFFIXES
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
set -o pipefail
cargo test -p http_client egress 2>&1 | tail -20
```

Expected: PASS, 4 tests.

- [ ] **Step 5: Enforce the guard on every request verb**

In `crates/http_client/src/lib.rs`, add this helper inside `impl Client` (immediately before
`pub fn get` at line 216):

```rust
    /// Resolves the URL a request should actually target under the egress
    /// policy. A denied request is redirected to a closed loopback port so it
    /// fails fast and *nothing leaves the machine* — in release builds as well
    /// as debug ones. Returning a bool and ignoring it would let release builds
    /// keep sending, which is the failure mode this whole harness exists to
    /// prevent.
    fn egress_checked_url<U: IntoUrl + Clone>(url: U) -> reqwest::Url {
        // RFC 863 discard port, on loopback, with nothing listening.
        fn blocked() -> reqwest::Url {
            reqwest::Url::parse("http://127.0.0.1:9/heddle-blocked")
                .expect("static URL is valid")
        }

        match url.into_url() {
            Ok(parsed) if is_egress_permitted(&parsed) => parsed,
            Ok(parsed) => {
                let host = parsed.host_str().unwrap_or("<no host>").to_owned();
                // Deliberately NOT debug_assert!(false): a policy violation
                // should be loud, not fatal. Panicking would crash debug builds
                // on any stray call site and break existing upstream tests.
                log::error!("Blocked OSS egress to {host}");
                blocked()
            }
            // An unparseable URL could never have succeeded anyway.
            Err(_) => blocked(),
        }
    }
```

Then rewrite each of `get`, `post`, `put`, `patch`, and `delete` (lines 216–244) to route through
it. Shown for `get`; repeat the identical shape for the other four, changing only the verb:

```rust
    pub fn get<U: IntoUrl + Clone>(&self, url: U) -> RequestBuilder<'_> {
        let url = Self::egress_checked_url(url);
        let include_warp_headers = Self::include_warp_http_headers(url.clone());
        let iap_token = self.iap_token_for(url.clone());
        self.builder(self.wrapped.get(url), include_warp_headers, iap_token)
    }
```

`reqwest::Url` implements `IntoUrl + Clone`, so `include_warp_http_headers` and `iap_token_for`
accept it unchanged.

This is defence in depth, not the primary guarantee. The hard guarantee is the `ChannelConfig`
`Option` migration in the follow-up plan, which deletes the endpoints from the binary entirely.

- [ ] **Step 6: Verify the workspace still builds and lints clean**

```bash
set -o pipefail
cargo build -p warp_tui --bin warp-tui-oss 2>&1 | tail -5
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
set -o pipefail
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
set -o pipefail
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
set -o pipefail
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
set -o pipefail
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
set -o pipefail
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
set -o pipefail
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
set -o pipefail
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

**Known residual gaps, to be closed by the follow-up plan.** Tasks 5 and 6 stop the *behaviour*
but not the *lifecycle*, and that distinction matters:

- Task 5 stops the collector's background work, but `TelemetryCollector` is still constructed and
  registered (`app/src/lib.rs:1916`), and the shutdown path still calls flushing
  (`app/src/lib.rs:1124`). Neither should exist on OSS.
- Task 6 stops experiments being *applied*, but `ServerExperiments` still caches membership and
  emits update events (`app/src/server/experiments/model.rs:46`), and production code still reads
  that cached state (`app/src/workspaces/user_workspaces.rs:1617`). Remote state therefore still
  reaches the client even though it no longer flips flags.
- The egress harness covers the **TUI** only. The GUI needs its own headless approach (Xvfb or
  equivalent) before Heddle can claim the guarantee for the app it actually ships.

These are recorded rather than fixed here because each depends on the `ChannelConfig` `Option`
migration, which removes the server clients that feed them.

**Type consistency.** `is_egress_permitted_for_channel(&Url, Channel) -> bool` and
`is_egress_permitted(&Url) -> bool` (Task 3); `should_disable_telemetry_for_channel(&self, Channel)
-> bool` (Task 4); `should_collect_for_channel(Channel) -> bool` (Task 5);
`should_apply_for_channel(Channel) -> bool` (Task 6). All take `warp_core::channel::Channel` and
return `bool`; each pairs a channel-parameterised function (testable) with a call site that reads
the live channel via `ChannelState::channel()`.

**Known risk.** Task 1 is unproven and gates everything. If the workspace does not compile on this
machine, Tasks 2–7 cannot start and the plan needs revisiting rather than pushing through.
