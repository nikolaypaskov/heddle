# Codex Development Harness

Codex is Heddle's primary development harness. It helps contributors plan,
implement, review, and verify changes; it does not replace repository checks.
Scripts, [`lefthook.yml`](../lefthook.yml), and
[CI](../.github/workflows/ci.yml) are the authoritative enforcement layer.

## Start safely

Install the Codex CLI, open the repository as its working directory, and trust
the project configuration only after reviewing its tracked files. Codex loads
project `.codex/` configuration only for trusted projects. Review and trust
new project hooks separately through Codex's hook controls before they run.

The tracked project surfaces are:

| Path | Purpose |
| --- | --- |
| [`AGENTS.md`](../AGENTS.md) | Repository constraints, phases, and role-routing rules. |
| [`.codex/config.toml`](../.codex/config.toml) | Project permission and subagent defaults. |
| [`.codex/agents/`](../.codex/agents/) | Specialist role definitions. |
| [`.agents/skills/`](../.agents/skills/) | Shared Heddle workflows, including GUI and TUI guidance. |
| [`.codex/hooks.json`](../.codex/hooks.json) | Session reminder and narrow apply-patch Rust formatting hook. |

The hook formatter is deliberately small: it formats only existing in-repo Rust
files named by apply-patch markers. It is not a substitute for `rustfmt`,
`lefthook`, tests, privacy scanners, or CI. The Windows hook command is present
but has not been tested on Windows.

## Everyday workflow

Follow the phase and role rules in [`AGENTS.md`](../AGENTS.md):

1. Use KICKOFF to explore a new direction, then PLAN for non-trivial design.
2. Use DEVELOP for a focused patch; dispatch the matching scoped role only when
   it materially improves the work.
3. Use FINISH to run the appropriate checks and prepare the handoff.

The named roles separate ideation, exploration, implementation, planning,
review, security, debugging, and testing. Keep write work scoped to the main
thread or the implementer role; use read-only roles for investigation and
adversarial review. Prefer GUI-specific skills for the GUI and `tui-*` skills
for the headless TUI rather than mixing their verification methods.

## Verification and reporting

Run the narrowest relevant check after a meaningful change, then expand to the
project checks appropriate to the risk. Before a PR, run the required
formatting, linting, unit suite, and gate from [`AGENTS.md`](../AGENTS.md):

```bash
./script/format
cargo clippy --workspace --all-targets --all-features --tests -- -D warnings
cargo nextest run --locked --no-fail-fast --workspace \
  --exclude command-signatures-v2 --exclude integration --exclude remote_server
lefthook run gate
```

Use the exact project test commands rather than assuming `cargo test` covers
every workspace surface. Preserve the GUI endpoint and privacy checks whenever
their affected surfaces change. In every handoff, report changed files, exact
verification commands and results, checks intentionally skipped, and residual
risks or manual follow-up. Never claim visual, platform, or network verification
that did not run.

## Privacy, network, and secrets

Heddle must remain free of Warp-hosted services, `warp.dev` calls, telemetry,
hosted sign-in, and cloud sync. The default Codex project profile keeps network
access disabled and denies configured secret-bearing paths. Do not bypass those
boundaries, place credentials in tracked files or logs, or use an agent to send
repository data to an untrusted destination. A local harness setting is a
convenience boundary, not a replacement for code review or deterministic scans.

## Migration status

Codex is the documented primary harness, but it is optional for contributors
and deterministic checks remain tool-neutral. The following are temporary
compatibility or tooling artifacts and **must not be deleted or renamed in this
stage**:

- [`.claude/settings.json`](../.claude/settings.json)
- [`.claudeconf/`](../.claudeconf/)
- [`.warp/workflows/`](../.warp/workflows/)

In particular, `.claudeconf` still backs deterministic manifests and rules
referenced by scripts and CI. Retire a compatibility surface only in a separate,
reviewable change after its behavior is mapped or intentionally dropped, every
reference is migrated, and parity is rechecked. Do not infer authorization to
delete or rename these artifacts from the Codex cutover alone.
