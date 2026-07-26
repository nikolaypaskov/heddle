# Egress baseline evidence — unmodified `Channel::Oss`

**Date:** 2026-07-22
**Artifact:** `target/debug/warp-tui-oss`, built from upstream `a66337f4` with no fork changes
**Conditions:** empty `HOME`, no user config, logged out (no keychain available), ~25s run
**Source:** the application's own log, `$HOME/Library/Logs/oz/warp-tui.log`

This is the Task 2 deliverable: proof that the unmodified OSS build phones home.

## Confirmed egress

| # | Evidence | Meaning |
|---|---|---|
| 1 | `[warp::server::server_api] Fetching channel versions and changelogs from Warp server` | Outbound HTTP request initiated |
| 2 | `[warp::server::server_api] Received channel versions from Warp server: dev: … preview: … stable: …` | **Round trip completed.** Warp's servers responded with release metadata |
| 3 | `[warp::server::cloud_objects::listener] Attempting to start websocket connection in CloudObjects::Listener` | Warp Drive websocket connection attempted |
| 4 | `[…listener] websocket connection failed to connect or finished with an error; trying again: missing authentication credentials` | The websocket reached Warp and was rejected for lack of credentials — i.e. it connected far enough to be refused, and **retries** |
| 5 | `[warp::auth::auth_manager] Unable to persist user to secure storage` | An authentication flow ran despite the user never signing in |

## The most important finding

```
[warp::settings::privacy] Warp Drive privacy preferences are set, using those for
    telemetry=true, crash_reporting=true, cloud_conversation_storage=true
```

The client fetched **privacy preferences from Warp's server** and applied them, setting
`telemetry=true` and `crash_reporting=true` — on a build whose `ChannelConfig` has
`telemetry_config: None` and `crash_reporting_config: None`.

This is stronger evidence than the static analysis in the design spec. It is not merely that a
server-side experiment *could* override the user's telemetry opt-out (`privacy.rs:202`); it is that
on a cold, logged-out start the client **actually fetches server-supplied privacy state and turns
telemetry on**. Nulling the telemetry transport is demonstrably not sufficient — the policy input
itself arrives from the network.

## Embedded endpoints, confirmed at runtime

The startup line logs the full resolved `ChannelConfig`:

```
channel: Oss
server_config: WarpServerConfig {
    server_root_url: "https://app.warp.dev",
    rtc_server_url: "wss://rtc.app.warp.dev/graphql/v2",
    session_sharing_server_url: Some("wss://sessions.app.warp.dev"),
    firebase_auth_api_key: "AIzaSyBdy3O3S9hrdayLJxJ7mriBR4qgUaUygAs",
}
oz_config: OzConfig { oz_root_url: "https://oz.warp.dev" }
telemetry_config: None
autoupdate_config: None
crash_reporting_config: None
version: None
```

This directly validates the design's central decision: the OSS build carries Warp's production
endpoints and a Firebase API key. Making `server_config` and `oz_config` `Option` and passing
`None` removes them from the binary.

## Codex's finding #4, confirmed empirically

```
[warp_tui::autoupdate] TUI autoupdate disabled: no release version tag baked into this build
```

Exactly as Codex predicted from `autoupdate.rs:210`. A plain `cargo build` artifact has the
autoupdater switched off, so any egress test run against it **cannot observe autoupdate traffic**.
Testing the debug artifact would have measured the wrong thing. The `GIT_RELEASE_TAG` +
`versions/<ver>/` staging in `script/heddle/egress-test` exists precisely to prevent this.

## Instruments: what worked and what did not

- **The application's own log — worked.** Names the operations semantically.
- **`sandbox-exec` with `(deny network*)` — worked as a liveness harness.** The process survived
  the full 25s (exit 142 = SIGALRM), proving the run was long enough to be meaningful.
- **`lsof` polling — FAILED, and its results were discarded.** It reported zero remote endpoints
  during a run in which the log proves a completed round trip to Warp. Short-lived connections and
  child processes (the app spawns a separate terminal server process) evade 0.5s sampling. An
  earlier attempt was worse still: an empty PID variable made `lsof -p ""` dump every process on
  the machine. Neither result is used as evidence here.
- **`dtrace` — unavailable.** SIP is enabled.
- **`tcpdump` — unavailable unattended.** Requires an interactive sudo password.

## The asymmetry that still requires a syscall-level harness

Log-based evidence proves egress **exists**. It cannot prove egress is **absent**: a connection the
app does not log would not appear, so a clean log is not a clean process.

Proving the absence of egress — the actual Heddle guarantee — still needs observation below the
application, at the syscall or packet level. That is what `script/heddle/egress-test` (Linux
container + `strace`) is for, and why it remains in the plan despite this result. The alternative
on macOS is `sudo tcpdump`, which needs an interactive password.

**In short:** this document is the failing test. The Linux harness is still required for the
passing one.

## Side effect disclosure

The network-enabled runs let the unmodified client contact Warp's servers, including an
anonymous authentication attempt. Warp's servers may therefore hold an anonymous record from this
machine dated 2026-07-22. No user account was signed in.
