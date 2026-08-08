---
name: tui-ui-guidelines
description: Use for Heddle headless TUI changes in `crates/warp_tui` or `crates/warpui_core/src/elements/tui`. Covers cell-grid layout, terminal input, and TUI-specific behavior; do not use for GUI pixel/GPU UI.
---

# TUI UI Guidelines

Build the headless terminal front-end in `crates/warp_tui` with the `TuiElement`
cell-grid library in `crates/warpui_core/src/elements/tui`. Do not use GUI
`Element`, GPU, `.app`, or pixel-geometry assumptions.

## Implementation

- Express layout and paint through `TuiElement`, `TuiBuffer`, and TUI geometry;
  make narrow widths, wrapping, clipping, and Unicode width deliberate.
- Route terminal input through `TuiEvent` and preserve focus/event propagation.
- Create `MouseStateHandle` during construction when hover or click state is
  needed; retain it across renders instead of constructing a default inline.
- Keep TUI-only code behind the `tui` feature and avoid leaking it into GUI
  surfaces.

Use `tui-testing` for rendered-line coverage and `tui-verify-change` for the
focused test plus real-terminal verification.
