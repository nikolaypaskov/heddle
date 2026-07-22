# Heddle — De-commercialized FOSS Fork of Warp

**Date:** 2026-07-22
**Status:** Approved design, pending implementation plan
**Name:** Heddle

A heddle is the loom component that lifts and separates the warp threads — the part that controls
the warp. It signals lineage without borrowing the mark.

The name is deliberately *not* derived from "Warp". AGPL grants the code, not the trademark, and
trademark infringement turns on likelihood of confusion, so a near-homophone in the same product
category (e.g. "Frarp") was rejected. This follows the convention of every comparable fork:
Terraform → OpenTofu, Redis → Valkey, Vault → OpenBao, Hudson → Jenkins.

Availability verified 2026-07-22: `heddle` is free on crates.io; the GitHub namespace holds 48
repositories with the most-starred at 17 stars, and no dev-tool collisions.

## Goal

Produce a publicly distributed, genuinely free and open-source build of the Warp client that
requires no account, sends no telemetry, and cannot contact Warp's servers — with that last
property enforced by the compiler and by CI rather than asserted in a README.

## Legal framing

- Upstream `warpdotdev/Warp` is **AGPL-3.0**, except `warpui` and `warpui_core` which are **MIT**.
- The fork stays AGPL-3.0. This is not optional and not a burden — it is the license's purpose.
  Upstream's own FAQ: *"Can someone fork Warp? Yes — that's what AGPL is for."*
- AGPL licenses the **code, not the trademark**. The fork must be renamed and de-branded:
  new name, bundle identifier, icons, and removal of `warp.dev` URLs and brand assets.
- The **server, Warp Drive backend, hosted auth, and Oz** (agent orchestration) are proprietary
  and absent from the repository. Nothing in this design attempts to reconstruct them.

## Non-goals

Stated explicitly because they are the most likely misunderstandings:

1. **This does not unlock paid features.** Entitlements are enforced *server-side*
   (`app/src/workspaces/workspace.rs:496`, `app/src/workspaces/user_workspaces.rs:277`, HTTP 429
   handling at `app/src/server/server_api.rs:696`). Patching client-side gating removes upsell UI and dead
   code paths; it does not grant premium functionality, because the server refuses regardless.
   The value proposition is **privacy and independence, not free Pro**.
2. **This does not reimplement Warp Drive.** See Phase scope below.
3. **This does not fork the server.** There is no server to fork.

## Architecture

### The seam already exists

Upstream maintains an OSS build target at `app/src/bin/oss.rs` (`Channel::Oss`) which already sets:

```rust
telemetry_config: None,
crash_reporting_config: None,
autoupdate_config: None,
```

It is **not** clean: the same `ChannelConfig` still carries
`server_config: WarpServerConfig::production()` and `oz_config: OzConfig::production()`, so the
OSS build still reaches `app.warp.dev` and `oz.warp.dev`.

The fork's job is therefore **to harden a profile upstream already maintains**, not to invent one.
This matters for maintenance: the diff concentrates in a bin target and a channel config — files
that churn far less than `app/src/lib.rs` or `app/src/root_view.rs`.

### Core design move: the compiler is the egress auditor

In `crates/warp_core/src/channel/config.rs`, `telemetry_config`, `crash_reporting_config`,
`autoupdate_config`, and `mcp_static_config` are already `Option`. `server_config` and `oz_config`
are not. Change both to `Option`, and have the OSS profile pass `None`.

```rust
pub struct ChannelConfig {
    pub server_config: Option<WarpServerConfig>,
    pub oz_config: Option<OzConfig>,
    // ...
}
```

Rationale:

- **Type-level guarantee.** The endpoints do not exist in the OSS binary. This cannot be flipped
  by a config value or a server-pushed experiment.
- **Consumers become compile errors**, each consciously handled once. Bounded, mechanical cost.
- **Self-defending against upstream drift.** When upstream adds a server call, the fork *fails to
  build* rather than silently phoning home. This is a stronger guarantee than any audit and
  directly mitigates the drift risk.

Defence in depth: compile-time absence of endpoints, plus a runtime egress allowlist in
`crates/http_client`, plus a CI test asserting a cold start with no user config opens **zero**
outbound connections, plus a recurring Codex audit after each upstream rebase.

### Strategy: neuter at the source of truth, never at the call sites

Telemetry symbols appear in ~340 files and entitlement symbols in ~240. The fork must not touch
them. It overrides the small number of functions those call sites consult, leaving the call sites
byte-identical to upstream so they rebase cleanly.

Rejected alternatives:

- **Hard fork / deletion patches** — with 1.58M LOC and daily upstream commits, large deletion
  patches conflict repeatedly in `app/src/lib.rs`, `root_view.rs`, and cloud update code.
- **VSCodium-style patch stack** — same conflict problem, plus series-management ceremony.
- **Upstream-first** — good leverage where it applies, but cannot be relied on for changes that
  reduce Warp's telemetry and gating.

### Verified choke points

| Concern | Location | Change |
|---|---|---|
| Server + Oz endpoints | `crates/warp_core/src/channel/config.rs:8` | `Option`, `None` for OSS |
| Telemetry policy | `app/src/settings/privacy.rs:202` `should_disable_telemetry()` | unconditionally `true` |
| Telemetry transport | `app/src/server/telemetry/mod.rs` `TelemetryApi` (RudderStack) | no-op sink; queue never flushes |
| Collector registration | `app/src/server/telemetry/collector.rs:35` | never registered under OSS |
| Remote control | `app/src/server/experiments/mod.rs` `ServerExperiments` | never applied |
| Crash reporting | `warp_errors::report_error` (Sentry) | local log only |
| Auth | `crates/warp_server_auth/src/auth_state.rs:37`, `app/src/auth/auth_manager.rs:92` | logged-out as complete, first-class state |
| Drive / teams / sharing | `crates/cloud_object_*`, `app/src/lib.rs:2048` | not wired under OSS |
| Agent backend | `app/src/ai/agent/api/impl.rs:14` | `AgentBackend` trait; ACP implementation |

### The telemetry override, concretely

Upstream today:

```rust
pub fn should_disable_telemetry(&self) -> bool {
    !self.is_telemetry_enabled
        && !self.is_telemetry_force_enabled
        && !FeatureFlag::AgentModeAnalytics.is_enabled()
}
```

A user's telemetry opt-out is overridden when `is_telemetry_force_enabled` is set from team/server
data, or when the server-side `AgentModeAnalyticsExperiment` enables the `AgentModeAnalytics` flag
(`app/src/server/experiments/mod.rs:81`, applied from server state per that module's own header
comment). In the fork this function returns `true` unconditionally, and server experiments are
never applied at all.

### Agent replacement

Warp's agent harness is server-side and proprietary, so it cannot function in the fork. It is
replaced by an ACP (Agent Client Protocol) bridge to local CLI agents.

`app/src/terminal/cli_agent.rs` already detects and brands Claude Code, Codex, Gemini CLI, Amp,
Droid, and OpenCode, so detection is largely solved. The hard part is **semantic translation**:
Warp's UI consumes Warp-specific session, tool-call, transcript, and cancellation events, and the
current entry point is hardwired to `ServerApi` and `warp_multi_agent_client`.

The work is therefore: define an `AgentBackend` trait at `app/src/ai/agent/api/impl.rs:14`,
implement it over ACP, keep it strictly additive and isolated. This is the only place the fork
adds substantial code, and it is the highest-risk item — so it ships **after** a provably silent
terminal, not alongside it.

## Phases

Each phase is independently valuable and independently shippable.

1. **Baseline build.** Install the Rust 1.92.0 toolchain, bootstrap with
   `WARP_SKIP_GCLOUD_AUTH=1`, and build `Channel::Oss` unmodified. Establishes that 1.58M lines
   compile on this machine before any changes. Currently unproven — no toolchain is installed.
2. **Silence it.** The `Option` change, the choke-point overrides, no experiment application, no
   collector registration, Drive/teams/sharing unwired. Ends with the egress test passing.
3. **Verification harness.** Egress allowlist in `crates/http_client`, cold-start zero-connection
   CI test, rebase workflow against `upstream/master`, standing Codex audit for new network call
   sites and telemetry reintroduction.
4. **Rebrand to Heddle.** Application name, bundle identifier (`dev.warp.WarpOss` → Heddle's own),
   binary name (`warp-oss` → `heddle`), icons, removal of `warp.dev` URLs and brand assets, and a
   README documenting exactly what was removed and why.
5. **Release engineering.** GitHub Actions building macOS + Linux from `Channel::Oss`, with the
   egress test as a merge gate.
6. **ACP agent adapter.** The `AgentBackend` trait and ACP implementation.

Phases 1–3 produce the actual product thesis: a fast terminal that provably does not phone home.
Phases 4–5 make it distributable. Phase 6 makes it agentic again.

## Verification

The privacy claim is only credible if it is mechanically enforced:

- **Compile-time:** no Warp endpoints exist in the OSS binary; new upstream server calls break the
  build.
- **Runtime:** cold start with no user configuration opens zero outbound connections (CI-gated).
- **Per-rebase:** Codex audit pass hunting new network call sites, collectors, and telemetry.

A known instance of exactly the bug class this catches already exists upstream:
`crates/warp_tui/src/autoupdate.rs:489` requests `/client_version` *before* it rejects the OSS
channel.

## Risks

1. **ACP semantic mismatch** — translating Warp-specific agent events is the real work; process
   launching is trivial. Mitigated by sequencing it last, behind a trait.
2. **Baseline build unproven** — 1.58M LOC, no toolchain installed yet. Phase 1 exists to retire
   this risk before anything else.
3. **Upstream drift** — continuous new auth/telemetry/entitlement call sites. Mitigated by the
   compile-time guarantee and the standing audit.
4. **Entitlements have no single choke point** — scattered across `BillingMetadata`,
   `UserWorkspaces`, and server 429s. Mitigated by scoping to "remove upsell UI", not "unlock".
5. **Logged-out coherence** — a real logged-out route exists (`app/src/root_view.rs:1733`), but the
   TUI state machine models only awaiting-login / failure / logged-in
   (`app/src/tui/mod.rs:23`), and AI usage refresh assumes login
   (`app/src/ai/request_usage_model.rs:252`). Making logged-out *coherent* across the product is
   systematic work, not a single flag.
6. **Distribution burden** — macOS notarization requires an Apple Developer account (~$99/yr) or
   users hit Gatekeeper warnings. Deferred to Phase 5.

## Open decisions

- **macOS code signing** — whether to pay for Apple Developer notarization (~$99/yr) or ship
  unsigned with documented install instructions. Required before Phase 5; does not block
  Phases 1–4.

## Review process

Codex (`gpt-5.6-sol`, reasoning effort `xhigh`) is wired in as an independent reviewer at three
points: before major design decisions, after each implementation phase, and as a standing egress
audit after every upstream rebase. Its review of this design corrected three substantive errors in
the first draft, including the existence of `Channel::Oss` itself.
