---
name: tui-testing
description: Use for unit tests of Heddle TUI elements and screens, especially render-to-lines assertions in `crates/warpui_core` or focused `warp_tui` tests. Do not use GUI integration-test infrastructure.
---

# TUI Testing

Test terminal UI behavior with deterministic cells, not GUI screenshots.

## Render-to-lines tests

- Put focused tests in a neighboring `*_tests.rs` file and include it with the
  module's existing `#[cfg(test)]` and `#[path = "..."] mod tests;` convention.
- Use `crate::elements::tui::test_support::render_to_lines` for visible rows,
  `render_to_frame` for cell style assertions, and `dispatch_presented_event`
  when input routing matters.
- Cover widths, wrapping, clipping, selection/focus, and Unicode boundaries
  that the change affects. Assert the smallest stable set of rows or cells.

Run the narrowest affected test first:

```bash
cargo test -p warpui_core --features tui text_tests
cargo test -p warp_tui test_name
```

Use the first command shape for shared cell-grid elements and the second for
TUI application behavior. Do not use the GUI integration harness for TUI tests.
