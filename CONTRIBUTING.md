# Contributing to Heddle

Heddle is a small, volunteer-run, privacy-oriented fork of the open-source
[Warp](https://github.com/warpdotdev/Warp) client. It is **not** affiliated with
Warp / Denver Technologies. Contributions are welcome; this guide is short on
purpose.

> Heddle does **not** use Warp's contribution machinery — there is no Slack, no
> "Oz" agent triage, no CLA, no readiness labels, and no Warp team review. If you
> came here from Warp's `CONTRIBUTING.md`, ignore all of that; the flow below is
> standard GitHub.

## What contributions fit

Heddle's purpose is a genuinely free, private, backend-independent terminal.
Good contributions:

- Bug fixes, especially in the local (non-cloud) code paths.
- Removing remaining dependence on Warp's proprietary backend, telemetry, or
  hosted services (see the de-commercialization work in
  [`docs/design/`](docs/design/)).
- Portability, packaging, and build fixes.
- Documentation improvements.

Out of scope: re-adding cloud/account/telemetry features, reimplementing Warp's
server or Warp Drive, or anything that would reintroduce a `warp.dev` dependency.
See [Non-goals](README.md#non-goals).

## How to contribute

1. **Open an issue first** for anything non-trivial, so the approach can be
   discussed before you write code.
2. **Fork and branch.** Use a short descriptive branch name (e.g.
   `fix/telemetry-guard`).
3. **Make the change.** Keep PRs focused; one logical change per PR.
4. **Build and test locally** (see below) — a PR must compile and keep the test
   suite green before review.
5. **Open a PR** describing *what* changed and *why*, and include proof of manual
   testing for behavioral changes.

## Building and testing

**First, on macOS:** the Metal Toolchain is a separate Xcode component and the build fails without
it. `xcrun -f metal` resolves even when the component is absent, so its presence proves nothing —
only a build does.

```bash
xcodebuild -downloadComponent MetalToolchain
```

Then:

```bash
# Build + run the GUI app locally
./script/run

# Headless TUI front-end. A development tool only -- it is not released on any
# platform; both macOS and Linux ship the GUI.
./script/run-tui

# Tests. Note that `cargo test` alone covers only the workspace default members,
# which do NOT include warp_core -- name the crate explicitly.
cargo test -p warp --lib
cargo test -p warp_core

# Privacy scanners (must stay green — no warp.dev / keys / telemetry in the binary).
# Build the GUI bin: that is what ships, and what the scanner defaults to.
cargo build -p warp --bin heddle \
  --features release_bundle,extern_plist,gui,nld_classifier_v3,nld_heuristic_v2
./script/heddle/verify-no-warp-endpoints
./script/heddle/verify-bundled-assets
./script/heddle/verify-warp-supply-chain
```

`./script/bootstrap` installs build dependencies. It does not fetch anything from Warp and does not
ask you to authenticate to any service; if you are reading an older copy that mentions common-skill
installation or a `gcloud` login, that has been removed.

## Style

- Follow the surrounding code: match its naming, comment density, and idioms.
- Rust: prefer imports over path qualifiers, inline format args
  (`println!("{x}")`), and exhaustive `match` over `_` wildcards.
- Conventional-commit-style messages are appreciated (`fix:`, `refactor:`,
  `docs:` …). Explain *what* and *why*.

## Licensing of contributions

By contributing you agree that your contributions are licensed under the same
terms as the code you are modifying — **AGPL-3.0** for the app, **MIT** for the
`warpui` / `warpui_core` crates — consistent with upstream. There is no separate
CLA. Preserve existing copyright notices (Denver Technologies, Inc.); the AGPL
requires it.

## Code of Conduct

This project adopts the [Contributor Covenant](https://www.contributor-covenant.org/)
(v2.1). See [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md). Report violations through
the private channel described there — **not** to Warp.

## Reporting security issues

See [`SECURITY.md`](SECURITY.md). **Do not** open public issues for security
vulnerabilities.

## Getting help

- Open a [GitHub issue](https://github.com/nikolaypaskov/heddle/issues) for bugs or
  feature requests.
- See the [README](README.md) and [FAQ](FAQ.md).
