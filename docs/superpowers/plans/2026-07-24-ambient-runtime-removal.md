# Oz Ambient Cloud-Agent Runtime Removal Plan

> Multi-session, preserve-local surgery. **#1 constraint: do not break local
> Agent Mode (BYOK), the local terminal, the conversation-list panel, or local
> model/harness selection.** After every slice: build + full lib suite (baseline
> 5779 pass / 13 pre-existing isolation fails) + Codex `gpt-5.6-sol` review.

## Governing insight

`AmbientAgentViewModel::is_ambient_agent()` returns `true` unconditionally
(`terminal/view/ambient_agent/model.rs:861`). Across ~20 consumers the pattern is
uniform: `ambient_agent_view_model: Option<ModelHandle<AmbientAgentViewModel>>`
where **`None` == local pane, `Some` == cloud pane**, with an idempotent
`set_ambient_agent_view_model` setter and `is_some()`/`is_ambient_agent()` gates.
Cloud behavior is additive and Option-gated → most consumers collapse to their
existing local branch. Single wiring point: `terminal/input.rs:2380
attach_ambient_agent_view_model`.

**IMPORTANT (Codex, slice 1): the model CAN still be constructed in an OSS build**
via two entry paths — a `NewCloudAgentConversation` deeplink
(`uri/mod.rs:1030` → `workspace/view.rs:3851` → `pane_group/mod.rs:3235` →
`terminal/shared_session/viewer/terminal_manager.rs:381` → `terminal/view.rs:3091`
with `is_ambient_agent = true`) and a `type = "cloud"` tab config
(`tab_configs/tab_config.rs:365`). So `Some` is NOT impossible in OSS until those
entry paths are removed. **Slice 0 (do first): remove the ambient construction /
entry paths** (the `NewCloudAgentConversation` deeplink, `type="cloud"` tab
config, `AddAmbientAgentTab`, `EnterCloudAgentView`, spawn, and the `attach`
wiring) so `ambient_agent_view_model` is provably always `None`. Only then are the
consumer excisions (Slice 4) safe. Individual view deletions that are gated on a
sub-state the model never reaches (e.g. `is_in_setup()` — the model starts in
`Composing`) are safe regardless of construction.

## Do NOT touch (cloud-named but local/shared)

1. ~~`model_selector.rs` `ModelSelector`~~ — CORRECTED (Codex, slice 3a): this view
   was NOT a shared local picker. Its only constructor was the footer's
   `v2_model_selector`, gated on `FeatureFlag::CloudModeInputV2` (absent from every
   OSS flag set), so it was dead in OSS and deleted outright in Slice 3a
   (`7f4d130b`), not split. The real local Agent-Mode picker is the separate
   `ProfileModelSelector` (build_oz_menu_items → LLMPreferences); the inline picker
   is `InlineModelSelectorView`. Both untouched.
2. `terminal/profile_model_selector.rs` — local; only its ambient field is cloud.
3. `cloud_conversation_continuation.rs` `AIQueryRouting::Local` +
   `resolve_ai_query_routing` — single source of truth for local follow-up
   submission (`input.rs:4051,5789`, `tui.rs:72`). Keep the Local arm.
4. `AgentConversationsModel` `conversations`/local-history half (feeds 4 local
   surfaces). Only the `tasks` half goes.
5. `ai/harness_availability.rs`, `ai/agent_sdk/` local driver paths,
   non-ambient render arms in `rich_content.rs` / `block_list_element.rs`.

## Progress

- **Slice 1** (`742309e1`) — first-time-setup FTUX view. DONE, Codex-approved.
- **Slice 1b** (`d2200919`) — cloud-pane render leaves (loading_screen.rs, footer.rs,
  `render_ambient_agent_progress`, orphaned helper, stale comments). DONE, Codex-approved.
- **Slice 1c (block/ subtree)** — DEFERRED to Slice 6: the `AmbientAgentBlock` /
  `HarnessSessionHeader` `RichContentMetadata` rendering and `maybe_insert_setup_command_blocks`
  are embedded inside `view_impl.rs` event handlers that Slice 6 deletes wholesale, so peeling
  them separately is churn. Sites: `rich_content.rs:259-269` (2 variants),
  `block_list_element.rs:4335` (a `matches!` → make `true`), `view_impl.rs:397,~500,~828`
  (constructions + `maybe_insert_setup_command_blocks`), `view.rs:12000` (caller).
- **Slice 2 (composer selectors)** — DONE (`5a3763f4`), Codex-approved (4 rounds; the key
  round caught that the `AuthSecretFtuxView` + workspace "New API key…" modal are SHARED with
  local Agent Mode orchestration — restored those, removed only the cloud composer selectors +
  the dead delete-confirmation event chain). LESSON: the auth-secret FTUX is NOT cloud-only.
  Remaining orphans (delete_auth_secret, remove_deleted_auth_secret_entry,
  is_harness_auth_ftux_completed) left for a later slice.
- **Slice 0 (neutralize construction + creators)** — DONE (`68c723d4`), Codex-approved
  (3 rounds). Forced `TerminalView::new`'s upfront chokepoint to always yield a `None`
  `ambient_agent_view_model` (provably always None in OSS; no consumer panics). Removed
  46 now-unreachable cloud-mode tests + orphaned helpers/`_for_test` accessors (5731/13).
  KEY LESSON (Codex): construction-neutralization ALONE is insufficient — a would-be
  "cloud" pane whose VM is None renders an indefinite shell-less loading viewer, NOT a
  local pane. Must ALSO neutralize every OSS-reachable creator to degrade to a real
  local terminal: `type="cloud"` tab config → `PaneMode::Terminal`; the pane-restore
  dispatch folds `PaneMode::Cloud` into local `create_session`; persisted
  `LeafContents::AmbientAgent` snapshots restore as local terminals (retires
  `AmbientRestoreKind`, severs the restoration-queue feeder); the `NewCloudAgentConversation`
  deeplink is a logged no-op (its no-window branch was NOT CloudMode-gated — the real
  reachable gap Codex caught). All other viewer creators are CloudMode/`ViewingSharedSessions`-
  gated or need server-derived session/task events (unreachable: `oss.rs` server_config None).
- **Slice 3a (cloud-mode-V2 model selector)** — DONE (`7f4d130b`), Codex-approved.
  Deleted `model_selector.rs` (717 lines) + the footer's `v2_model_selector`
  field/construction/methods/render + the `/model` cloud-V2 branch + the input
  keymap term. Codex confirmed the view was cloud-only (never a shared local
  picker), `/model` still reaches the local `InlineModelSelectorView`, and the
  local `ProfileModelSelector` is untouched. Suite 5777/13 (unchanged baseline).
- **(historical mapping) Slice 2** — All cloud-only: `harness_selector`,
  `host_selector`, `auth_secret_selector`, `auth_secret_ftux_view`, `auth_secret_ftux_dropdown`,
  `delete_auth_secret_confirmation_dialog` are fields of the `Option<AmbientAgentViewState>` on
  `Input` (built only in `attach_ambient_agent_view_model`). Deleting the 6 files gives a
  ~23-error cascade to fix: input.rs imports (317-318), the `AmbientAgentViewState` fields
  (1754-1756), `build_harness/host/auth_secret_selector` (2180/2211/2292), their construction
  (2389, 2452-2466, gated by `is_cloud_mode_composer`), field reads (4130,4173,16158), the
  ambient event-handler arms that drive them; workspace/view.rs `AuthSecretFtuxView` modal
  (403,1153-1156,14803-14824); mod.rs re-exports; two comment refs in model.rs (257,1823).
  `host_picker_tests.rs` is a DIFFERENT (orchestration) host-picker — do not touch.

## Ordered slices (each must compile + pass suite + Codex)

- **Slice 0 — DONE (`68c723d4`, Codex-approved).** neutralize ambient construction / entry paths (do FIRST). Make
  `ambient_agent_view_model` provably always `None` in OSS: force the upfront
  construction chokepoint (`terminal/view.rs:3089`
  `is_ambient_agent.then(|| AmbientAgentViewModel::new(...))`) to never create the
  model, and neutralize the user-facing creators (`NewCloudAgentConversation`
  deeplink `uri/mod.rs:1030`, `type="cloud"` tab config `tab_config.rs:365`,
  `AddAmbientAgentTab`, `EnterCloudAgentView`). The lazy
  `ensure_ambient_agent_view_model` (`terminal/view.rs:7905`) is reachable only
  via cloud shared-session viewing (needs a server → unreachable in OSS); address
  it with the shared-session ambient path. Verify graceful degradation (no
  `.unwrap()`/`.expect()` on the VM anywhere — confirmed), so a "cloud" pane
  degrades to a local pane. Only after this are the Slice 4 consumer excisions
  safe. Risk MEDIUM (touches the pane/tab creation chain).
  NOTE: Slice 1 (below) was executed before Slice 0 because it is gated on a
  sub-state (`is_in_setup()`) the model never reaches, making it safe regardless
  of construction.
- **Slice 1 — cloud render leaves.** Delete `progress.rs`, `progress_ui_state.rs`,
  `tips.rs` (+`ai/agent_tips.rs:625-626`), `footer.rs`
  (+`terminal/view.rs:27444-27446`), `loading_screen.rs`, `first_time_setup.rs`
  (+`terminal/view.rs:2840,4129`), `block/` subtree + `view_impl.rs`
  `AmbientAgentBlock` rendering (+`rich_content.rs:259,269`,
  `block_list_element.rs:4335`). ~4000 lines. Risk MEDIUM (shares view.rs).
- **Slice 2 — cloud composer selectors.** Delete `harness_selector.rs`,
  `host_selector.rs`, `auth_secret_selector.rs`, `auth_secret_ftux_dropdown.rs`,
  `auth_secret_ftux_view.rs`, `delete_auth_secret_confirmation_dialog.rs`; remove
  `build_harness/host/auth_secret_selector` in `input.rs:2180-2479` + workspace
  modal (`workspace/view.rs:403,1153-1156,14803-14824`). ~3000 lines. MEDIUM.
- **Slice 3a — DONE (`7f4d130b`, Codex-approved).** Removed the ambient
  `model_selector.rs` (`ModelSelector`) outright: it was cloud-only (only ever
  built as the footer's `v2_model_selector`, gated on `CloudModeInputV2`, absent
  from OSS), NOT a shared local picker as originally believed. No split/relocate
  needed. The `/model` command still reaches the local `InlineModelSelectorView`.
- **Slice 3b — DONE (`9b9cbc44`), Codex-approved.** Stripped all ambient bits from
  `ProfileModelSelector` (the LOCAL model picker): removed the field/setter/`new()`
  param, the cloud-lock predicates + harness-model machinery + `SelectHarnessModel`
  action, and the `HarnessAvailabilityModel::Changed` subscription; collapsed every
  consumer to its local branch. Codex confirmed local selection + shared-session
  viewer/executor protection are unchanged. `universal_developer_input`'s ambient
  param kept (underscored) for the Slice 4 sweep. 5731/13.
  Original note: `profile_model_selector.rs`: drop the
  `ambient_agent_view_model` field, `set_ambient_agent_view_model`,
  `is_third_party_harness`, and the harness-model menu paths. This field is fed by
  the shared `attach_ambient_agent_view_model` fan-out (`input.rs`) — the SAME
  setter that wires the footer, status bar, slash/skills data sources, and inline
  model view — so it is NOT cleanly isolatable. Do Slice 0 first (make the
  view-model provably always `None`), then collapse 3b together with the Slice 4
  consumer excisions as one dead-branch sweep.
- **Slice 4a — DONE (`849d2a48`), Codex-approved (2 rounds).** Excised ambient from
  the two inline selector subsystems: `models/{data_source,view}.rs` (dropped field/
  setter/param + the `include_model_in_picker` custom-endpoint suppression, which was
  a no-op locally) and `skills/{data_source,view}.rs` (dropped field/setter/param +
  the `is_cloud_pane` "hide skills" guard). Removed their `set_*` calls from
  `attach_ambient_agent_view_model` + the `new()` ambient args in `input.rs`. LESSON:
  removing a field's only reader (its setter) makes the field never-read `dead_code`
  and any `let mut` it mutated `unused_mut` — grep ALL warning kinds, not just imports.
- **Slice 4b — DONE (`f2c7887d`), Codex-approved (4 rounds).** Excised ambient from all
  remaining consumers: maa (PRESERVED its VM-independent suppression fallback from live
  terminal state), display_chip/display, block + block/view_impl (impl split across two
  files — grep both!), status_bar (deleted both always-None cloud-setup renderers,
  promoted local branches), agent_input_footer (deleted cloud env-selector; DISSOLVED
  the then-single-variant `EnvironmentSelectorTarget` enum into a direct
  `HandoffComposeState` handle after `match_single_binding` fired), slash gui.rs
  (`is_cloud_mode` → `is_cloud_mode_v2` only), `/environment` → false. Rounds 2-4 were
  all clippy-only lints: LESSON — run the REAL gate
  (`cargo clippy -p warp --lib --tests -- -D warnings`) before every submission;
  `cargo check` misses `let_and_return`/`match_single_binding`/dead-code cascades.
  Crate-wide clippy baseline is ~82 pre-existing errors (dead cloud code for slices
  6-8); per-slice standard = no NEW violation in touched files. 5731/13.
- **Slice 4c — REMAINING (do WITH slice 5; owner approved combining).** Retire the
  fan-out root: `attach_ambient_agent_view_model` + `subscribe_to_ambient_agent_view_model`
  + `AmbientAgentViewState` + `Input::new`'s ambient param + UDI's underscored param +
  the `Input::ambient_agent_view_model()` getter and its ~12 reader sites (several feed
  `resolve_ai_query_routing` — the slice-5 overlap).
- **Slice 5 — REVISED SCOPE (with 4c).** The plan's "collapse `AIQueryRouting` to Local"
  is WRONG: `LiveRemoteVm` also serves shared-LOCAL-session viewers (forwarding
  follow-ups to the sharer), and the resolver's inputs (`is_shared_ambient_agent_session`,
  `is_conversation_transcript_viewer`, `ambient_agent_task_id`) all have VM-independent
  sources on `terminal_model`. Correct scope: remove only the always-None
  `ambient_agent_view_model` PARAMETER from `resolve_ai_query_routing` (+ callers),
  keeping all four variants and the session-sharing logic intact. Session-sharing
  removal is its own later project.
- **(historical) Slice 4b/4c scoping note.** The rest of Slice 4: slash-command
  data source (`gui.rs` — ripples through `GuiDataSourceArgs`; `is_cloud_mode` collapses
  to `self.is_cloud_mode_v2`, keeping the separate `is_cloud_mode_v2`/CloudModeInputV2
  flag for a later cleanup), agent_input_footer (env-selector + display-chip rendering),
  block/status_bar (cloud setup-status rendering), maa, display_chip, then the fan-out
  root (`input.rs` `attach_ambient_agent_view_model` + `Input::new` ambient param +
  `AmbientAgentViewState`) and `universal_developer_input`'s underscored param.
- **Slice 4 (original) — excise per-consumer Option fields** (LOW each): `block.rs`,
  `zero_state_block.rs`, `maa.rs`, `display_chip.rs`, `models/*`, `skills/*`,
  `slash_commands/data_source/gui.rs`, `universal_developer_input.rs`,
  `environment_selector.rs`, handoff button `agent_input_footer/mod.rs:2234-2239`.
- **Slice 5 — split submission router.** `cloud_conversation_continuation.rs`:
  collapse `AIQueryRouting` to `Local`; update `input.rs:4051,5789`, `tui.rs:72`,
  `agent_input_footer/mod.rs`. Risk MEDIUM (local submission).
- **Slice 6 — delete VM + bridges.** `model.rs`, `model_tests.rs`,
  `handoff/snapshot.rs`, `pane_group/child_agent/hydration.rs`,
  `pane_group/ambient_pane_restoration.rs`, `pane_group/mod.rs` alias +
  `PaneKind::AmbientAgent`; `terminal/view.rs` owner methods (7904-7967,
  `handle_ambient_agent_event`); `input.rs:2380`; delete `ambient_agent/` module.
- **Slice 7 — cloud back-half of `AgentConversationsModel`.** Remove `tasks`/
  `list_ambient_agent_tasks` half (`:909,981,1216,1793,1975`), preserve
  conversations/local-history half. Risk MEDIUM.
- **Slice 8 — agent_sdk cloud paths + orchestration + server methods.** Remove
  `ai/agent_sdk/ambient.rs`, ambient calls in `mod.rs`/`harness_support.rs`,
  `orchestration_viewer_model.rs:271`, then the 4 `ServerApi` methods
  (`server/server_api/ai.rs:1207-2571`). Must be last.

Realistic: 9 slices (0–8) ≈ 11-13 PRs. Highest-risk (most test coverage): 3, 5, 7.
Tests to update: `model_tests.rs`, `terminal/input_tests.rs:1531-1579`,
`terminal/view_tests.rs:77,102,1932`, `queued_prompts_tests.rs:34`,
`shared_session/view_impl_tests.rs`, `cloud_conversation_continuation_tests.rs`.
