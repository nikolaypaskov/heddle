# Handoff

**As of 2026-07-26.** Written to be read by someone — or some agent — starting in a fresh checkout
with no memory of how any of this came to be. It records the things that are *not* recoverable by
reading the code: the standing constraints, the procedures, and the mistakes that cost the most.

For what the product is, read [`../README.md`](../README.md). For how the de-commercialization works,
read [`HOW-IT-WORKS.md`](HOW-IT-WORKS.md). For the phase-by-phase history, read
[`design/HEDDLE_STATUS.md`](design/HEDDLE_STATUS.md), which is a historical record and is
deliberately not kept current.

---

## 1. Standing constraints — do not violate these

**The hard one.** Never break, in the course of removing commercial surfaces:

- local Agent Mode with the user's own API keys (BYOK)
- the local terminal itself
- the conversation-list panel
- local model / harness selection
- session sharing

Removal work has repeatedly come close to breaking these by accident. See §4.

**Where "local model / harness selection" actually holds.** It means the GUI. Stated plainly so
nobody reads the bullet above as a claim about both frontends:

- **GUI — works.** The harness catalog is derived client-side
  (`ai/harness_availability.rs::local_harness_catalog`), CLI presence is probed live per render
  (`local_harness_setup_state`), and Claude's model choice reaches the child as `ANTHROPIC_MODEL`.
- **TUI — local harness selection does not exist.** `page_sequence` in
  `warp_tui/src/orchestration_block.rs` gives Local only `[Location, Model]`, and
  `normalize_tui_local_harness` (`orchestration_block/configuration.rs`) rewrites any non-Oz local
  harness back to `oz` and clears the model id. This is **upstream design, not a Heddle
  regression** — it arrived whole in upstream `4b33a6a78` — so the constraint was never true there
  to break. Local *model* selection does work in the TUI (Oz models).
- The consequence worth knowing: the TUI's Harness page is built and wired
  (`configuration.rs` handles `ConfigPage::Harness`), but `page_sequence` only offers it for
  Remote — and Remote needs a backend this fork does not talk to. So in the TUI that page is now
  unreachable in practice. Making local harness selection work there is a small change on paper
  (add `ConfigPage::Harness` to the Local sequence, drop the normaliser) but it is a **product
  decision plus a TUI test surface**, not a bug fix. Nobody has taken it.

**Non-goals.** This does not unlock Warp's paid features, does not reimplement Warp Drive, and does
not fork Warp's server. Entitlements were enforced server-side; removing paywall UI removes nags, not
restrictions. Anyone framing this as "free Pro" has misunderstood the project.

**Trademark.** AGPL grants the code, not the name. Warp's marks must not ship. But `WARP_*`
environment variables, `.warp/` project directories, keybinding action ids and wire-protocol markers
are *persistence and compatibility contracts* — renaming them silently breaks users' shells and
keybindings. The distinction is: user-visible copy gets renamed, contracts do not.

## 2. Where things stand

Released: v0.1.0, v0.2.0, v0.3.0 — each superseded by the next, with the reason stated on the
superseded release rather than quietly replaced.

In flight at the time of writing: **v0.3.1**, carrying

- login-item registration made opt-in (it previously added itself to startup items without asking)
- state moved out of Warp's app-group container, with a migration for anyone who already had data
  there
- the persisted telemetry queue discarded rather than accumulating forever
- the updater's team-ID check corrected (it trusted *Warp's* signing team, so it would have accepted
  a Warp-signed update and rejected a Heddle-signed one)
- `email_address` pinned to a revision — it was the last dependency tracking a branch on a
  Warp-owned repository
- Warp's logomark glyph removed from the renderers (see §4)
- macOS permissions reduced from seven entitlements to one (see §5)
- the bundle version fixed: every release before this one reported itself as `0.1.0`

Deliberately **not** done, in rough priority order:

1. **Delete `app/src/drive/` and `app/src/billing/`.** Warp Drive is 22,807 lines with 179 inbound
   references; the billing modal is only ever triggered from it, so they are one unit. This is the
   largest remaining block of dead commercial code. It is a dedicated slice, not a tidy-up.
2. The remaining ambient cloud-agent slices — see
   [`design/plans/2026-07-24-ambient-runtime-removal.md`](design/plans/2026-07-24-ambient-runtime-removal.md).
3. `AuthManager` itself: 164 files, `current_user` at ~197 sites.
4. The DMG recipe is still Warp-branded, so releases ship a zipped `.app` instead.
5. `WarpDockTilePlugin` still ships inside the bundle; `warpctrl_command_name` is still
   `warpctrl-oss`.
6. The ACP agent bridge — designed, deliberately unimplemented. A half-finished bridge that streams
   tool calls without working permission prompts can run commands the user never agreed to.

## 3. Procedures

### Cutting a release

```bash
./script/bundle -c oss --release-tag vX.Y.Z                      # macOS -> Heddle.app
```

The **TUI is not released on any platform.** `crates/warp_tui` still builds and is still tested,
but no release artefact contains it: Linux ships the same GUI as macOS, built by
`.github/workflows/heddle-release.yml` on a `heddle-v*` tag. Nothing has to be run by hand there —
the workflow does `./script/bundle -c oss --packages appimage,deb,rpm --release-tag vX.Y.Z` itself
and publishes **three separate assets**, each with its own `.sha256`:

| Asset | Notes |
|---|---|
| `Heddle-x86_64.AppImage` | Raw, not wrapped in a tarball. `chmod +x` and run. |
| `heddle_X.Y.Z_amd64.deb` | `Package: heddle`, installs under `/opt/heddle/heddle`. |
| `heddle-X.Y.Z-1.x86_64.rpm` | `Name: heddle`, same layout. |

Things about that shape that are load-bearing:

- **No tarball wrapper.** It used to be the only Linux asset and carried the AppImage plus ~2 MB
  of duplicated `bundled/` skills and a `settings_schema.json` that the app never read — the
  AppImage already carries its own copies at `opt/heddle/heddle/resources/`.
- **The licence texts come from `script/prepare_bundled_resources`,** which stages `LICENSE-AGPL`
  and `LICENSE-MIT` into every bundle's resources directory. Before that, the tarball was the only
  thing carrying them, and the AppImage inside it had neither. If you ever change how resources
  are staged, the release job's verify step will catch it: it refuses to publish an artefact that
  does not contain both, in all three formats.
- **No apt/yum/zypper repository, and no signing key.** The other channels' packaging templates
  configure Warp's repository in their post-install; the oss channel uses
  `resources/linux/debian/heddle/` and `resources/linux/rpm/heddle/` instead, which do neither.
  This project hosts no package repository, and a source list pointing at one that does not exist
  fails on every `apt update`.
- **ALSA is declared, not bundled.** `libasound.so.2` is the only non-universal library the binary
  links; it comes from `crates/voice_input` → `cpal`, which `gui = ["voice_input"]` makes a hard
  component of the GUI. The `.deb` declares `libasound2t64 | libasound2` and the `.rpm` declares
  the `libasound.so.2()(64bit)` SONAME. Both bundlers fail the build if that declaration is
  missing, because a package that declares nothing installs cleanly and then dies at startup. The
  AppImage cannot declare anything, so its requirement is documented in README.md,
  docs/index.html and docs/RELEASE_NOTES.md instead.
- **`/usr/bin/heddle` is SHIPPED IN BOTH PACKAGES, not created by a maintainer script.** A
  postinst that `rm -f`s a path and then `ln -s`es it produces a file the package manager does not
  own: no conflict check on install, nothing under `dpkg -L`, and a matching `rm -f` in postrm
  that deletes whatever is at that path on removal — including another package's file. Both
  formats now put the link in the payload (`script/linux/bundle_deb`, and `%files` in
  `resources/linux/rpm/heddle/heddle.spec.template`). If you ever move link creation back into a
  maintainer script, you reintroduce that.

To reproduce the Linux artefacts locally, on Linux:

```bash
./script/linux/install_linuxdeploy                                       # pinned; adds ~/.local/bin
sudo apt-get install -y fakeroot rpm                                     # for the native packages
./script/bundle -c oss --packages appimage,deb,rpm --release-tag vX.Y.Z
# -> target/release-lto/bundle/linux/{Heddle-<arch>.AppImage,heddle_*.deb,heddle-*.rpm}
```

### Checking the rpm's dependencies actually resolve

The release job resolves the **`.deb`**'s dependencies for real, with `apt-get install -s` against
the runner's own apt index, so a name that does not exist fails the release. It does **not** do
the equivalent for the `.rpm`: that needs a Fedora or openSUSE container and their metadata
mirrors, and letting a third party's outage fail a two-hour release build is a worse trade than
checking this by hand when the `Requires:` line changes. What the job does instead is compare the
declaration against the SONAMEs the shipped binary actually links (`readelf -d`), which is
network-free and cannot be satisfied by a plausible-looking wrong name.

So: **when you change `Requires:` in `resources/linux/rpm/heddle/heddle.spec.template`, run this
once by hand** against the built package.

```bash
docker run --rm -v "$PWD:/w" fedora:41 \
  dnf -q -y install --assumeno /w/heddle-X.Y.Z-1.x86_64.rpm     # expect: alsa-lib in the plan
docker run --rm -v "$PWD:/w" opensuse/leap:15.6 \
  zypper -n se --provides --match-exact 'libasound.so.2()(64bit)'   # expect: libasound2
```

Those two lines are why the spec uses the SONAME rather than a package name: Fedora calls that
package `alsa-lib` and openSUSE calls it `libasound2`, so no single literal name resolves on both.

The `--release-tag` is **not optional**. Without it `option_env!("GIT_RELEASE_TAG")` is `None`, the
app cannot report its own version, and `script/update_plist` leaves `CFBundleShortVersionString` at
Cargo's `0.1.0`. Every release before v0.3.1 shipped claiming to be 0.1.0 for exactly this reason.

Then sign inside-out (never `--deep`, which is deprecated and mis-signs nested code):

```bash
codesign --force --options runtime --timestamp \
  --entitlements script/Entitlements.plist \
  --sign "Developer ID Application: Nikolay Paskov (4STAAHTNCN)" <nested binaries first, app last>

xcrun notarytool submit <zip> --keychain-profile heddle --wait
xcrun stapler staple Heddle.app        # bundles only
ditto -c -k --keepParent Heddle.app Heddle-aarch64-apple-darwin.app.zip
```

Facts that cost time to learn:

- **Developer ID Application** is the only identity that works. Apple Development and Apple
  Distribution are for other things and will notarize-fail.
- Tickets **staple to bundles, disk images and packages only** — never to a bare executable. The CLI
  is notarized but unstapled, so local `spctl` reports "Unnotarized Developer ID" even though
  Apple's record says otherwise. That is expected, not a failure.
- Package with `ditto`, and tell users to unpack with `ditto` or Archive Utility. Some third-party
  unzip tools strip the signature and macOS then refuses to launch the app.
- Publish with `--repo <owner>/<name>` **explicitly**. See §6.

Then publish the update manifest **on the same release as the `.app.zip`, and only after it**:

```bash
# 1. the payload first
gh release upload <tag> Heddle-aarch64-apple-darwin.app.zip --repo <owner>/<name>
# 2. only then the thing that advertises it
./script/heddle/generate-release-manifest vX.Y.Z channel_versions.json
gh release upload <tag> channel_versions.json --repo <owner>/<name> --clobber
```

The order is not cosmetic. Clients read `releases/latest/download/`, so a release carrying the
manifest but not the app tells every running Heddle that a new version exists and then 404s
when it goes to fetch it — and it keeps doing that until the next release. The release
workflow enforces the same rule for its own runs: it skips the manifest when no
`Heddle-*.app.zip` is present, because it builds only the Linux artefacts. That check stays
macOS-specific because the updater is — it fetches `Heddle-<arch>-apple-darwin.app.zip`, so a
Linux artefact cannot satisfy it however current it is.

The version passed here is the **app** version — the same `vX.Y.Z` given to `--release-tag`,
which becomes `CFBundleShortVersionString`. It is NOT the git tag: releases have been tagged
`heddle-v0.3.1-macos-arm64` while the app reports `v0.3.1`. The script refuses anything that
is not `v?MAJOR.MINOR.PATCH`, because `HeddleVersion::parse` refuses it too and an
unparseable version means "do not update" — a malformed manifest would silently disable
updates for everyone who installed that release, with no error anywhere.

The client reads `releases/latest/download/channel_versions.json`, so the manifest must be on
the release GitHub considers *latest*. It downloads
`releases/latest/download/Heddle-<arch>-apple-darwin.app.zip` from that same release, then
reads the version out of the downloaded bundle and refuses to install anything that is not
strictly newer than what is running — so the manifest tells the client to look, and the
payload itself is what authorises the install.

### Gates

```bash
./script/heddle/verify-no-warp-endpoints      # no Warp addresses in the built binary
./script/heddle/verify-bundled-assets         # bundled binary assets match a reviewed manifest
./script/heddle/verify-warp-supply-chain      # no Warp-controlled code path; deps pinned by rev
./script/heddle/gui-surface-gate              # commercial UI + Warp strings may only shrink
./script/heddle/wasm-diagnostic-gate          # wasm32 diagnostics may only shrink
```

**Run the self-test, not just the gate.** The reason is §4. But "each has a `-selftest`
companion" — which this section used to claim — is false, and enumerating it is the point:

"Has a self-test" and "the gate runs it" are different questions, so they get different
columns — conflating them is how this table was wrong on its first attempt:

| gate | canary | canary runnable locally? | gate in `lefthook run gate`? | canary in `lefthook run gate`? |
| --- | --- | --- | --- | --- |
| `verify-warp-supply-chain` | `-selftest` script | yes | yes | **yes** |
| `gui-surface-gate` | `-selftest` script | yes | yes | **no** — CI only |
| `wasm-diagnostic-gate` | inline CI YAML, GNU-`sed` only | **no** | yes | no |
| `verify-no-warp-endpoints` | inline CI YAML (plants `oz.warp.dev`) | **no** | **no** | no |
| `verify-bundled-assets` | `-selftest` script | yes | yes | yes |

So: three of the five have a runnable self-test, and **two canaries fire in the local
gate**. `verify-bundled-assets` was the one that had never been shown able to fail anywhere;
its self-test plants a changed asset, an added one and a removed one, across several entries
spanning the manifest — a single-asset check would pass a gate that compared only the first
hash and the total count. `gui-surface-gate-selftest` exists but is still CI-only.
`verify-no-warp-endpoints` is the one gate not in the local run at all — it scans a *built*
artifact, so it needs a full GUI codegen+link first (CI allows it 90 minutes). Run it by hand
before a PR that touches endpoints, config or bundled assets; a green `lefthook run gate` says
nothing about it. Open items: write the two missing `-selftest` scripts, call
`gui-surface-gate-selftest` from the gate, and add a `check-project-gates` asserting every
`projectGates` entry in `.claudeconf/manifest.json` has a job.

`wasm-diagnostic-gate` needs `rustup target add wasm32-unknown-unknown`, and on macOS a
clang with the WebAssembly backend (Apple's has none — the script finds Homebrew's LLVM by
itself). It exits 2 with instructions rather than passing if it cannot really check.

To record a deliberate addition to a baseline: `--update --allow-additions`, and say why in the
commit. The gate refuses additions otherwise, on purpose.

### Testing

```bash
# What the gate and CI run — byte-identical to both. ~9,180 tests, ~100s warm.
cargo nextest run --locked --no-fail-fast --workspace \
  --exclude command-signatures-v2 --exclude integration --exclude remote_server

cargo test -p warp --lib      # ~5,593 pass, 13 known isolation failures
cargo test -p warp_core       # 52
cargo test -p warp_tui        # 524
cargo test -p onboarding      # 12
```

**`cargo test` alone covers only workspace default members, and `warp_core` is not one of them.**
Four failures hid behind that for a long time. Always name the crate.

**The gate used to run only those four packages.** 52 crates under `crates/` contain tests and
49 were outside that set — they compiled as dependencies, so only their *tests* were skipped.
Widening to the whole workspace took the suite from 6,293 tests to 9,167 (+47s warm) and every
newly-included test passed. Three crates are excluded **by name**, each for a stated reason in
`lefthook.yml`:

| crate | why | cost |
| --- | --- | --- |
| `command-signatures-v2` | `build.rs` needs `yarn`; `js/build/` is gitignored | no tests |
| `integration` | enables `warp/integration_tests`, which does not compile (7 errors in `app/src/integration_testing/agent_mode/`) — including it breaks `warp` itself | **~310 tests, incl. hard-constraint UI** |
| `remote_server` | `setup_tests.rs` does not compile — `install_script()` is `Option<String>` and is `None` on the OSS channel (`:288 :289 :343 :496`) | ~99 tests |

Deleting an exclusion is the fix. `remote_server` needs a decision about what its tests should
assert when there *is* no install script, which is why it was not patched to force the gate green.

**`integration` is the expensive one, and it is easy to under-read.** Its bin sets
`test = false`, and grepping the crate for `#[test]` returns **0** — so it looks empty. It is
not: `tests/integration.rs` is a Cargo integration-test target pulling in `ui_tests.rs` (248)
and `shell_integration_tests.rs` (62), whose `integration_tests!` macro
(`tests/common/mod.rs:102`) *generates* the `#[test]` fns. Among the 310 are
`test_inline_model_selector_restores_prompt_on_*` (`ui_tests.rs:143-145`) and
`test_agent_mode_pane_minimum_size` (`:323`) — local model selection and Agent Mode UI, two of
the four capabilities this fork must never break. **Nothing is testing them.** That is
pre-existing (the old four-package command skipped this crate too), not a regression from
widening — but do not let "whole workspace" imply otherwise. Note also that these drive the
real GUI binary as a subprocess and are `#[ignore]`d off macOS unless `run_on_linux` is set,
so making the crate compile is necessary but not sufficient to gate on them.

**Exclude a crate only when it cannot BUILD, never because its tests are inconvenient.**
`http_client` was excluded for one round on the strength of two compile errors in its
positive-origin test — which would also have dropped the independent
`third_party_origin_does_not_match` beside it, the only thing covering `is_warp_server_origin`,
the predicate that scopes IAP bearer-token attachment. A whole-crate exclusion silently drops
every invariant in the crate, not just the broken one. The errors were `Option`-migration
fallout and are fixed; the crate runs. If a crate ever genuinely must be skipped, skip at the
*test* level so its neighbours keep running.

**A test module can be compiled away and nobody notices.** `crates/warp_core/src/channel/state.rs`
gated its tests on `#[cfg(all(test, not(feature = "test-util")))]`. Building `warp_core` alongside
`warp` — which the gate, CI and `cargo test -p warp` all do — turns `test-util` on, so those three
tests ran **zero** times in every configuration anyone actually runs. Fixed by moving the feature
guard to the function under test (`any(test, not(feature = "test-util"))`) and giving the module a
plain `#[cfg(test)]`. It is the only such module in the tree; the other `not(feature = "test-util")`
sites are production alternates, not tests.

The 13 failures in `-p warp --lib` are pre-existing test-isolation issues, unrelated to this fork's
work. Two secret-redaction tests alternate, so the count is sometimes 14. If you see a different
number, something changed — investigate rather than adjusting the number.

## 4. Mistakes worth not repeating

**A gate you have never seen fail is not evidence.** An early gate was written, run, reported clean,
and was vacuous: it had been copied somewhere its repo-root resolution broke, so an unmodified copy
and a deliberately-broken copy passed identically. Every gate now has a self-test that plants a real
violation and requires rejection, plus a control asserting the clean tree passes.

**Write test cases from the artifact, not from your own pattern.** The supply-chain gate matched a
literal `raw.githubusercontent.com/warpdotdev`, and its self-test passed — but the real script built
that URL from variables, so the literal appeared nowhere in it and the gate passed the very file it
was written for. The self-test now restores the real file from git history.

**The account-gate trap.** Upstream gates paid capabilities behind an account check. Remove accounts
and the predicate becomes constant, so the capability does not become free — it becomes *permanently
off*, silently. Three instances were found, including AI reporting itself unavailable at 312 call
sites, and BYOK erasing a key the user had saved. Test suites miss this class because they build auth
state with a helper that reports a **signed-in** user, so the state this fork is always in is never
exercised. Read the predicate whenever you remove an account check.

**Removing an asset without removing its consumer is silent.** Warp's logomark was a private-use
glyph (U+E500) patched into the bundled fonts. Subsetting it out was correct; three renderers kept
asking for it, so the AI loading indicator drew a missing-glyph box before every label. The asset
scanner verifies the asset is *gone* — nothing verified the code that *drew* it had been updated.

**Verify against the real consumer, not the linter.** `plutil -lint` accepts an XML comment
containing `--`; AMFI rejects the entire entitlements file, breaking signing. Lint is necessary, not
sufficient.

**Doc comments become user-visible.** A `///` on a clap args struct becomes the command's `--help`
text. One such comment cheerfully documented the flags it said had been removed.

**Check what you shipped, not what you built.** A stale TUI binary nearly went out as new, and one
release shipped the TUI when it should have shipped the GUI. Compare artifact timestamps against
`git log -1` before publishing.

## 5. macOS permissions

Heddle declares **one** entitlement: `com.apple.security.automation.apple-events`. Kept because a
terminal runs the user's own programs and macOS attributes a child process's request to the
responsible app — without it, `osascript` driving another application fails with error 1743.

Camera, microphone, contacts, calendars, reminders, location and photo library were removed together
with their `NSxxxUsageDescription` strings, after checking the code: none had any reference, and the
microphone's only consumer was agent voice input, which transcribes through `ServerVoiceTranscriber`
against a server this fork does not have and is not compiled in anyway.

**Entitlements and usage strings must move together.** An app that touches a protected resource while
holding the entitlement but no usage string is terminated by the OS.

## 6. Infrastructure facts

- The `upstream` remote is **fetch-only** (`git remote set-url --push upstream no-push`). Taking a
  change from Warp should be a deliberate act.
- All 20 `warpdotdev/*` git dependencies are pinned by `rev`. `verify-warp-supply-chain` enforces it.
- `script/bootstrap` and `script/run` used to `curl` a script from a Warp-owned repository at
  unpinned `main` and execute it. Removed. Bootstrap no longer fetches anything from Warp and no
  longer asks for a `gcloud` login.
- **Always pass `--repo owner/name` to `gh`.** While the GitHub repo was a fork, `gh release create`
  resolved the *parent* and published to `warpdotdev/Warp`. If the fork relationship has since been
  detached this is less dangerous, but the habit is cheap.
- Signing identity: `Developer ID Application: Nikolay Paskov (4STAAHTNCN)`. Notary profile is stored
  in the keychain as `heddle`. Warp's team ID `2BBY89MBSN` survives in exactly one place —
  `LEGACY_WARP_APP_GROUP_TEAM_ID`, used only to find pre-existing data to migrate.

## 7. Build performance

`release-lto` sets `debug = 0`. Upstream's inherited `debug = 1` plus `split-debuginfo = "packed"`
exists so Sentry can symbolicate crash reports; this fork compiles no Sentry, so that pipeline ran
`dsymutil` single-threaded and wrote a 2.2 GB `.dSYM` with no consumer — the serial tail that left an
18-core machine ~85% idle. Nothing observable changed, because `strip = "symbols"` already strips the
shipped binary.

If you need to debug a release binary: `CARGO_PROFILE_RELEASE_LTO_DEBUG=1`.

Note that `GIT_RELEASE_TAG` is recorded as an `env-dep` in cargo's dep-info, so changing it correctly
invalidates `warp_core` and everything downstream. A version bump therefore costs a near-full
rebuild. That is correct behaviour, not a bug.

## Upstream cherry-picking

Upstream is `warpdotdev/Warp`. The fork point is `a66337f4` (2026-07-21) —
`git merge-base HEAD upstream/master`, not the root commit of the public Warp repo.
`.upstream-sync` records the last evaluated sha, starting there.

    script/heddle/upstream-review          # fetches, then four buckets; read CANDIDATE only
    git cherry-pick <sha>                  # one at a time
    lefthook run gate                      # the ratchets get their say
    script/heddle/upstream-review --advance <sha>   # <sha> = the one the report printed
    git commit .upstream-sync -m "chore(upstream): evaluated through <sha>"

**`--advance` takes the sha, and the report prints it.** Copy its last line verbatim. The
sha is required because the runbook is two invocations: the report fetches, so a bare
`--advance` re-resolving `upstream/master` would record anything that landed in between as
evaluated, having appeared in no report anyone read. Advancing never fetches, and refuses
any sha that is not the one the report it just printed covers — so the marker can only
ever be set to a value a human had on screen.

**Do not run `git fetch upstream` first — the script does it.** That is deliberate: the
report is only as current as the last fetch, and `--advance` writes the result into a
tracked file as a permanent "evaluated through here" decision, so a forgotten fetch buries
real commits somewhere re-running does not reach. `--no-fetch` reports against the last
fetch when you are offline or deliberately re-reading; a failed fetch exits 2 rather than
producing a quietly short report.

Exit 2 is always infrastructure (no upstream remote, missing or unresolvable marker,
failed fetch). Finding commits never fails.

If a pick trips `gui-branding.baseline` or `gui-surfaces.baseline`, the default is to
DROP THE PICK, not re-record the baseline. Re-recording turns the ratchet into a
formality. Re-record only when the pick genuinely shrinks the surface, and check the
diff shows removals only.

`AUTO-REJECT` means the commit touches a file this fork deleted. That set is **derived at
run time** from `git diff --diff-filter=D --no-renames <merge-base> HEAD -- app/ crates/`,
not hand-maintained, so it stays correct as more is removed. Nothing to update when you
delete a subsystem.

`COLLISION` means upstream touched something this fork reworked deliberately — all of
`app/src/drive/`, the update mechanism, the channel config, the privacy page, the gates,
the workflows. Ours wins; the bucket is listed so repeated upstream activity there is
visible, not so it gets re-litigated each pass. Widen `COLLISION_RE` in the script when
you rework something new, and add a case to `upstream-review-selftest`.

### Known limits of upstream-review

Recorded from an adversarial review pass, accepted rather than fixed. None affects the
current 118-commit range; all three are latent and would need a hostile or unusual
upstream to matter. Fix them when the tool next gets attention.

- **The temporary marker file is a predictable path.** `${MARKER_FILE}.tmp.$$` is opened
  with an ordinary truncating redirect, so a symlink or hardlink pre-placed at that exact
  path would be followed and the real marker truncated. `mktemp` with exclusive creation
  closes it. Until then the atomic-rename guarantee holds against crashes and write
  errors, but not against a pre-placed temp path.
- **The subject sanitizer strips C0 and DEL only.** `LC_ALL=C tr -d '\000-\037\177'`
  leaves raw C1 controls (`0x80`-`0x9f`) intact, which some terminals interpret as 8-bit
  OSC/ST. A commit subject crafted with 8-bit C1 sequences could still affect the terminal
  rendering the report. Note also that the fixture injects only ESC, so the suite's
  "no BEL byte" assertion would pass against a sanitizer that filters ESC alone — that
  assertion is weaker than its name suggests.
- **Only `upstream/master` is exercised by the self-test**, so the fallback that selects
  the upstream default branch is not covered against a stale or unusual remote layout.

The first two were disproved claims before they were known limits: the spec asserted the
marker "is never opened for writing" and the suite asserted control bytes were filtered,
and both were true only under conditions nobody had checked. That is the failure mode this
tool exists to guard against, so it is worth naming here rather than in a commit message
nobody re-reads.
