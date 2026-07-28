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
./script/bundle -c oss --release-tag vX.Y.Z                      # GUI  -> Heddle.app
./script/bundle -c oss --artifact tui --release-tag vX.Y.Z       # TUI  -> heddle-tui
```

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
`Heddle-*.app.zip` is present, because it builds only the TUI.

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
```

Each has a `-selftest` companion. **Run the self-test, not just the gate.** The reason is §4.

To record a deliberate addition to a baseline: `--update --allow-additions`, and say why in the
commit. The gate refuses additions otherwise, on purpose.

### Testing

```bash
cargo test -p warp --lib      # ~5,593 pass, 13 known isolation failures
cargo test -p warp_core       # 49
cargo test -p warp_tui        # 524
cargo test -p onboarding      # 12
```

**`cargo test` alone covers only workspace default members, and `warp_core` is not one of them.**
Four failures hid behind that for a long time. Always name the crate.

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
