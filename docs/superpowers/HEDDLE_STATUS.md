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
