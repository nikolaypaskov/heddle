---
name: gui-integration-test-video
description: 'GUI desktop app only. Author or run Heddle Rust integration tests that capture screenshots and video through `TestStep::with_start_recording()` and `with_take_screenshot()`, including mouse and keyboard overlays. Use only for the `crates/integration` recording pipeline; use computer-use tooling for general live-app captures.'
---

# GUI Integration Test Video Recording

This is for Heddle's GUI desktop front-end only. It does not apply to the
headless TUI. Use it only to record or inspect artifacts from the Rust
integration-test harness, not to capture a running app or arbitrary UI flow.

## Relevant code

- `crates/integration/src/bin/integration.rs`
- `crates/integration/src/test/video_recording.rs`
- `crates/integration/tests/integration/ui_tests.rs`
- `crates/warpui_core/src/integration/{driver,step,video_recorder,artifacts,overlay}.rs`

## Run a recording test

Use a real display for frame capture:

```bash
WARPUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS=1 \
cargo run -p integration --bin integration -- test_video_recording
```

To auto-record one or more tests without editing test code:

```bash
WARPUI_USE_REAL_DISPLAY_IN_INTEGRATION_TESTS=1 \
WARP_INTEGRATION_TEST_VIDEO=test_foo,test_bar \
cargo nextest run --no-fail-fast --workspace test_foo
```

`WARP_INTEGRATION_TEST_VIDEO` is disabled when unset or empty; `1` and `all`
record every test, and any other value is a comma-separated test-name list.

## Author recordings

For a specific span, use explicit steps:

```rust
Builder::new()
    .with_real_display()
    .with_step(TestStep::new("Start recording").with_start_recording())
    .with_step(/* actions and events to capture */)
    .with_step(TestStep::new("Stop recording").with_stop_recording())
```

Use `TestStep::with_take_screenshot("filename.png")` to request a PNG after a
rendered step. Driver-managed recording begins at test start for matching names;
explicit start/stop steps are unnecessary in that mode unless a narrower span is
needed.

Overlays come from dispatched input while recording is active. Prefer
`with_click_on_saved_position(...)`, `with_event(...)`, `with_event_fn(...)`,
or `with_keystrokes(...)`; mouse down/drag/up creates click and drag overlays,
and key events create shortcut pills.

## Find and review artifacts

Set `WARP_INTEGRATION_TEST_ARTIFACTS_DIR` to choose the artifact root. By
default artifacts are written to:

```text
${WARP_INTEGRATION_TEST_ARTIFACTS_DIR:-$TMPDIR/warp_integration_test_artifacts}/<test_name>/<timestamp>/
```

Review `recording.mp4`, requested PNGs, and `recording.log` in the latest run.
If MP4 encoding fails, inspect the `recording_frames/` PNG fallback. Report the
exact artifact directory when handing off results.
