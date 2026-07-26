# How Heddle works

This is the technical companion to the [README](../README.md). It covers what was removed and by
what mechanism, the finding that shaped the approach, what the automated checks do and do not
prove, and the bug pattern that caused the most damage.

If you only want to download and run the app, the README is all you need.

---

## What this is

Warp open-sourced its client under AGPL-3.0. The client is genuinely open; the **server, Warp Drive
backend, hosted authentication, and Oz** (the agent orchestration layer) are not, and remain
proprietary.

Heddle takes the open client and removes its dependence on the closed parts — the way a heddle
takes hold of the warp threads and decides which ones lift.

| Heddle **is** | Heddle **is not** |
|---|---|
| The open AGPL Warp client, with its dependence on Warp's closed server removed | A "cracked" Warp — entitlements live server-side; there is nothing to crack |
| Offline by construction: endpoints deleted at the type level, not flag-gated | A Warp Drive reimplementation — cloud sync is removed, not replaced |
| Verified: byte-level endpoint scan on every change, in CI and locally | Agentic (yet) — today it is a terminal |
| Independent: no affiliation with, endorsement by, or support from Warp | A place to report Warp bugs — please don't |

## The finding that shaped the design

Upstream treats `WarpDrivePrivacySettings` — a **cloud object** — as, in its own words, "the source
of truth for these booleans" (`app/src/settings/privacy.rs`). On a logged-out cold start, the
unmodified client logged:

```
[warp::settings::privacy] Warp Drive privacy preferences are set, using those for
    telemetry=true, crash_reporting=true, cloud_conversation_storage=true
```

The client **fetched privacy preferences from the server and switched telemetry on** — on a build
whose `telemetry_config` was already `None`. Disabling the telemetry transport was therefore never
sufficient: the *policy input itself* arrived over the network. Heddle clamps privacy state at every
write and never performs the fetch.

## The mechanism: a type change, not a feature flag

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

Most items below are removed from the source outright. A subset — every `warp.dev` string, the
Firebase key, and telemetry destinations — is additionally checked against the built binary on every
change; see [Verification](#verification) for exactly what that check does and does not prove.

| Removed | Detail |
|---|---|
| **Warp's endpoints** | `app.warp.dev`, `rtc.app.warp.dev`, `sessions.app.warp.dev`, `staging.warp.dev`, `oz.warp.dev` are absent from the binary |
| **Firebase credentials** | The hardcoded Firebase auth API key is gone |
| **Telemetry** | RudderStack destinations absent; the collector never starts |
| **Server-supplied privacy settings** | See [the finding](#the-finding-that-shaped-the-design) above |
| **Remote configuration** | Server-driven experiments are never applied |
| **Crash reporting** | No Sentry in the shipped channel |
| **Warp Drive sync** | The cloud-object listener does not start |
| **Hosted auth** | Sign-in, sign-up, SSO and device-authorization flows have no endpoint |
| **Billing surfaces** | Upgrade links, Stripe pages, plan-comparison and pricing links removed |
| **The ambient cloud-agent runtime** | Cloud pane creation, composer, restoration and the view-model are being excised slice by slice — see the [removal plan](design/plans/2026-07-24-ambient-runtime-removal.md) |
| **Warp's legal pages** | Terms of Service and privacy policy no longer linked as if they govern Heddle |
| **Warp support contacts** | `support@`, `sales@`, `feedback@` and `referrals@warp.dev` replaced with Heddle's issue tracker |
| **All `warp.dev` links** | Every URL, mailto and documentation link — the scanner forbids the apex domain |
| **Warp's branded assets** | The logomark glyph patched into the bundled fonts, the promotional badges, and the Oz/Drive promotional imagery |
| **Warp-controlled code paths** | `script/bootstrap` and `script/run` used to fetch a script from a Warp-owned repository at unpinned `main` and execute it; every Warp-owned dependency is now pinned to an immutable revision |

## Verification

```bash
./script/heddle/verify-no-warp-endpoints     # no Warp addresses in the built binary
./script/heddle/verify-bundled-assets        # bundled binary assets match a reviewed manifest
./script/heddle/verify-warp-supply-chain     # no Warp-controlled code path; dependencies pinned
./script/heddle/gui-surface-gate             # commercial UI and Warp strings can only shrink
```

Each has a self-test that plants a deliberate violation and requires the checker to reject it, so a
passing result means something. That habit exists because an earlier gate was written, run, reported
clean, and was **vacuous** — it had been copied somewhere its path resolution broke, and an
unmodified copy passed identically to a deliberately broken one. A check that has never been shown
to fail is not evidence.

### What the scan proves, and what it does not

The scan is a regression tripwire, not proof of network silence. An endpoint could in principle be
assembled at runtime, and no property of any binary can stop a user typing `curl app.warp.dev` into
their own terminal. Fully substantiating the claim additionally requires syscall-level tracing
across startup, onboarding, agents, updates and shutdown. **That work is not done.**

### Observed behaviour on a logged-out cold start

Evidence, not proof:

| Unmodified upstream did | Heddle |
|---|---|
| Fetched channel versions from Warp's server | nothing |
| Opened a Warp Drive websocket, retried on failure | listener never starts |
| Fetched privacy preferences and set `telemetry=true` | nothing |

### The rebrand is incomplete, and measured rather than estimated

A gate tracks every string literal mentioning Warp and fails if the count rises
(`script/heddle/gui-surface-gate`). It currently stands at **2,972**. An earlier draft of the README
guessed "roughly 600", which was wrong by a factor of five.

Most of that number is not renameable, and should not be:

| Category | Why it stays |
|---|---|
| `WARP_*` environment variables, `.warp/` project directories | read by the shell integration and by users' own repositories |
| keybinding action ids (`terminal:heddlify_subshell` and friends) | persisted in `keybindings.yaml`; renaming orphans user bindings |
| `SourcedRcFileForWarp` and `Warp OSC` markers | wire-protocol identifiers the shell hooks depend on |
| `warpdotdev/claude-code-warp`, `warpdotdev/codex-warp` | the real upstream repos the CLI-agent plugins install from |
| test fixtures, telemetry event ids | not user-visible |

About 1,100 sit outside tests and telemetry; the user-visible remainder is a few hundred and is
being worked down.

An earlier version of this section claimed the Linux and Windows data directories were still
Warp-named. That was wrong: it was read off test assertions exercising the crate's default
application id rather than the shipped binaries, both of which set their own. The default has since
been corrected to match them, so no supported configuration writes to a Warp-named directory.

## The account-gate trap

Worth writing down, because it caused more damage than any leftover label and took three encounters
to recognise as a pattern.

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

A related instance, found by running the app rather than reading it: Warp's logomark was a
private-use glyph (U+E500) patched into the bundled fonts. Removing the glyph from the fonts was
correct — it is their trademark — but three renderers still asked for it, so the AI loading
indicator drew a missing-glyph box before every label. The asset scanner could not catch it: it
verifies the asset is *gone*, which it was; nothing verified that the code which *drew* it had been
updated. Removing an asset and leaving its consumer behind is silent by construction.

## Non-goals

- **This does not unlock paid Warp features.** Entitlements are enforced server-side. Removing
  paywall UI removes nags and dead controls; it does not grant premium functionality, because there
  is no server to grant it. The value here is privacy and independence, not free Pro.
- **This does not reimplement Warp Drive.** Cloud sync is removed, not replaced.
- **This does not fork Warp's server.** There is no server to fork.

## The agent story

Warp's built-in agent runs on their proprietary server and cannot work here. The intended
replacement is [ACP](https://agentclientprotocol.com/), bridging to a local agent process. The
design is written up in `design/specs/2026-07-22-acp-agent-bridge-design.md`.

It is deliberately **not** implemented yet. An agent that streams tool calls without working
permission prompts and cancellation can execute commands the user never agreed to, so a
half-finished bridge is worse than none. Heddle will not be described as agentic until the
acceptance criteria in that spec are demonstrably met.

Note that `app/src/terminal/cli_agent.rs` recognising Claude Code, Codex and Gemini CLI is
*presentation logic, not protocol compatibility* — recognising a binary name is not the same as
speaking ACP to it.

**Until then, Heddle is a terminal, not an agentic environment.**

## Signing and provenance

The published Apple Silicon builds are signed with a Developer ID and notarized by Apple. The GUI
carries a stapled ticket, so it opens by double-click with no quarantine flag to clear.

The CLI is signed and notarized too, but carries no stapled ticket: Apple staples only to bundles,
disk images and packages, never to a bare executable. Running `spctl` against it locally therefore
reports "Unnotarized Developer ID" even though Apple's record says otherwise, because there is no
stapled ticket for it to read.

Two things a signature does *not* give you, worth stating since they are easy to over-read:

- It proves the build came from this signing identity and has not been altered since. It says
  nothing about whether the code does what the README claims — that is what the endpoint scanner,
  the surface gates and the test suite are for.
- A SHA-256 published beside the artefact proves the file matches what the release job produced.
  Provenance comes from the signature; the checksum remains useful, but anyone who can write to the
  release page can replace both.

If that trade-off is not acceptable, build from source. It is the same code and you establish
provenance yourself.
