---
name: gui-ui-guidelines
description: Use for Heddle GUI desktop UI changes in `app/`, `crates/warpui`, or GUI WarpUI elements. Covers pixel/GPU layout, actions, input, and GUI-specific validation; do not use for the headless TUI.
---

# GUI UI Guidelines

Work in the GUI desktop surface: `app/` on WarpUI's pixel/GPU element system.
Do not apply TUI cell-grid patterns or TUI-only verification here.

## Implementation

- Model layout with WarpUI `Element`s and handle interactions through actions and
  the entity/view context conventions already used nearby.
- Create `MouseStateHandle` once during construction and retain or clone it for
  every render path that needs mouse state. An inline default during rendering
  loses interaction state.
- Keep terminal model locks short; never add a lock before confirming callers
  do not already hold one.
- Add discoverable Command Palette actions and context flags with toggleable
  settings.

## Verification

Use focused GUI unit tests first. For end-to-end GUI behavior, use the
`gui-integration-test` skill; use `gui-integration-test-video` only when the
test must capture screenshot or video artifacts.
