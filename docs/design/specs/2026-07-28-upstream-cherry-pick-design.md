# Heddle — upstream cherry-pick strategy

**Status:** Designed, not implemented
**Date:** 2026-07-28

## The situation, measured

Heddle forked `warpdotdev/Warp` at `0dbd3d56` (2026-04-28). Since then:

| | Count |
|---|---:|
| Upstream commits | 1799 |
| Touching `app/` or `crates/` | 1591 |
| Touching the terminal core | 556 |
| Touching subsystems this fork removed | 406 |
| Mentioning security / CVE / RUSTSEC in the message | **0** |

That last row shapes the urgency, with a caveat recorded below.

## Decisions

1. **Cherry-pick selectively.** Never merge upstream wholesale. At this divergence a
   merge would conflict everywhere and fight the ratchets on every file.
2. **Scope: bug fixes and terminal features.** Not fixes-only. Heddle should remain a
   good terminal, not merely a stable one.
3. **Cadence: monthly-ish, batched.** One focused pass rather than continuous drip.
4. **Philosophy filter.** Reject anything requiring cloud, account or telemetry, however
   well it works.
5. **Ours wins on collision.** Where upstream touches a subsystem this fork reworked,
   reject theirs without deep review.

## Why "ours wins" is a default rather than a judgement call

The rule exists because the judgement erodes. Three weeks after fixing the account gate
that made Drive invisible to every user, it is obvious that an upstream Drive commit
must not be taken blindly. Six months later, an upstream commit tidying Drive search
will look like a free improvement. Encoding the list makes the default outlive the
memory of why it was set.

The cost is real: occasionally a genuine upstream improvement to a collision subsystem
will be skipped. That is accepted.

## Mechanism

Two artefacts.

**`.upstream-sync`** — a tracked file recording the last upstream sha evaluated.
Committed, so the marker survives machines and its movement is visible in review.

**`script/heddle/upstream-review`** — bash, matching the other gates in that directory:
read-only, self-testing, no new dependencies.

### Classification

Every commit since the marker lands in exactly one bucket, by the paths it touches:

| Bucket | Rule | Action |
|---|---|---|
| `ignore` | Touches nothing under `app/` or `crates/` | Silent |
| `auto-reject` | Touches a removed surface | Listed, not reviewed |
| `collision` | Touches a reworked subsystem | Listed with its subject — ours wins |
| `candidate` | Everything else | **The only bucket read** |

**Precedence: collision beats candidate.** A commit touching both must classify as
collision. Getting this backwards is the most expensive failure available to this
script — an upstream change to the Drive gate would land in the bucket read with intent
to accept.

**`ignore` is silent; `auto-reject` is listed.** A rejection never seen is
indistinguishable from a commit that never existed. If upstream fixes the same cloud
bug five times, that pattern is worth noticing even though the answer stays no.

### Removed surfaces

```
crates/cloud_objects/
crates/warp_multi_agent_*/
app/src/ai/blocklist/           orchestration, ambient agents
```

### Collision paths

Each entry is a deliberate decision that an upstream change would silently undo:

```
app/src/drive/settings.rs       the account gate that hid Drive from every user
app/src/autoupdate/             consent-gated updates, notarisation and team checks
crates/warp_core/src/channel/   endpoint and channel configuration
script/heddle/                  the gates themselves
.github/workflows/              CI, privacy gate, release
```

## A sync pass

```
1.  git fetch upstream
2.  script/heddle/upstream-review        → four buckets, candidates last
3.  read candidates; cherry-pick wanted ones individually
4.  lefthook run gate                    → the ratchets get their say
5.  advance .upstream-sync, commit with the picks
```

**The marker advances past rejected commits too.** Rejections are decisions; resurfacing
them makes each pass grow rather than shrink, which is how 1799 accumulated in the first
place.

## When a cherry-pick trips a ratchet

A pick that ADDS to `gui-branding.baseline` or `gui-surfaces.baseline` fails the gate.

**Default: reject the pick.** Re-recording is a one-liner and feels like unblocking
yourself; it converts the safety net into a formality. An upstream commit that
reintroduces a vendor brand string or a commercial UI surface is exactly what the
ratchet exists to stop.

**Narrow exception:** the pick genuinely shrinks the surface. Then `--update` is correct,
and the diff must show removals only.

## Testing

The script gets a self-test, matching every other gate in `script/heddle/`. That
convention is not decoration: three separate checks in this repo turned out to be
vacuous — a semgrep rule that matched nothing, a version guard that errored on every
real tag, and a release workflow whose trigger never fired. Self-tests are what caught
them.

Fixture commits, asserting classification:

| Fixture touches | Expected |
|---|---|
| `crates/cloud_objects/…` | `auto-reject` |
| `app/src/drive/settings.rs` | `collision` |
| `crates/warp_terminal/…` | `candidate` |
| upstream docs only | `ignore` |
| **both a collision and a candidate path** | **`collision`** |

The last row is the one that matters.

## Recorded uncertainty

**The zero-security finding is not proven.** It comes from grepping upstream commit
messages for `security|CVE|vulnerab|overflow|panic|unsound|RUSTSEC`. If upstream does not
label security fixes that way — many projects do not — the inference "no urgency" is
unfounded. Message-grepping is the same shape of check that has produced wrong answers
in this repo before.

What is independently true: Rust security issues arrive predominantly through
dependencies, and `cargo deny check advisories` already runs on every build. That
argument stands on its own and does not depend on the grep.

**The removed-surface count above was wrong in the first draft.** It originally read 635,
measured over the pathspec `crates/cloud_objects app/src/ai app/src/drive` — which counts
all of `app/src/ai` rather than just `app/src/ai/blocklist/`, and counts `app/src/drive`,
which this fork did not remove. Drive was *restored*, not removed: an always-false account
gate had hidden it from every user, and it is listed in this document's own collision path
list, not the removed one. The corrected figure, measured over the pathspec this document
actually specifies, is 406. The error is instructive on its own terms: the wrong
measurement treated a restored subsystem as a removed one, which is the exact confusion
the collision list exists to prevent.

**Not consulted:** Codex was asked to critique this design and both attempts ran too long
to be useful — it is fast on concrete diffs and slow on open-ended design questions. It
should review this spec as a written artefact instead, which is where it has repeatedly
found real defects.
