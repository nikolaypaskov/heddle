---
name: tui-verify-change
description: Use after a Heddle TUI change to run focused tests and verify behavior in a real terminal with `script/run-tui`. Covers honest reporting when visual verification cannot be performed; do not use for GUI validation.
---

# Verify a TUI Change

1. Run the narrowest unit test that covers the changed TUI behavior. Use
   `tui-testing` to add coverage when a stable rendered-line or event assertion
   is possible.
2. Run the console front-end in a real terminal:

   ```bash
   ./script/run-tui
   ```

3. Exercise the changed flow at relevant terminal dimensions and inspect focus,
   keyboard input, wrapping, clipping, colors, and redraw behavior.
4. Report the exact test command, whether real-terminal verification ran, what
   was observed, and any limitation. If a real terminal or interactive display
   is unavailable, say visual verification was not performed; do not infer it
   from a successful build or GUI test.
