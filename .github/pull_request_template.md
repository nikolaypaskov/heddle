## Description

<!-- What does this PR change, and why? -->

## Linked issue

<!-- Link the GitHub issue this PR addresses, if any. -->

## Testing

<!--
How did you test this change? Include screenshots or a short video for
user-visible / UI changes.
-->

- [ ] Builds and the test suite passes (`cargo test -p warp --lib`)
- [ ] Manually tested locally with `./script/run` (see [AGENTS.md](../AGENTS.md))
- [ ] For anything touching networking/privacy: the privacy scanners still pass
      (`./script/heddle/verify-no-warp-endpoints`, `./script/heddle/verify-bundled-assets`)
- [ ] This change does **not** reintroduce a dependency on Warp's backend,
      telemetry, hosted auth, or Warp Drive cloud sync (see
      [Non-goals](../README.md#non-goals))
