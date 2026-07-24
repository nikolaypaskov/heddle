# Frequently Asked Questions

Heddle is a privacy-oriented, community fork of the open-source
[Warp](https://github.com/warpdotdev/Warp) client. It is **not** affiliated with,
endorsed by, or supported by Warp / Denver Technologies. This FAQ is about
Heddle; for Warp-the-product, ask Warp.

## What Heddle is

### What is Heddle?

Warp open-sourced its terminal **client** under AGPL-3.0. Heddle takes that open
client and removes its dependence on Warp's closed parts — the server, Warp Drive
backend, hosted authentication, telemetry, and Oz (the agent orchestration
layer). The result is a terminal with **no account, no telemetry, and no
`warp.dev` in the binary**. See the [README](README.md) for the full list of what
was removed and how it is verified.

### Can I use it fully offline, without signing in?

Yes — that is the entire point. Heddle has no sign-in, no cloud sync, and no
telemetry. There is no server for it to talk to. Local features (the terminal,
local shell integration, workflows, notebooks, BYOK local agents) work; anything
that required Warp's backend is gone, not merely hidden.

### Does this unlock paid Warp features?

No. Warp's paid entitlements are enforced server-side; removing paywall UI just
removes dead controls and nags. There is no server here to grant premium
functionality. The value is privacy and independence, not free Pro. See
[Non-goals](README.md#non-goals).

### Does Heddle have an AI agent?

Not yet. Warp's built-in agent runs on its proprietary server and cannot work
here. The intended replacement is a local bridge over
[ACP](https://agentclientprotocol.com/); it is designed but deliberately **not**
implemented in v0.1 (a half-finished agent that can run commands without working
permission prompts is worse than none). Until then, Heddle is a terminal, not an
agentic environment. Recognising the Claude Code / Codex / Gemini CLI binaries is
presentation only — not ACP compatibility.

## Licensing

### Is this legal? Can Warp be forked like this?

Yes. Warp open-sourced the client under [AGPL-3.0](LICENSE-AGPL) (the UI crates
`warpui` / `warpui_core` are [MIT](LICENSE-MIT)). AGPL exists precisely to allow
open forks; it prevents fully-proprietary relaunches, not open derivatives.
Heddle keeps the license files and upstream copyright notices (Denver
Technologies, Inc.), as the AGPL requires, and does not use the "Warp"
trademark.

### Under what license is Heddle distributed?

The same as upstream: **AGPL-3.0** for the app, **MIT** for the `warpui` /
`warpui_core` crates. There is no CLA — contributions are licensed under those
same terms.

## Contributing, help, and security

### How do I contribute?

Open a [GitHub issue](https://github.com/nikolaypaskov/heddle/issues) to discuss,
then a PR. See [CONTRIBUTING.md](CONTRIBUTING.md). Heddle does **not** use Warp's
contribution machinery — no Slack, no Oz triage, no CLA, no readiness labels.

### Where do I get help?

Open a [GitHub issue](https://github.com/nikolaypaskov/heddle/issues) or start a
discussion on the repository. There is no Slack or support desk; Heddle is a small
volunteer project.

### How do I report a security vulnerability?

Do **not** open a public issue. See [SECURITY.md](SECURITY.md) — report via a
private [GitHub Security Advisory](https://github.com/nikolaypaskov/heddle/security/advisories/new)
on this repository. Do not report Heddle issues to Warp.
