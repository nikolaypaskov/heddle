# Heddle

```text
     │  │  │  │  │  │  │  │
   ╔═╪══╪══╪══╪══╪══╪══╪══╪═╗
   ║ ●  │  ●  │  ●  │  ●  │ ║       h e d d l e  /ˈhɛd(ə)l/  n.
   ╚═╪══╪══╪══╪══╪══╪══╪══╪═╝       the loom component that lifts and separates
     │  │  │  │  │  │  │  │         the warp threads — the part that controls
     ┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄┄       the warp
```

A de-commercialized fork of the [Warp](https://github.com/warpdotdev/Warp) terminal.

**No account. No telemetry. No `warp.dev` anywhere in the binary.**

[![privacy gate](https://github.com/nikolaypaskov/heddle/actions/workflows/heddle-privacy-gate.yml/badge.svg)](https://github.com/nikolaypaskov/heddle/actions/workflows/heddle-privacy-gate.yml)
[![license: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-8250df)](LICENSE)
[![phone home](https://img.shields.io/badge/phones_home-never-success)](#verification)

The claim above is not a promise in a README — it is a CI gate that reads the raw
bytes of every built artefact and blocks the publish if it fails:

```console
$ ./script/heddle/verify-no-warp-endpoints target/release/heddle-tui
PASS: no Warp endpoints, credentials, or telemetry destinations found.

$ ./script/heddle/verify-bundled-assets
PASS: all 34 bundled binary assets match the reviewed manifest.
```

## At a glance

| Heddle **is** | Heddle **is not** |
|---|---|
| The open AGPL Warp client, with its dependence on Warp's closed server removed | A "cracked" Warp — entitlements live server-side; there is nothing to crack |
| Offline by construction: endpoints deleted at the type level, not flag-gated | A Warp Drive reimplementation — cloud sync is removed, not replaced |
| Verified: byte-level endpoint scan on every change, in CI and locally | Agentic (yet) — see [Status](#status); today it is a terminal |
| Independent: no affiliation with, endorsement by, or support from Warp | A place to report Warp bugs — please don't |

## What this is

Warp open-sourced its client under AGPL-3.0. The client is genuinely open; the **server, Warp
Drive backend, hosted authentication, and Oz** (the agent orchestration layer) are not, and remain
proprietary.

Heddle takes the open client and removes its dependence on the closed parts — the way a heddle
takes hold of the warp threads and decides which ones lift.

## The finding that shaped the design

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
    pub server_config: Option<WarpServerConfig>,   // Heddle: None
    pub oz_config: Option<OzConfig>,               // Heddle: None
    // ...
}
```

The endpoints are not in the binary, so there is nothing to re-enable — no flag to flip, no
server-pushed configuration that can restore them. Every one of the ~101 call sites that wanted a
Warp URL became a compile error and was resolved individually, so absence is enforced by the type
system rather than by a runtime check that could be bypassed.

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
| **Server-supplied privacy settings** | See [the finding](#the-finding-that-shaped-the-design) above |
| **Remote configuration** | Server-driven experiments are never applied |
| **Crash reporting** | No Sentry |
| **Warp Drive sync** | The cloud-object listener does not start |
| **Hosted auth** | Sign-in, sign-up, SSO and device-authorization flows have no endpoint |
| **Billing surfaces** | Upgrade links, Stripe pages, plan-comparison and pricing links removed |
| **The ambient cloud-agent runtime** | Cloud pane creation, composer, restoration and the view-model are being excised slice by slice — see the [removal plan](docs/superpowers/plans/2026-07-24-ambient-runtime-removal.md) |
| **Warp's legal pages** | Terms of Service and privacy policy no longer linked as if they govern Heddle |
| **Warp support contacts** | `support@`, `sales@`, `feedback@` and `referrals@warp.dev` replaced with Heddle's issue tracker |
| **All `warp.dev` links** | Every URL, mailto and documentation link — the scanner forbids the apex domain |

## Building

```bash
cargo build --release -p warp_tui --bin heddle-tui
./script/heddle/verify-no-warp-endpoints target/release/heddle-tui
```

<details>
<summary><strong>macOS notes</strong> — Metal Toolchain, and the unsigned Apple Silicon binary</summary>

On macOS the Metal Toolchain is a separate Xcode component and the build fails without it.
Note that `xcrun -f metal` resolves even when the component is absent, so its presence proves
nothing — only a build does:

```bash
xcodebuild -downloadComponent MetalToolchain
```

**The published Apple Silicon builds are signed with a Developer ID and notarized by Apple.**
The GUI carries a stapled ticket, so it opens by double-click with no quarantine flag to clear
and no security override asked of you.

The CLI is signed and notarized too, but carries no stapled ticket: Apple staples only to bundles,
disk images and packages, never to a bare executable. Running `spctl` against it locally therefore
reports "Unnotarized Developer ID" even though Apple's record says otherwise, because there is no
stapled ticket for it to read.

Two things a signature does *not* give you, worth stating since they are easy to over-read:

- It proves the build came from this signing identity and has not been altered since. It says
  nothing about whether the code does what the README claims — that is what the endpoint scanner,
  the surface gates and the test suite are for.
- A SHA-256 published beside the artefact proves the file matches what the release job
  produced. Provenance now comes from the signature; the checksum remains useful because anyone who can write
  to the release page can replace both.

If that trade-off is not acceptable to you, build from source; it is the same code and you
establish provenance yourself. If it is acceptable:

```bash
shasum -a 256 -c heddle-aarch64-apple-darwin.tar.gz.sha256
tar -xzf heddle-aarch64-apple-darwin.tar.gz
xattr -d com.apple.quarantine heddle-aarch64-apple-darwin/heddle-tui
```

</details>

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

Honest caveats:

- **The string rebrand is incomplete, and now measured rather than estimated.** A gate tracks every
  string literal mentioning Warp and fails if the count rises: `script/heddle/gui-surface-gate`.
  It currently stands at **2,987**. An earlier draft of this README guessed "roughly 600", which was
  wrong by a factor of five.

  Most of that number is not renameable, and should not be:

  | Category | Why it stays |
  |---|---|
  | `WARP_*` environment variables, `.warp/` project directories | read by the shell integration and by users' own repositories |
  | keybinding action ids (`terminal:heddlify_subshell` and friends) | persisted in `keybindings.yaml`; renaming orphans user bindings |
  | `SourcedRcFileForWarp` and `Warp OSC` markers | wire-protocol identifiers the shell hooks depend on |
  | `warpdotdev/claude-code-warp`, `warpdotdev/codex-warp` | the real upstream repos the CLI-agent plugins install from |
  | test fixtures, telemetry event ids | not user-visible |

  About 1,100 sit outside tests and telemetry; the user-visible remainder is a few hundred and is
  being worked down. On Linux and Windows the data directory is still Warp-named, because those
  platforms derive it from an application id whose default is unchanged — that holds real settings,
  so it needs a migration rather than a rename.
- Observed behaviour on a logged-out cold start is below; it is evidence, not proof.

| Unmodified upstream did | Heddle |
|---|---|
| Fetched channel versions from Warp's server | nothing |
| Opened a Warp Drive websocket, retried on failure | listener never starts |
| Fetched privacy preferences and set `telemetry=true` | nothing |

### The account-gate trap

Worth writing down, because it caused more damage than any leftover label and took three
encounters to recognise as a pattern.

Upstream gates paid capabilities behind an account check. Remove accounts and the predicate becomes
a constant — so the capability does not become free, it becomes **permanently off**, silently:

| Function | Consequence before the fix |
|---|---|
| `AISettings::is_any_ai_enabled` | returned false for every user at 312 call sites; every AI surface reported itself unavailable |
| `UserWorkspaces::is_byo_api_key_enabled` | API-key fields built disabled, and one path erased a key the user had already saved |
| `apply_onboarding_settings(has_account)` | choosing the agent during first run completed onboarding with AI switched off |

None of these looked like anything until the predicate was read. All 64 call sites of
`is_anonymous_or_logged_out()` and `is_logged_in()` have since been audited: most are correct and
guard operations that genuinely need a server.

Test suites do not catch this class, and the reason is worth knowing: they build auth state with a
helper that reports a **signed-in** user, so the state this fork is always in is never exercised.

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
| Ambient cloud-agent runtime | Being excised slice by slice, each independently reviewed — [plan](docs/superpowers/plans/2026-07-24-ambient-runtime-removal.md) |
| Rebrand | Own name, icon, logo, bundle ID, paths, CLI command; remaining Warp strings tracked by a gate that can only shrink |
| Commercial UI | Account menus, Warp Drive, the billing/Account page, upgrade prompts and the first-run login all removed; surface tracked by the same gate |
| Release builds | Linux x86_64 via CI; macOS Apple Silicon GUI + TUI (**ad-hoc signed, not notarized** — see macOS notes) |
| Built-in agent | **Cannot send requests.** Its transport requires a Warp server; keys can be stored but not used |
| Third-party CLI agents | Working — local child processes reading credentials from your environment |
| Agent support (ACP) | Designed + `AgentBackend` seam planned; **not implemented** (next milestone) |

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

---

```text
┄┄┄┄┄┄┄ the warp is only half the cloth ┄┄┄┄┄┄┄
```
