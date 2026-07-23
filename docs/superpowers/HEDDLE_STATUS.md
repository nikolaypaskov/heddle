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
| P2 · Oz / orchestration / OpenWarp launch marketing modals | ✅ removed (`878db60f`) |
| P1 · Block sharing (modal, hosted API, settings page, context menu, telemetry) | ✅ removed (`4580b9b8`) |
| P1 · Teams settings page (plan/seat/invite/billing/upgrade UI + all triggers) | ✅ removed (`072dbc4b`) |
| P1 · Session sharing (116 files, terminal-core-woven) | ⏳ deep |
| P1 · Warp Drive (cloud sync/share over local workflows/notebooks — preserve-local) | ⏳ deep |
| P1 · Cloud conversation history (cloud loader/retention over local SQLite — preserve-local) | ⏳ deep |
| P0 · Warp-hosted agent/AI transport, Oz cloud agents | ⏳ (overlaps Phase 6) |

**The entire P2 tier is complete** (billing, paywalls, paid onboarding, and the
Oz/orchestration/OpenWarp launch marketing modals) and passed an independent
Codex sign-off (2026-07-23). The two P1 features that were cleanly *removable*
— block sharing and the Teams settings page — are also done. Privacy scanners
(`verify-no-warp-endpoints`, `verify-bundled-assets`) PASS after every
increment; the OSS TUI binary has shrunk from ~799 MB to ~794 MB across the
sweep. Every additive commercial *surface* the fork shipped is now gone.

### Boundary reached: clean removals vs. preserve-local surgery

The remaining P1 items are a **different class of work** from everything removed
so far. Block sharing, Teams, and all of P2 were *additive commercial surfaces*
that could be deleted outright. The rest — **session sharing** (woven through
terminal-core rendering across 116 files), **Warp Drive** and **cloud
conversation history** (cloud sync/share/retention layered *on top of* local
workflows, notebooks, prompts, and SQLite history that must be **kept and
localized**) — require careful architectural surgery to separate cloud from
local without breaking legitimate FOSS features. They do not yield clean
single-pass, always-compiling increments and are genuinely multi-session.

**P0 is the true "no Warp backend" core:** the app still routes AI (even BYOK)
through `warp_multi_agent_client`/`ServerApi`, and Oz cloud agents remain. Codex
estimated the agent-transport replacement (Phase 6 / ACP) alone at **9–14
engineer-weeks**. Reaching a fully backend-free, production-ready build is a
multi-week program, not a single session.

Residual stubs kept inert until their owning feature is removed:
`UserWorkspaces::upgrade_link[_for_team]` (→ `None`), plus dead team methods /
Drive-adjacent components (warnings only) that the Warp Drive removal will clear.

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
