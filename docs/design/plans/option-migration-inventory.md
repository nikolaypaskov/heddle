# `ChannelConfig` `Option` migration — blast radius inventory

**Date:** 2026-07-22
**Branch:** `option-migration-inventory` (a measurement spike; it does **not** compile end-to-end yet)
**Purpose:** retire the largest unknown in the Heddle design — how far does making
`server_config` and `oz_config` `Option` actually reach?

## Verdict

**101 call sites**, confirmed empirically at **92 compile errors** across 8 crates. Bounded,
mechanical, and concentrated behind an accessor layer that already existed. The choke-point
strategy in the design spec is sound and the migration should proceed.

**77% of the remaining work is a single pattern.** Of the 70 errors in the `warp` app crate, 54 are
`Option<Cow<str>> doesn't implement Display` — i.e. a URL being interpolated into a `format!`. Each
needs the same shape of guard; only the *handling* differs.

| Error class | Count | Nature |
|---|---|---|
| `Display` on `Option<Cow<str>>` | 54 | URL interpolation — the dominant pattern |
| `mismatched types` | 8 | Direct assignment/return |
| `no method` (`into_owned`, `contains`) | 5 | Method called on the `Option` |
| **non-exhaustive match** | **2** | **The compiler forcing a decision on the new error variants** |

Those last two are the design working as intended: adding `GraphQLError::NoServerConfigured` and
`warp_multi_agent_client::Error::NoServerConfigured` made every error-handling site declare what
"no Warp server" means, rather than silently falling into a catch-all.

For contrast, the failure mode we were testing for was "thousands of call sites touching the config
directly", which would have meant abandoning the compile-time guarantee in favour of a runtime-only
kill switch.

## Distribution

| Accessor | Call sites |
|---|---|
| `server_root_url()` | 73 |
| `oz_root_url()` | 11 |
| `rtc_http_url()` | 6 |
| `ws_server_url()` | 4 |
| `session_sharing_server_url()` | 3 |
| `workload_audience_url()` | 1 |
| `firebase_api_key()` | 1 |
| `iap_config()` | 1 |
| `server_root_domain()` | 1 |
| **Total** | **101** |

| Crate | Call sites |
|---|---|
| `app/src` | 87 |
| `crates/warp_server_client` | 4 |
| `crates/http_client` | 4 |
| `crates/warp_tui` | 2 |
| `crates/warp_multi_agent_client`, `remote_server`, `isolation_platform`, `graphql` | 1 each |

## Why it stayed contained

`ChannelState` already exposed an accessor layer (`server_root_url()`, `ws_server_url()`,
`oz_root_url()`, …) rather than letting consumers read `config.server_config` directly. Changing
the field produced **13 errors, all inside `crates/warp_core/src/channel/state.rs`**. Every other
crate breaks only at the accessor return type, which is exactly where we want the decision to be
forced.

## Work completed on the spike

| Crate | Errors | Resolution |
|---|---|---|
| `warp_core` | 13 | Accessors return `Option`; `server_root_domain() -> Option<Origin>`; `uses_staging_server()` returns `false`; the three `override_*` setters became no-ops with no config |
| `http_client` | 1 | `.flatten()` over the candidate origins |
| `isolation_platform` | 1 | Workload-token issuance returns `Err` with no audience |
| `graphql` | 1 | New `NoServerConfigured` variant; `build_graphql_request` returns `Result<_, GraphQLError>` and **refuses to build a request** rather than substituting a URL |
| `remote_server` | 1 | `download_url`/`install_script`/`download_tarball_url` return `Option` |
| `warp_server_client` | 5 | OAuth device-flow client is `Option`; Firebase token fetch bails; public API bails; new `GraphQLError` variant handled |
| `warp_multi_agent_client` | 1 | **The Oz endpoint.** `endpoint_url` returns `Option`; the call site returns `Error::NoServerConfigured` |
| `warp` (app) | **70 remaining** | Not yet done — see below |

The `warp_multi_agent_client` change deserves note: `endpoint_url` builds the address of Warp's
proprietary multi-agent (Oz) backend. With `server_config: None` it returns `None`. The Oz
integration is not disabled by a flag someone could flip — **it has no address to call.**

### Remaining work, by file

| File | Errors |
|---|---|
| `app/src/server/server_api.rs` | 16 |
| `app/src/auth/auth_manager.rs` | 5 |
| `app/src/remote_server/ssh_transport/installation/scp_fallback.rs` | 4 |
| `app/src/ai/conversation_details_panel.rs` | 3 |
| `app/src/ai/agent_sdk/ambient.rs` | 3 |
| `app/src/workspaces/user_workspaces.rs` | 2 |
| `app/src/settings_view/privacy_page.rs` | 2 |
| `app/src/settings_view/show_blocks_view.rs` | 2 |
| ~20 further files | 1–2 each |

### A measurement trap worth recording

`cargo check -p <crate>` standalone resolves Cargo features differently from checking the same
crate through `warp_tui`. Checking `warp_multi_agent_client` in isolation produced a spurious
`E0599: Instrumented<...> doesn't satisfy Stream` (the `tracing-futures/futures-03` feature is
enabled elsewhere in the full graph). It vanishes under the full check. **Always verify a crate
through the real binary target before "fixing" an error that appeared when you changed how you were
looking.**

## Handling categories

Every site resolved so far fell into one of four patterns. Plan 2 should classify all 101 against
these rather than treating each as bespoke.

**A — "Is this a Warp origin?" → `false` when absent.**
`crates/http_client/src/lib.rs:403` `is_warp_server_origin`. With no config nothing matches, so
Warp's custom headers (`X-Warp-Client-Version`, OS name/version, client ID) and IAP tokens are
never attached to *any* request. This is a privacy improvement that falls out of the type change
rather than needing its own task.

**B — "This operation requires a backend" → return `Err`.**
`crates/isolation_platform/src/namespace.rs:27` issues a Warp workload-identity token. With no
audience the operation is not degraded, it is inapplicable, and failing cleanly is correct.

**C — "Construct a URL to talk to Warp" → must not construct one.**
`crates/graphql/src/client.rs:93` (the GraphQL endpoint) and
`crates/remote_server/src/setup.rs:607` (the CLI download URL). These are the interesting cases:
the surrounding function often returns an error type we cannot easily construct
(`Result<Request, reqwest::Error>`), so the fix needs a signature change rather than a local edit.
**Do not** paper over these by substituting a placeholder or unroutable URL — that reintroduces the
egress path and hides the failure. Expect a handful of signature changes here.

**D — "Environment predicate" → `false`/`None`.**
`ChannelState::uses_staging_server()`. A build with no server uses no server, staging included.

## What this means for the plan

1. **The migration is the primary mechanism, as the spec claimed.** Proceed with it.
2. **Tasks 3–6 shrink to defence in depth.** Several leaks we observed at runtime — the
   channel-versions fetch, the Drive websocket, `fetch_or_update_settings` — are built from
   `server_config`. Removing it turns those into compile errors rather than behaviours needing
   individual overrides. Re-evaluate whether Task 3's `http_client` tripwire is still worth its
   diff once category A is in place.
3. **Category C needs design attention**, not mechanical translation. Budget for signature changes
   in `graphql` and `remote_server`.
4. **87 of 101 sites are in `app/src`**, so the bulk of the work is one crate and can be done in a
   single focused pass.

## Status of this branch

`option-migration-inventory` is a spike, not a deliverable. `warp_core`, `http_client`, and
`isolation_platform` compile; `graphql` and `remote_server` do not, and the `warp` app crate has
not been reached. It exists to produce the number above. Plan 2 should start from a clean branch
and work through the 101 sites by category.
