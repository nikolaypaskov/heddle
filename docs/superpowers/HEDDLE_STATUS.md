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

### Menu audit + privacy pass (owner-requested, after seeing the GUI run)

The de-commercialized GUI was built and run locally; the owner confirmed it
"looks good" but asked to (a) audit the menus for OSS-irrelevant items and (b)
keep going, framing Heddle as a **privacy-oriented** version. Done in this pass:

- **Warp Drive team-object creation** removed from the Drive menu / palette /
  keybindings (`New Team Workflow/Notebook/Prompt/Env Vars/Folder`); local
  `NewPersonal*` kept.
- **"Cloud Agent"** new-session entry and **"Share pane" / "Share current
  session"** Drive-menu items removed.
- **Slack-community** surfaces removed everywhere (Help menu, account menu,
  Resource Center footer, `JoinSlack` action, `SLACK_URL`, and the bundled Slack
  trademark logo).
- **Upstream links fixed:** `GITHUB_ISSUES_URL` and the feedback form pointed at
  `warpdotdev/Warp`; repointed to the fork, and feedback no longer auto-attaches
  OS/app version.
- **`/handoff` and `/remote-control` slash commands** removed.
- **Privacy fix (live-verified):** the OSS build was repeatedly attempting
  `fetching updated cloud objects`. Guarded all cloud-object / workspace-metadata
  pollers on `ChannelState::server_root_url().is_some()` — a hard no-op without a
  server. A fresh build now logs **zero** cloud-fetch/auth-fail lines and starts
  with `server_config: None, oz_config: None, telemetry_config: None,
  crash_reporting_config: None`.

### Comprehensive flag neutralization (privacy-oriented, live-verified)

Rather than risk the working terminal by surgically unpicking deeply-woven,
mixed cloud/local modules, the remaining reachable cloud/commercial/telemetry
surfaces were **neutralized at their feature gates** — every one is a graceful
runtime `.is_enabled()` check, so the code stays compiled but dormant and the
feature never activates. Disabled across `app/src/features.rs` +
`update_session_sharing_enablement`:

- **Session sharing** (create + view + ACLs + shared-with-me + remote-control):
  CreatingSharedSessions forced off without a server; ViewingSharedSessions,
  SharedWithMe, SessionSharingAcls, SharedSessionWriteToLongRunningCommands,
  HOARemoteControl dropped.
- **Oz cloud agents / ambient**: CloudMode family, AgentManagementView/Details,
  Sync/Scheduled/CommandLine/ImageUpload/RTC ambient agents, OzPlatformSkills,
  OzIdentityFederation, OzChangelogUpdates, CreateEnvironmentSlashCommand,
  Cloud{Environments,Runners,AgentRunners}.
- **Cloud handoff**: OzHandoff, HandoffLocalCloud, HandoffCloudCloud.
- **Cloud conversation sync**: ConversationApi, CloudConversations.
- **Cloud secrets / team**: WarpManagedSecrets, TeamApiKeys.
- **Telemetry / billing**: GlobalAIAnalytics{Collection,Banner},
  RecordAppActiveEvents, UsageBasedPricing, DriveObjectsAsContext,
  BillingAndUsagePageV2 (dead).

**Live verification:** a fresh build logs **zero** cloud/telemetry/ambient/oz/
fetch activity and starts with every backend config `None`. The "Hand off to
cloud" / "Share session" chips, "Agent Management" toolbar, and "Cloud agent"
selector no longer render. No local terminal / Agent Mode / BYOK / CLI-agent /
workflow / notebook / MCP capability is affected.

Left enabled (verified local, not Warp-cloud): CrossRepoContext, RemoteCodeReview,
WarpifyFooter, McpServer, SendTelemetryToFile (local file; telemetry_config is
None so nothing ships).

### Physical deletion of the now-dormant code (in progress)

Every deep module intermixes cloud with shared/local code — e.g.
`agent_management_model.rs` holds both the cloud panel model and the general
`AgentNotificationsModel` used by the tab bar; `TerminalModel::shared_session_
status()` is threaded through core rendering. Physically deleting the dormant
flag-gated code is therefore careful, file-splitting surgery, done module by
module (session sharing, Warp Drive cloud objects, Oz/ambient runtime) — plus the
P0 agent transport. The build is fully usable and privacy-clean in the meantime.

**Agent-management panel — DONE (`5a8c33dd`, `0a238c53`).** The first module
physically removed. The Oz cloud-agent management view (`AgentManagementView`,
~2.4k lines) and its agent-type selector (475 lines) are deleted, along with the
full **local-control protocol cascade** that had blocked an earlier attempt:
`ActionKind::SurfaceAgentManagementOpen` is gone from the `local_control`
catalog, the `warp_cli` `surface agent-management` subcommand, the app
bridge/`app_state` dispatch, and `SurfaceDestination::AgentManagement` in the
metadata handler (catalog action count 84 → 83). The ~40 workspace-crate sites
(view handle, close-on-activate gates, state setter, toolbar button +
`HeaderToolbarItemKind::AgentManagement`, `cmd-shift-M` binding, and the
Toggle/Open/ViewAgentRuns/CloudAgentSetupGuide actions) are removed; the two
deeplink entry points (`environments_page` "View my runs", `uri` `CloudAgentSetup`)
are inert. The dead cloud setup-guide sub-view (~680 lines of Oz onboarding copy)
is deleted too, its tiny `SetupGuideDocs` enum relocated into `telemetry.rs`.
**Kept** (not cloud surfaces): `AgentNotificationsModel` / `AgentManagementEvent`,
telemetry, details-action-buttons, and the now-dormant
`FeatureFlag::AgentManagementView`. `warp` lib suite: 5777 passed / 13 failed
(the 13 are pre-existing isolation failures); `local_control` 40, `warp_cli` 204.

A Codex `gpt-5.6-sol` (xhigh, read-only) adversarial review of the removal
initially **rejected** it for one shipped-surface gap and flagged two robustness
items — all now fixed:

- **(blocking) bundled `warpctrl` skill** still advertised `surface
  agent-management open`; agents following the default-enabled skill would hit a
  Clap error. Removed from `resources/bundled/skills/warpctrl/SKILL.md`, and the
  control-CLI spec (`specs/warp-control-cli/*`) updated 84 → 83 actions with the
  `surface.agent_management.open` entry dropped.
- **(robustness) settings forward-compat:** removing a persisted enum variant
  (`HeaderToolbarItemKind::AgentManagement`) previously made the *whole* custom
  header-toolbar layout fail to decode and reset to default. Fixed with an
  **opt-in** tolerant `SettingsValue` impl on `HeaderToolbarChipSelection` that
  drops only unknown toolbar items (mirroring the derive's wire format). A first
  attempt made `Vec<T>::from_file_value` globally tolerant, but Codex correctly
  flagged that as unsafe — it would silently swallow malformed elements in
  security-sensitive list settings (the command-execution denylist, custom
  secret-redaction regexes), so a dropped deny-rule could vanish unnoticed. The
  global decoder stays strict; only the toolbar layout opts in. Tested
  (round-trip + drop-unknown-item + reject-bad-element).
- **(UX) inert "View my runs" link** on the environments page removed outright
  (label + separator + mouse-state plumbing), rather than left as a dead
  click/hover target.

Codex confirmed no egress guard was added or removed by the range and the
protocol code removal is otherwise consistent. Next candidate under
investigation: the `AgentConversationsModel` + "Agent conversations"
conversation-list panel + Oz ambient-agent cloud fetches
(`list_ambient_agent_tasks`). An Explore pass found this is **preserve-local
surgery, not clean deletion** (~35–40 files): the model is a deliberate
local+cloud aggregator whose cloud half is already inert in OSS (constructor
no-ops when `AgentManagementView` is off; the three ambient `ServerApi` calls
hard-fail with no egress when `server_root_url()` is `None`), but whose local
conversation-history half still feeds four local-serving surfaces (the panel, the
terminal inline resume-conversation menu, slash commands, the @-conversation
context menu) that the fork wants to keep.

### Boundary reached: clean removals vs. preserve-local surgery (historical)

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
