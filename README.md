# Heddle

A de-commercialized fork of the [Warp](https://github.com/warpdotdev/Warp) terminal.

**No account. No telemetry. No `warp.dev` anywhere in the binary.**

A heddle is the loom component that lifts and separates the warp threads — the part that
controls the warp.

---

## What this is

Warp open-sourced its client under AGPL-3.0. The client is genuinely open; the **server, Warp
Drive backend, hosted authentication, and Oz** (the agent orchestration layer) are not, and remain
proprietary.

Heddle takes the open client and removes its dependence on the closed parts. It is not a
"cracked" Warp — see [Non-goals](#non-goals).

## What was removed

Most items below are removed from the source outright. A subset — every
`warp.dev` string, the Firebase key, and telemetry destinations — is additionally
checked against the built binary on every change; see
[Verification](#verification) for exactly what that check does and does not prove.

| Removed | Detail |
|---|---|
| **Warp's endpoints** | `app.warp.dev`, `rtc.app.warp.dev`, `sessions.app.warp.dev`, `staging.warp.dev`, `oz.warp.dev` are absent from the binary |
| **Firebase credentials** | The hardcoded Firebase auth API key is gone |
| **Telemetry** | RudderStack destinations absent; the collector never starts |
| **Server-supplied privacy settings** | See below — the most important finding |
| **Remote configuration** | Server-driven experiments are never applied |
| **Crash reporting** | No Sentry |
| **Warp Drive sync** | The cloud-object listener does not start |
| **Hosted auth** | Sign-in, sign-up, SSO and device-authorization flows have no endpoint |
| **Billing surfaces** | Upgrade links, Stripe pages, plan-comparison and pricing links removed |
| **Warp's legal pages** | Terms of Service and privacy policy no longer linked as if they govern Heddle |
| **Warp support contacts** | `support@`, `sales@`, `feedback@` and `referrals@warp.dev` replaced with Heddle's issue tracker |
| **All `warp.dev` links** | Every URL, mailto and documentation link — the scanner forbids the apex domain |

### The finding that shaped the design

Upstream treats `WarpDrivePrivacySettings` — a **cloud object** — as, in its own words, "the
source of truth for these booleans" (`app/src/settings/privacy.rs`). On a logged-out cold start,
the unmodified client logged:

```
[warp::settings::privacy] Warp Drive privacy preferences are set, using those for
    telemetry=true, crash_reporting=true, cloud_conversation_storage=true
```

The client **fetched privacy preferences from the server and switched telemetry on** — on a build
whose `telemetry_config` was already `None`. Disabling the telemetry transport was therefore never
sufficient: the *policy input itself* arrived over the network. Heddle clamps privacy state at
every write and never performs the fetch.

## How it works

The mechanism is a type change, not a feature flag:

```rust
pub struct ChannelConfig {
    pub server_config: Option<WarpServerConfig>,
    pub oz_config: Option<OzConfig>,
    // ...
}
```

Heddle passes `None`. The endpoints are not in the binary, so there is nothing to re-enable — no
flag to flip, no server-pushed configuration that can restore them. Every one of the ~101 call
sites that wanted a Warp URL became a compile error and was resolved individually, so absence is
enforced by the type system rather than by a runtime check that could be bypassed.

## Building

```bash
cargo build --release -p warp_tui --bin heddle-tui
./script/heddle/verify-no-warp-endpoints target/release/heddle-tui
```

On macOS the Metal Toolchain is a separate Xcode component and the build fails without it.
Note that `xcrun -f metal` resolves even when the component is absent, so its presence proves
nothing — only a build does:

```bash
xcodebuild -downloadComponent MetalToolchain
```

**Binaries are published for Linux x86_64 only.** macOS is built from source deliberately:
notarization needs a paid Apple Developer ID this project does not have, and shipping unsigned
binaries would mean publishing `xattr -d com.apple.quarantine` as the official install path. That
normalizes bypassing a security control and leaves users unable to distinguish a genuine download
from a tampered one.

## Verification

```bash
./script/heddle/verify-no-warp-endpoints
```

The scanner reads **raw bytes** of the built binary and fails if any Warp endpoint, credential or
telemetry destination appears. It is validated in both directions: it passes on a clean build and
fails on a binary with a planted canary string.

**What this proves, and what it does not.** The scan is a regression tripwire, not proof of
network silence. An endpoint could in principle be assembled at runtime, and no property of any
binary can stop a user typing `curl app.warp.dev` into their own terminal. Fully substantiating the
claim additionally requires syscall-level tracing across startup, onboarding, agents, updates and
shutdown. **That work is not done.**

One honest caveat:

- **The string rebrand is incomplete.** Menu items, dialogs, notifications and icons say Heddle, but
  roughly 600 further strings — mostly log messages and internal diagnostics — still say "Warp".
  Two categories are left alone deliberately: `Warp OSC` markers are a wire-protocol identifier
  that shell hooks depend on, and "Warp Drive" names an upstream feature Heddle removed, so
  renaming it would imply Heddle has it.
- Observed behaviour on a logged-out cold start is below; it is evidence, not proof.

| Unmodified upstream did | Heddle |
|---|---|
| Fetched channel versions from Warp's server | nothing |
| Opened a Warp Drive websocket, retried on failure | listener never starts |
| Fetched privacy preferences and set `telemetry=true` | nothing |

## Non-goals

- **This does not unlock paid Warp features.** Entitlements are enforced server-side. Removing
  paywall UI removes nags and dead controls; it does not grant premium functionality, because
  there is no server to grant it. The value here is privacy and independence, not free Pro.
- **This does not reimplement Warp Drive.** Cloud sync is removed, not replaced.
- **This does not fork Warp's server.** There is no server to fork.

## Status

| Area | State |
|---|---|
| Endpoint removal | Verified — scanner passes, enforced in CI |
| Telemetry / experiments / Drive | Removed |
| Rebrand | Own name, icon, logo, bundle ID, paths; upstream doc links remain |
| Release builds | Linux x86_64; macOS builds from source |
| Agent support (ACP) | Designed, **not implemented** — see below |

Warp's built-in agent runs on their proprietary server and cannot work here. The intended
replacement is [ACP](https://agentclientprotocol.com/), bridging to a local agent process. The
design is written up in `docs/superpowers/specs/2026-07-22-acp-agent-bridge-design.md`.

It is deliberately **not** implemented in v0.1. An agent that streams tool calls without working
permission prompts and cancellation can execute commands the user never agreed to, so a
half-finished bridge is worse than none. Heddle will not be described as agentic until the
acceptance criteria in that spec are demonstrably met.

Note that `app/src/terminal/cli_agent.rs` recognising Claude Code, Codex and Gemini CLI is
*presentation logic, not protocol compatibility* — recognising a binary name is not the same as
speaking ACP to it.

**Until then, Heddle is a terminal, not an agentic environment.**

## Licence

AGPL-3.0, inherited from upstream and unchanged. The `warpui` and `warpui_core` crates remain MIT,
as upstream licensed them.

Copyright © 2026 Denver Technologies, Inc. Modified work © 2026 Heddle contributors.

Upstream's copyright notices are preserved throughout, as the AGPL requires. "Warp" is a trademark
of Denver Technologies, Inc.; Heddle is an independent fork and is **not** affiliated with,
endorsed by, or supported by them. Please do not report Heddle issues to Warp.
