# Heddle Phase 6 — ACP agent bridge

**Date:** 2026-07-22
**Status:** Designed, deliberately NOT implemented in v0.1
**Decision:** Heddle ships v0.1 as a terminal. It will not be described as an
agentic environment until permission handling and cancellation work end to end.

## Why this is deferred, not abandoned

Warp's built-in agent harness runs **server-side** and is proprietary (upstream's
FAQ states this plainly). Removing Warp's endpoints therefore leaves a real hole:
Heddle has no agent.

The temptation is to ship something that looks agentic quickly. That would be
dishonest in a specific and dangerous way: an agent that streams tool calls
without working permission prompts or cancellation can execute commands a user
did not agree to. A half-implemented agent bridge is worse than none.

So v0.1 ships without it, and the README and release notes say so in plain
words rather than implying capability.

## Foundation

The official Rust SDK exists and is licence-compatible:

- Crate: `agent-client-protocol` v1.3.0
- Licence: **Apache-2.0** — one-way compatible into AGPL-3.0, so it can be used
  here without licence conflict
- Repository: https://github.com/agentclientprotocol/rust-sdk

ACP is designed around exactly this shape: a local subprocess speaking JSON-RPC
over stdio, streaming updates, and bidirectional permission requests. This is
not a protocol we would be inventing.

## Minimum honest scope for v1

Deliberately narrow. Everything omitted is omitted on purpose.

**In scope:**

1. **One user-configured ACP executable.** Not auto-detection of every CLI.
   `app/src/terminal/cli_agent.rs` already recognises Claude Code, Codex,
   Gemini CLI, Amp, Droid and OpenCode — but *recognising a binary name is
   presentation logic, not protocol compatibility.* Only agents with a real ACP
   adapter are supported, and the user names the executable explicitly.
2. **Local subprocess over stdio**, spawned via the official SDK.
3. **Protocol surface:** `initialize`, `session/new` (with the terminal's CWD),
   `session/prompt`, streamed message/thought/tool-status updates, permission
   requests, cancellation, and clean handling of process failure.
4. **Explicit tool approvals**, surfaced in the UI. No silent execution.
5. **Advertise no capabilities we have not implemented** — no filesystem,
   terminal, MCP, persistence, resume, registry, multimodal, or orchestration.

**Out of scope for v1:** agent orchestration, cloud runs, shared sessions,
conversation sync, and anything else that only made sense with Warp's backend.

## Architectural constraint

The current entry point is hardwired to `ServerApi` and
`warp_multi_agent_client` at `app/src/ai/agent/api/impl.rs:14`.

**Do not make ACP masquerade as `ServerApi`.** Introduce a local-agent provider
boundary and put ACP behind it. The two have genuinely different shapes: one is
a hosted request/response service, the other a supervised local process with
its own lifecycle, permissions and failure modes. Pretending otherwise pushes
the mismatch into every call site.

The hard part is **not** launching a subprocess. It is translating ACP's event
stream into a conversation UI that was designed around a server, safely — with
cancellation that actually cancels and permission prompts that actually gate.

## Acceptance criteria

Heddle may describe itself as agentic only when all of the following hold:

1. A user-configured ACP agent starts, answers a prompt, and streams output.
2. A tool call requiring permission **blocks** until the user approves it.
3. Denying a permission prevents the action; the agent is told and continues.
4. Cancellation stops in-flight work promptly, and the agent process is left in
   a clean state (or is killed and reported).
5. Agent process crash or non-zero exit surfaces as a clear error, not a hang.
6. `script/heddle/verify-no-warp-endpoints` still passes — the bridge must not
   reintroduce any Warp endpoint.
7. No capability is advertised over the wire that is not implemented.

Until every one of these is demonstrable, the honest description stays
"a terminal".

## Consensus

This scope and the decision to defer were reviewed and endorsed by the Codex
evaluator (`gpt-5.6-sol`, reasoning effort `xhigh`), which independently judged
ACP "achievable, but defer it from v0.1" and warned specifically against
treating CLI-name detection as protocol compatibility.


## Consensus addendum (2026-07-23): scope, safety, and honest effort

After v0.1 shipped (Phases 1-5), the standing project goal — "all six phases
implemented" — required deciding how to proceed on Phase 6. This was put to the
Codex evaluator as a consensus decision. The agreed outcome:

**Implement toward full (option A), incrementally, seam-first. Phase 6 counts as
"implemented" ONLY when all seven acceptance criteria pass — not before.**

- **Land the `AgentBackend` seam now** as a behavior-preserving refactor. This is
  safe: an interface executes nothing.
- **Do NOT ship a partial, prompt-capable ACP backend.** A default-off runtime
  flag is explicitly judged *insufficient* protection — it creates "a hazardous
  latent backend" that could execute unapproved tool calls if ever reached. ACP
  must stay genuinely unreachable until permission-blocking, denial, cancellation,
  and crash handling all work end-to-end.
- **"Implemented by design" does not satisfy "all six phases implemented."**
  Calling the seam alone "Phase 6 done" would be dishonest.

**Honest effort estimate (one experienced engineer, from Codex):** 9-14
engineer-weeks plus review and soak.

| Work | Estimate |
|---|---|
| Seam + call-site decoupling | 1-2 weeks |
| ACP process/session lifecycle | 1-2 weeks |
| Stateful transactional translation (ClientAction: 13 variants, FieldMask, begin/commit/rollback transactions) | 3-4 weeks |
| Permission UI + blocking round trip | 2-3 weeks |
| Cancellation, crash recovery, tests, hardening | 2-3 weeks |

This is why Phase 6 was deferred from v0.1 and why it cannot be honestly
compressed into a checkbox. The translation target
(`warp_multi_agent_api::ResponseEvent` -> `ClientActions` -> 13 transactional
`ClientAction` variants) is a stateful, field-mask-based message-mutation
protocol, measured against the real proto at
`~/.cargo/git/checkouts/warp-proto-apis-*/apis/multi_agent/v1`.
