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

1. `model_selector.rs` `ModelSelector` **None path** — local Agent-Mode model
   picker (`build_oz_menu_items` → `LLMPreferences`). SPLIT, keep local branch.
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
- **Slice 2 (composer selectors)** — MAPPED, ready. All cloud-only: `harness_selector`,
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

- **Slice 0 — neutralize ambient construction / entry paths (do FIRST).** Make
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
- **Slice 3 — split shared model pickers.** `model_selector.rs`: drop harness
  branch (532-602), keep `build_oz_menu_items`; relocate out of `ambient_agent/`.
  `profile_model_selector.rs`: drop ambient field + `is_third_party_harness`.
  Risk MEDIUM (local model picker).
- **Slice 4 — excise per-consumer Option fields** (LOW each): `block.rs`,
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
