# Update notification for Heddle.app

**Status:** implemented in v0.5.0. **Design date:** 2026-07-27.

The implemented GUI flow is opt-in and asked once; the historical rationale and constraints below
remain the design record. `heddle-tui` remains deliberately out of scope.

## The problem

v0.3.0 shipped with two real defects: it added itself to the user's login items without
asking, and it stored data inside Warp's app-group container. v0.3.1 fixed both. **Nobody
running v0.3.0 can learn that v0.3.1 exists.** There is no update path, no notice, and no
reason for a user to check. Shipping fixes to people who cannot find out about them is
close to not shipping them.

This spec covers the GUI app only. The terminal binary (`heddle-tui`) is deliberately out
of scope — see [Deferred](#deferred).

## The tension, stated plainly

Heddle's README says *nothing leaves this device*, and today that is literally true. Any
update check is a network request, and a network request reveals an IP address and the fact
that someone is running Heddle. There is no version of this feature that keeps the
unqualified claim.

The resolution is not to make the request invisible but to make it **chosen**: Heddle asks
once, in plain language, and does nothing until answered. A product whose pitch is *not
doing things behind your back* cannot quietly acquire a default that phones home — but it
also should not hide behind that pitch as an excuse to leave known-broken builds in the
field.

## Decisions

| Decision | Choice | Why |
| --- | --- | --- |
| Default posture | **Ask once on first launch** | Neither a silent default nor a setting nobody finds. The user decides with the tradeoff in front of them. |
| Scope of action | **One-click update** — download, verify, install | Notice-only reaches far fewer people. The machinery already exists and is platform-complete. |
| Artifacts | **`Heddle.app` only** | One install shape, one place to show a notice, atomic bundle replacement. |
| Trust root | **Apple Developer ID + notarization + monotonicity** | No new secret to protect; the same trust users already rely on when they first open the app. |
| Signed manifest | **Rejected (YAGNI)** | The payload's Apple signature already proves provenance; monotonicity closes the downgrade hole a manifest signature would catch. It would add a second key to lose. |

## Architecture

Five pieces. Four already exist in the tree, inherited from upstream and currently inert
because `autoupdate` is not a default feature.

| Piece | Current state | Work required |
| --- | --- | --- |
| Version check | `app/src/autoupdate/channel_versions.rs` fetches via Warp's `ServerApi` | **Replace** with a plain HTTPS GET |
| Manifest type | `crates/channel_versions` — `ChannelVersions { dev, preview, stable, changelogs }` | Reuse unchanged |
| Notice UI | `AutoupdateStateEvent::UpdateAvailable`, handled in `workspace/view.rs` | Reuse |
| Download / stage / install | `app/src/autoupdate/{mod,mac}.rs`, ~4,600 lines | Reuse; enable the feature |
| Consent, monotonicity, notarization check | — | **New**, ~150 lines |

### Manifest hosting

The manifest is an ordinary release asset published at:

```
https://github.com/nikolaypaskov/heddle/releases/latest/download/channel_versions.json
```

GitHub resolves `latest/download/<name>` to the asset on the current release, so the entire
infrastructure is *one extra file per release*. No server, no new hostname, no new secret,
nothing to operate.

All three channels (`dev`, `preview`, `stable`) are published pointing at the same stable
release. Heddle ships one channel; keeping the struct's shape avoids churn in
`crates/channel_versions` and leaves the door open if that ever changes.

## Data flow

```
launch
  └─ consent answered?
       no  ─→ show the one-time notice, store the answer, do nothing else this run
       yes ─→ enabled?
                no  ─→ stop. No network activity of any kind.
                yes ─→ GET the manifest      (HTTPS; no body, no identifier, no telemetry)
                         └─ manifest.stable.version > running version?
                              no  ─→ stop, silently
                              yes ─→ notice: "Heddle 0.3.2 is available"  [Update] [Dismiss]
                                       └─ Update ─→ download ─→ VERIFY ─→ install on next launch
```

### Verification — all three must pass, in this order

1. **Developer ID.** `codesign -v -R="certificate leaf[subject.OU] = 4STAAHTNCN"` against
   the staged bundle. An attacker needs the project's actual Apple signing identity.
2. **Notarization.** `spctl -a -t exec` must report `source=Notarized Developer ID`. This
   is a *separate* Apple gate: it catches a build signed with a leaked key that was never
   submitted to Apple.
3. **Monotonicity.** The staged version must be **strictly greater** than the running one.

Point 3 is not paranoia. An older Heddle release is *validly signed* — checks 1 and 2 pass
on it. Without a version rule, anyone able to influence what the manifest points at could
serve a downgrade to a build with a known bug, and every signature check would agree.

**Why this deserves the scrutiny:** until 2026-07-26, `verify_code_signature` compared
against `APPLE_TEAM_ID`, which held **Warp's** team identifier. The updater would have
accepted a Warp-signed update and rejected a Heddle-signed one. It was dormant, so nothing
broke and nothing caught it. Enabling this feature makes that check load-bearing for the
first time.

## The first-run notice

A one-time notice on first launch, using the same surface as the update notice itself —
one component, not two. Onboarding is being dismantled, so a first-run slide would be the
wrong home.

Wording, to be used as written:

> **Check for updates automatically?**
> Heddle can check GitHub for new releases. This reveals your IP address to GitHub and
> nothing else — no identifier, no usage data, no telemetry.
> You can change this any time in Settings.
> [ Yes ] [ No ]

The answer is stored as a normal setting. Until it is answered, **no network request is
made**, including on subsequent launches if the user dismisses without choosing.

## Error handling

Every failure is silent and non-blocking: no network, DNS failure, malformed manifest,
GitHub unreachable, HTTP error. The app behaves exactly as if the feature were off. An
update check that interrupts someone's terminal because a CDN hiccupped is worse than no
update check.

**One exception:** a verification failure is logged. A downloaded bundle that fails the
Developer ID or notarization check is a fact worth having, not noise — it means either a
corrupted download or something worse. It is never installed.

## Testing

`WARP_CHANNEL_VERSIONS_PATH` already exists and reads the manifest from a local file, so
the whole flow is testable with no network.

Required tests — each written alongside its code, not after:

| Test | Asserts |
| --- | --- |
| consent unanswered | **zero** network calls |
| consent declined | **zero** network calls, on this and every later launch |
| manifest version == running | no update offered |
| manifest version < running | no update offered (downgrade refused) |
| manifest version > running | update offered |
| bundle signed by a different team | rejected, not installed, failure logged |
| bundle signed but not notarized | rejected, not installed, failure logged |
| manifest malformed / unreachable | silent no-op, app unaffected |

The last four are the ones that matter, and the ones easiest to write so they pass against
a broken implementation. Three defects this project shipped — the account gate, the vacuous
SAST rule, and the telemetry test — all existed because something was asserted without
being exercised. A test that cannot fail is worse than no test, because it reports safety.

## Release process changes

1. Publish `channel_versions.json` as a release asset, alongside the existing artifacts.
2. Generate it from the tag being released, so version and manifest cannot disagree.
3. Add a release-time check that the published manifest's `stable.version` matches the tag.

## Deferred

- **`heddle-tui`.** A bare executable is a different install shape: no bundle to swap
  atomically, no stapled ticket to validate locally (Apple staples only to bundles, disk
  images and packages), and it may live anywhere on `PATH`. A TUI notice would also compete
  with the user's actual terminal output. Deferred until someone asks — recorded here so
  the omission reads as a decision rather than an oversight.
- **Signed manifest.** See [Decisions](#decisions).
- **Linux and Windows.** Neither is currently published as a signed artifact.

## Success criteria

1. A user on v0.3.1 learns that v0.3.2 exists, without being asked to check.
2. A user who declines is never contacted, and no network request is made.
3. A tampered, downgraded, or non-notarized bundle is never installed.
4. Every failure mode leaves the app working exactly as before.
