# Heddle de-commercialization audit (Codex, gpt-5.6-sol, max reasoning)

**Date:** 2026-07-23
**Auditor:** Codex `gpt-5.6-sol`, reasoning effort `max`, read-only
**Verdict:** The tree is **not yet honestly de-commercialized.** `server_config: None`
(`app/src/bin/oss.rs:19`) blocks backend access, but commercial features remain
**default-compiled** (`app/Cargo.toml:490`) and compiled flags are enabled
wholesale (`app/src/features.rs:6-12`). Result: visible UX that fails, no-ops, or
asks for Warp accounts/payment.

This is the standing work-list for finishing the FOSS transition. Order of
attack: P2 (self-contained paywalls) → P1 (teams/sharing/drive/cloud history) →
P0 (agent transport, Oz). Each removal gets a Codex egress/sign-off pass.

## Prioritized findings

### P0 — Warp-hosted Agent/AI transport (deepest)
BYOK keys/custom providers/routers are still packaged into a request sent through
`warp_multi_agent_client` via `ServerApi`: `app/src/ai/agent/api/impl.rs:56,108,141`.
Model discovery server-backed: `app/src/ai/llms.rs:1644`. Quotas/hosted
generation/embeddings/legacy AI APIs: `app/src/server/server_api/ai.rs:1124`. AI
onboarding forces Warp login: `app/src/root_view.rs:2159`.
→ **Neutralize** hosted transport/account/catalog/quota; retain local Agent UI,
tools, harnesses, BYOK. (Overlaps Phase 6 ACP.)

### P0 — Oz / cloud agents, handoff, hosted automation
"New Oz cloud agent" (`app/src/ai/blocklist/agent_view/zero_state_block.rs:402,751`),
handoff/remote-control toolbar (`.../agent_input_footer/toolbar_item.rs:209`),
agent management (`app/src/workspace/header_toolbar_item.rs:31`), Cloud Platform
settings (`app/src/settings_view/mod.rs:324`, `environments_page.rs:1998`).
CLI describes proprietary-backend ops (`crates/warp_cli/src/lib.rs:109`).
Schedules/secrets/runners/keys/federation/artifacts route through
`app/src/ai/agent_sdk/mod.rs:182`. TUI remote spawning
`crates/warp_tui/src/orchestration_model.rs:337`.
→ **Remove** cloud task mgmt/environments/schedules/secrets/runners/handoff UI
(leaves); **neutralize** local agent orchestration.

### P1 — Warp Drive, Knowledge, cross-device settings sync
Drive advertises cloud/team workflows/notebooks/prompts/env vars
(`app/src/settings_view/warp_drive_page.rs:235`). Cloud-object create/share/tier:
`app/src/drive/index.rs:3061,4450,4953`. Sync: `app/src/server/cloud_objects/update_manager.rs:188`.
Knowledge rules use cloud objects: `app/src/ai/facts/view/rule.rs:170`. Settings
sync: `app/src/settings_view/main_page.rs:680`, `app/src/settings/cloud_preferences_syncer.rs:234`.
→ **Remove** cloud clients/share/team/tier UI; **preserve+localize** workflows,
notebooks, prompts, env vars, rules, settings-file persistence.

### P1 — Teams, account management, commercial policy metadata
Teams settings always render + fetch hosted state
(`app/src/settings_view/teams_page.rs:1819`), incl. billing/plan/seat/invite/upgrade
(`:2377,2511,3014`). Backend: `app/src/server/server_api/team.rs:69`.
`UserWorkspaces` owns invitations/hosted ops (`app/src/workspaces/user_workspaces.rs:1034`)
and gates sharing via team billing metadata (`:1612`).
→ **Remove** Teams UI/API/update managers; **reduce** `UserWorkspaces`/ownership/
current-workspace to local/personal semantics.

### P1 — Session sharing, remote control, block sharing
Shared-session viewing default-built; creation enabled when missing tier metadata
defaults to allowed (`app/src/workspaces/user_workspaces.rs:1627`). Paths:
`app/src/terminal/view/init.rs:928`, `app/src/terminal/shared_session/mod.rs:329`,
URI `app/src/uri/mod.rs:226`. Block sharing live: context-menu
`app/src/terminal/view.rs:16635`, upload modal `app/src/terminal/share_block_modal.rs:312`,
API `app/src/server/server_api/block.rs:75`.
→ **Remove** upload/permalink/network/ACL/role modules; **keep** terminal blocks/
input/model.

### P1 — Cloud conversation history, sharing, TUI resume
Loaders fetch hosted conversations (`app/src/ai/blocklist/history_model/conversation_loader.rs:105`);
Privacy exposes "Store AI conversations in cloud" (`app/src/settings_view/privacy_page.rs:1649`);
links `app/src/uri/mod.rs:258`. TUI `--resume` accepts Warp/Oz tokens
(`crates/warp_tui/src/session.rs:27`).
→ **Remove** cloud adapters/tokens/metadata sharing/cloud retention pref; **retain**
local SQLite history/export/restore.

### P2 — Remaining paywalls, paid onboarding, marketing modals (self-contained, safe)
- `WorkspaceAction::ShowUpgrade`: `app/src/workspace/action.rs:315`, handler `app/src/workspace/view.rs:24096`.
- `UserWorkspaces::upgrade_link*`: `app/src/workspaces/user_workspaces.rs:192` (return `None` w/o server → dead callers).
- `free_ai_removal_modal`: `app/src/workspace/view/free_ai_removal_modal.rs:32`, triggered by backend quota `app/src/workspace/one_time_modal_model.rs:510`.
- Out-of-credit "Subscribe" CTA: `app/src/ai/blocklist/block/view_impl/common.rs:3034`.
- Paid first-run "Starting at $18/mo": `crates/onboarding/src/slides/ai_access_slide.rs:257`; upgrade action no-op at `app/src/root_view.rs:2269`.
- Default-on Oz + orchestration launch marketing: `app/src/workspace/view/launch_modal/oz_launch.rs:77`, `app/src/workspace/view/orchestration_launch_modal/view.rs:270`.
- Build-plan migration modal (credit auto-reload + paid plan ads): `app/src/workspace/view/build_plan_migration_modal.rs:353,500,779`.
→ **Remove entirely** (generic provider-error handling stays, stripped of quota/subscription actions).

## Safe to rip out entirely
Oz/cloud task mgmt, handoff, environments, runners, schedules, managed secrets,
platform keys + hosted clients · Teams mgmt/invite/billing pages+APIs · Block
permalink upload + shared-session networking/ACL/role UI · Cloud-only conversation
loaders, server-token resume, sharing adapters · Warp signup/billing UI, paid
onboarding, upgrade actions, pricing/capacity/launch/migration modals · Drive
sharing/team/tier UI + cloud/settings-sync clients.

## Neutralize, not delete
Agent Mode, tool execution, local orchestration, BYOK config · Workflows,
notebooks, prompts, env vars, Knowledge/rules, local persistence · Local
conversation history/export/restore · Terminal blocks, terminal input/model,
workspace/owner primitives.

## Referrals
No reachable referral/invite-a-friend feature survives. Remaining matches are
generated GraphQL artifacts (`crates/graphql/src/api/mutations/send_referral_invite_emails.rs:7`),
stale compat state, and reward-theme variants explicitly hidden at
`app/src/themes/theme_chooser.rs:170` — none is a current feature.

## Structural note (both P0s hinge on this)
Commercial capability is gated at compile time by `app/Cargo.toml` default
features + `app/src/features.rs` wholesale flag enabling. A durable fix trims the
default feature set and the enabled-flag list, not only per-call-site deletions.
