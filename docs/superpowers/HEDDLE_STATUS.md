# Heddle — project status

**As of 2026-07-23.**

## Delivered: v0.1.0 (Phases 1–5)

A de-commercialized, trademark-clean, telemetry-free fork of the Warp terminal.
Tagged `v0.1.0`, published via the release workflow (Linux x86_64; macOS builds
from source). Signed off by the Codex evaluator across seven adversarial review
rounds.

| Phase | State |
|---|---|
| 1 · Baseline build | ✅ verified |
| 2 · Endpoint/credential removal | ✅ scanner-enforced, CI-gated |
| 3 · Verification harness | ✅ string scan + asset-hash manifest, both self-tested |
| 4 · Rebrand | ✅ name, icon, logo, fonts, bundle ID, paths, copy |
| 5 · Release engineering | ✅ Linux release + privacy/asset gates |
| 6 · ACP agent | ⏳ **designed, not implemented — next milestone** |

## Post-v0.1 de-commercialization sweep (in progress, 2026-07-23)

A Codex `gpt-5.6-sol` (max reasoning) feature audit found the tree **not yet
honestly de-commercialized**: commercial features are still default-compiled and
flag-enabled, so backend-gated UX fails/no-ops/asks for accounts. Full work-list
and remove-vs-neutralize guidance:
`docs/superpowers/specs/2026-07-23-decommercialization-audit.md`.

Progress on the sweep:

| Feature | State |
|---|---|
| Referrals / refer-a-friend / rewards | ✅ removed (`2635768b`) |
| Billing & Usage page, buy-credits banner, auto-reload modal, /usage /cost, Upgrade menu, prompt-alert credit CTAs | ✅ removed (`3d5ff762`) |
| P2 · upgrade/paywall residues (ShowUpgrade, out-of-credits CTA, frontier-models footer, free_ai_removal_modal, upgrade toast) | ✅ removed (`9bf250a1`) |
| P2 · Build-plan migration modal | ✅ removed (`7b87ba65`) |
| P2 · paid onboarding ($18/mo slide) + command-search upgrade CTA | ✅ removed (`f81d4209`) |
| P2 · Oz / orchestration launch marketing modals | ⏳ (removed with Oz feature) |
| P1 · Teams, Warp Drive, session/block sharing, cloud conversation history | ⏳ |
| P0 · Warp-hosted agent/AI transport, Oz cloud agents | ⏳ (overlaps Phase 6) |

The self-contained **P2 paywall/paid-onboarding tier is complete** and passed an
independent Codex sign-off (2026-07-23); the only P2 items left are the Oz/
orchestration launch modals, which are that feature's marketing and go with the
Oz removal (P0). `UserWorkspaces::upgrade_link[_for_team]` remain as
`None`-returning stubs for callers inside not-yet-removed features (teams,
ai_page); they're removed with those features. Privacy scanners
(`verify-no-warp-endpoints`, `verify-bundled-assets`) still PASS.

## Phase 6 decision (owner call, 2026-07-23)

The standing "all six phases" goal surfaced a genuine conflict: Phase 6 (the ACP
agent bridge) is a **9–14 engineer-week** feature that Codex judged must **not**
ship partially — a default-off flag is insufficient protection against a latent
backend executing unapproved tool calls.

**The owner chose: ship v0.1 now; Phase 6 remains a fully-specified future
milestone, not claimed as implemented.** This is the honest resolution Codex
endorsed. "Implemented by design" was explicitly rejected as satisfying "all
phases implemented" — so the goal is understood as *v0.1 shipped; Phase 6 next*,
not *all six done*.

Full Phase 6 design and effort breakdown:
`docs/superpowers/specs/2026-07-22-acp-agent-bridge-design.md`.

## Verification you can run

```bash
cargo build -p warp_tui --bin heddle-tui --features standalone
./script/heddle/verify-no-warp-endpoints   # no warp.dev / key / telemetry in binary
./script/heddle/verify-bundled-assets       # no Warp trademarked images/fonts
```
