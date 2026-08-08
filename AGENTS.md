# AGENTS.md

This file provides guidance when working with code in this repository.

> **This is Heddle**, a privacy-oriented, de-commercialized fork of the
> open-source [Warp](https://github.com/warpdotdev/Warp) client. It has **no
> server, no cloud backend, no hosted authentication, and no telemetry** — the
> dependence on Warp's proprietary backend has been removed. When working here,
> do not reintroduce network calls to `warp.dev`, telemetry, account/sign-in
> flows, or Warp Drive cloud sync. Many types still carry `Warp`/`Oz`/cloud names
> from upstream; that code is being removed incrementally (see
> [`docs/design/`](docs/design/)). The **GUI** binary — the `heddle` bin, which is
> what ships as `Heddle.app` and as the Linux AppImage — is verified endpoint-free
> by `script/heddle/verify-no-warp-endpoints` on every change. (This line used to
> name `warp_tui`, which nobody downloads.)

## Development Commands

### Build and Run
- `cargo run` / `./script/run` - Build and run the GUI desktop app locally
- `./script/run-tui` - Build and run the headless TUI front-end (`crates/warp_tui`)
- `cargo bundle` - Bundle the main (GUI) app (produces `Heddle.app`)

There is no server to connect to — Heddle removed the backend, so the upstream
`WITH_LOCAL_SERVER` / `SERVER_ROOT_URL` workflow does not apply.

### Testing
- `cargo nextest run --locked --no-fail-fast --workspace --exclude command-signatures-v2 --exclude integration --exclude remote_server` - Run tests with nextest. Byte-identical to the `unit` job in both `lefthook.yml` and `.github/workflows/ci.yml`; if you change it, change all three. The two extra exclusions are crates that cannot build (see the `unit` job in `lefthook.yml` for the per-crate reason) — without them this command fails to compile.
- `cargo nextest run -p warp_completer --features v2` - Run completer tests with v2 features
- `cargo test --doc` - Run doc tests
- `cargo test` - Run standard tests for individual packages

### Linting and Formatting
- `./script/presubmit` - Run the comprehensive local presubmit checks. Rust formatting is advisory while inherited formatter drift remains.
- `./script/format` - Format code
- `cargo clippy --locked --workspace --exclude command-signatures-v2 --exclude integration --exclude remote_server` - Run the workspace Clippy contract. It does not deny warnings while inherited warning debt remains.
- `./script/run-clang-format.py -r --extensions 'c,h,cpp,m' ./crates/warpui/src/ ./app/src/` - Format C/C++/Obj-C code
- `find . -name "*.wgsl" -exec wgslfmt --check {} +` - Check WGSL shader formatting

### Platform Setup
- `./script/bootstrap` - Platform-specific setup.
- `./script/install_cargo_build_deps` - Install Cargo build dependencies
- `./script/install_cargo_test_deps` - Install Cargo test dependencies

Bootstrap installs build dependencies and nothing else. It used to also fetch agent skills, by
`curl`ing a script from `warpdotdev/common-skills` at unpinned `main` and running it under bash --
on every `./script/bootstrap` **and** every `./script/run`. That made routine development execute
code from a Warp-controlled repository that could change at any time, in a fork whose premise is not
depending on Warp. The mechanism, its `skills-lock.json` (which pinned 25 skills, none of them
present in this checkout), and the flags that drove it are all gone. `--install-common-skills`,
`--skip-common-skills` and `--skip-gcloud-auth` are still accepted so old invocations do not fail,
but they print a note and do nothing.

Bootstrap also no longer prompts for `gcloud auth login`. The only thing that needed a Google Cloud
identity was Warp's `warp-ssh-integration-testing` project, which no fork contributor can reach.

## Architecture Overview

This is a Rust-based terminal emulator with a custom UI framework called **WarpUI**. It has **two front-ends** that share a common core.

### Front-ends: GUI and TUI

Warp has two front-ends that share the `warp_core`/`warpui` Entity/model core (App/Entity/`AppContext`, actions, `Appearance`, `FeatureFlag`, telemetry, logging) but differ in UI framework, rendering, input, and verification:
- **GUI desktop app** — the `app/` crate on the WarpUI pixel/GPU framework (`warpui`, `crates/warpui_core`): `Element`/`View` layout, GPU/WGSL rendering, mouse input, `.app` bundles. Run with `cargo run` / `./script/run`; verify visually with `computer_use` or the real-display integration framework (`crates/integration`).
- **Headless TUI** — the `crates/warp_tui` crate: a console app (run with `./script/run-tui`; no `.app`/GPU) rendered with a parallel cell-grid element library at `crates/warpui_core/src/elements/tui` (the `TuiElement` trait), behind the `tui` cargo feature. Verify by running it in a real terminal and observing output; test with render-to-lines unit tests.

### Key Components

**Shared UI core** (`crates/warpui`, `crates/warpui_core`) — used by **both** front-ends:
- Entity-Component-Handle pattern: a global `App` object owns all views/models (entities); views hold `ViewHandle<T>` references to other views; `AppContext` provides temporary access to handles during render/events.
- Actions system for event handling.
- `crates/warpui_core` also hosts the TUI cell-grid element library under `src/elements/tui` (behind the `tui` feature).

**GUI rendering** (WarpUI GUI elements — GUI-specific):
- `Element`s describe visual layout (Flutter-inspired), rendered on the GPU (WGSL).
- Mouse input uses `MouseStateHandle`: create it once during construction and reference/clone it wherever mouse input is tracked. An inline `MouseStateHandle::default()` while rendering means no mouse interactions work. (The TUI's hover/click elements — `TuiHoverable`, `tui_collapsible` — also build on `MouseStateHandle`, so the same ownership rule applies there.)

**TUI rendering** (`crates/warp_tui` + `crates/warpui_core/src/elements/tui` — TUI-specific):
- Headless console front-end. The `TuiElement` trait lays out and paints into a cell-grid `TuiBuffer`; crossterm input is converted to `TuiEvent`. No GPU/WGSL, pixel geometry, or `.app` bundle.

**Main app / shared surfaces** (`app/`) — the GUI desktop app plus feature surfaces the TUI reuses:
- Terminal emulation and shell management (`terminal/`)
- AI integration including Agent Mode (`ai/`)
- Cloud synchronization and Drive features (`drive/`)
- Authentication and user management (`auth/`)
- Settings and preferences (`settings/`)
- Workspace and session management (`workspace/`)

**Core Libraries**:
- `crates/warp_core/` - Core utilities and platform abstractions (shared)
- `crates/warp_tui/` - Headless TUI front-end
- `crates/editor/` - Text editing functionality
- `crates/warpui/` and `crates/warpui_core/` - Custom UI framework (shared core plus the GUI and TUI element libraries)
- `crates/ipc/` - Inter-process communication
- `crates/graphql/` - GraphQL client and schema

### Key Architectural Patterns

1. **Entity-Handle System**: Views reference other views via handles, not direct ownership
2. **Modular Structure**: Workspace contains multiple workspace configurations, each with terminals, notebooks, etc.
3. **Cross-Platform**: Native implementations for macOS, Windows, Linux, plus WASM target
4. **AI Integration**: Built-in AI assistant with context awareness and codebase indexing
5. **Cloud Sync**: Objects can be synchronized across devices via Warp Drive

### Development Guidelines

**Workspace Structure**:
- This is a Cargo workspace with 60+ member crates
- Main binary is in `app/`, UI framework in `crates/warpui/`
- Platform-specific code is conditionally compiled
- Integration tests are in `crates/integration/`

**Coding Style Preferences**:
- Avoid unnecessary type annotations, especially in closure params.
- Avoid using too many Rust path qualifiers and use imports for concision. Place import statements at the top of the file as per convention.
  An exception to this is inside cfg-guarded code branches. In those cases, you can either embed the import into the relevant scope or just use an absolute path for one-offs.
- If a function takes a context parameter (`AppContext`, `ViewContext`, or `ModelContext`), it should be named `ctx` and go last. The one exception is for
  functions that take a closure parameter, in which case the closure should be last.
- Always remove unused parameters completely rather than prefixing them with `_`. Update the function signature and all call sites accordingly.
- Prefer inline format arguments in macros like `println!`, `eprintln!`, and `format!` (for example, `eprintln!("{message}")` instead of `eprintln!("{}", message)`) to satisfy Clippy's `uninlined_format_args` lint.
- Do not pass `Itertools::format` results directly to logging macros (`log::*`, `safe_*`, etc.). `Itertools::format` produces a single-use formatter, while logging implementations may format a message more than once. Use a reusable `String` such as `iter.join(", ")` for logging arguments instead. Direct use in `format!` or `write!` is fine.
- Do not remove existing comments when making unrelated changes. Only remove or modify a comment if the logic it describes has changed.
- When adding a toggleable setting, also add the matching Command Palette enable/disable entry and any required context flags so the setting is discoverable outside Settings.

**Terminal Model Locking**:
- Be extremely careful when calling `model.lock()` on the terminal model (`TerminalModel`). Acquiring multiple locks on the same model from different call sites can cause a deadlock, resulting in a UI freeze (beach ball on macOS).
- Before adding a new `model.lock()` call, verify that no caller in the current call stack already holds the lock.
- Prefer passing already-locked model references down the call stack rather than acquiring new locks.
- If you must lock the model, keep the lock scope as short as possible and avoid calling other functions that might also attempt to lock.

**Testing**:
- Use `cargo nextest` for parallel test execution
- Integration tests use the custom framework in `crates/integration/` — this is **GUI-only**. TUI elements/screens are covered by render-to-lines unit tests instead.
- Tests should be run via presubmit script before submitting
- Unit tests should be placed in separate files using the naming convention `${filename}_tests.rs` or `mod_test.rs`
- Test files should be included at the end of their corresponding module with:
  ```rust
  #[cfg(test)]
  #[path = "filename_tests.rs"]  // or "mod_test.rs"
  mod tests;
  ```

**Pull Request Workflow**:
- **ALWAYS** run `lefthook run gate` before opening a PR or pushing updates to an existing PR branch. The exact workspace Clippy and unit commands above must pass completely.
- Repository-wide Rust formatting is advisory while inherited formatter drift remains. Format touched Rust files, inspect the resulting scope, and do not mix unrelated formatting changes into a feature patch.
- If a required gate job fails, fix it before proceeding with the PR. Report advisory failures honestly rather than claiming a clean run.
- Do not create public pull requests or public issues that disclose a non-public security vulnerability. Refer users to `SECURITY.md` for the proper disclosure methods instead.
- This applies to:
  - Opening new pull requests
  - Pushing new commits to existing PR branches
  - Any branch updates that will be reviewed
 - When opening PRs, use the PR template at `.github/pull_request_template.md`
 - Add changelog entries when appropriate using the format at the bottom of the PR template. Use the following prefixes (without the `{{}}` brackets):
   - `CHANGELOG-NEW-FEATURE:` for new, relatively sizable features (use sparingly - these may get marketing/docs)
   - `CHANGELOG-IMPROVEMENT:` for new functionality of existing features
   - `CHANGELOG-BUG-FIX:` for fixes related to known bugs or regressions
   - `CHANGELOG-IMAGE:` for GCP-hosted image URLs
   - Leave changelog lines blank or remove them if no changelog entry is needed

**Database**:
- Uses Diesel ORM with SQLite
- Migrations in `crates/persistence/migrations/`
- Schema defined in `crates/persistence/src/schema.rs`

**GraphQL**:
- Schema and client code generation from `crates/warp_graphql_schema/api/schema.graphql`
- TypeScript types generated for frontend integration

### Feature Flags

Warp uses compile-time feature flags with a small runtime plumbing layer.

How to add a feature flag:
- Add a new variant to `warp_core/src/features.rs` in the `FeatureFlag` enum
- (Optional) Enable it by default for dogfood builds by listing it in `DOGFOOD_FLAGS`
- Gate code paths with `FeatureFlag::YourFlag.is_enabled()`
- For preview or release rollout, add to `PREVIEW_FLAGS` or `RELEASE_FLAGS` respectively (as appropriate)

Best practices:
- **Prefer runtime checks over cfg directives**: Prefer `FeatureFlag::YourFlag.is_enabled()` over `#[cfg(...)]` compile-time directives so flags can be toggled without recompilation and are easier to clean up later. Use `#[cfg(...)]` only when the code cannot compile without them (for example, platform-specific code or dependencies that do not exist when the feature is disabled).
- Keep flags high-level and product-focused rather than per-call-site
- Remove the flag and dead branches after launch has stabilized
- For UI sections that expose a new feature, hide the UI behind the same flag

Example:
```rust
#[derive(Sequence)]
pub enum FeatureFlag {
    YourNewFeature,
}

// Default-on for dogfood builds
pub const DOGFOOD_FLAGS: &[FeatureFlag] = &[
    FeatureFlag::YourNewFeature,
];

// Use in code
if FeatureFlag::YourNewFeature.is_enabled() {
    // gated behavior
}
```

### Exhaustive Matching

When adding/editing match statements, avoid using the wildcard _ when at all possible. Exhaustive matching is helpful for ensuring that all variants are handled, especially when adding new variants to enums in the future.
