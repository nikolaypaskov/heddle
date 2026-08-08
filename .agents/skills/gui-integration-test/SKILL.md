---
name: gui-integration-test
description: Use for Heddle GUI desktop integration tests in `crates/integration`, including authoring or running a named GUI flow through the Rust integration harness. Do not use for TUI tests or video/screenshot artifact capture.
---

# GUI Integration Tests

Use this only for the GUI integration harness under `crates/integration`.
The TUI has a separate cell-grid test path; recording artifacts belong to
`gui-integration-test-video`.

## Workflow

1. Find the closest test in `crates/integration/src/test/` and the registrations
   in `src/bin/integration.rs` and `tests/integration/ui_tests.rs`.
2. Build a `Builder` flow from deterministic `TestStep`s and assertions. Reuse
   integration helpers instead of bypassing app state or adding sleeps.
3. Run the named flow while iterating:

   ```bash
   cargo run -p integration --bin integration -- test_name
   ```

4. Set `WARPUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS=1` only when a real GUI
   window is needed for manual visual inspection. Do not add recording steps
   unless artifact capture is the requested behavior.

Keep integration coverage GUI-specific and preserve Heddle's endpoint-free
constraint; run `script/heddle/verify-no-warp-endpoints` when GUI binary code
changes require its normal validation.
