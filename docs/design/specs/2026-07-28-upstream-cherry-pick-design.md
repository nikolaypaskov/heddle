# Heddle — upstream cherry-pick strategy

**Status:** Implemented — `script/heddle/upstream-review`, `.upstream-sync`
**Date:** 2026-07-28

## The situation, measured

Heddle's fork point is `git merge-base HEAD upstream/master` = **`a66337f4`** (2026-07-21,
"Add TUI logout slash command (#14117)"). Measured over
`a66337f4..upstream/master --not HEAD`:

| | Pathspec / filter | Count |
|---|---|---:|
| Upstream commits not already in HEAD | — | 118 |
| Touching `app/` or `crates/` | `app/ crates/` | 104 |
| Touching the terminal core | `app/src/terminal crates/warp_terminal` | 15 |
| Touching subsystems this fork removed | the AUTO-REJECT bucket | 8 |
| Mentioning security / CVE / panic / RUSTSEC in the message | `git log -Ei --grep` | 11 |

Every pathspec is named because the first draft of this table did not name one, and the
row measured over an unstated pathspec was the row that turned out to be wrong.

An earlier draft recorded the fork point as `0dbd3d56` and the backlog as 1799 commits.
`0dbd3d56` is the **root commit of the public Warp repository**, not a fork point:
1681 of those 1799 commits were already ancestors of HEAD. Every number in the original
table inherited that error. See "Recorded uncertainty" for what else it took down with it.

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

**`.upstream-sync`** — a tracked file recording the last upstream sha evaluated, starting
at the fork point `a66337f4`. Committed, so the marker survives machines and its movement
is visible in review.

The marker is hand-maintained, and it has already been wrong once. Enumeration therefore
passes `--not HEAD` as well, so git decides what HEAD does not already contain and the
report self-corrects against marker drift instead of compounding it.

**`script/heddle/upstream-review`** — bash, matching the other gates in that directory:
self-testing, no new dependencies. Read-only with respect to the working tree and to
upstream; the two things it does write are the `upstream` remote's refs (it fetches) and,
under `--advance` only, `.upstream-sync`. An earlier draft called it "read-only" flatly,
which was wrong in exactly the direction that matters — the marker write is the one action
here with lasting consequences.

### Classification

Every commit since the marker lands in exactly one bucket, by the paths it touches:

| Bucket | Rule | Action |
|---|---|---|
| `auto-reject` | Touches a removed surface | Listed, not reviewed |
| `collision` | Touches a reworked subsystem | Listed with its subject — ours wins |
| `candidate` | Touches `app/` or `crates/`, and neither of the above | **The only bucket read** |
| `ignore` | Matches nothing we classify | Silent (a count only) |

`ignore` is not "no `app/` or `crates/` change": a commit touching only
`.github/workflows/` has no such change and is still reported, under `collision`. The
buckets are evaluated in the order above and always sum to the commit count.

**Precedence: collision beats candidate.** A commit touching both must classify as
collision. Getting this backwards is the most expensive failure available to this
script — an upstream change to the Drive gate would land in the bucket read with intent
to accept.

**`ignore` is silent; `auto-reject` is listed.** A rejection never seen is
indistinguishable from a commit that never existed. If upstream fixes the same cloud
bug five times, that pattern is worth noticing even though the answer stays no.

### Removed surfaces — derived, not listed

A hand-maintained list of removed paths is wrong the day after the next removal lands.
The first draft of this document listed three entries; measured against the tree, they
covered 4 of the 160 files this fork has actually deleted, and two of the three named
subsystems Heddle still ships (`crates/cloud_objects/` is a live workspace member;
`app/src/ai/blocklist/` is 201 present files). An upstream panic fix in either would have
appeared under a heading reading "touches a removed subsystem" and never been read.

So the set is derived at run time:

```
git diff --diff-filter=D --no-renames --name-only <merge-base> HEAD -- app/ crates/
```

A path is auto-reject if it **existed at the merge base and is gone from HEAD**. Both
halves matter. Without the first, every file upstream newly *adds* is also absent from
HEAD, so the whole candidate bucket collapses into auto-reject — measured: 95 candidates
become 72, with 23 commits silently written off. `script/heddle/upstream-review-selftest`
asserts that case by name.

`--no-renames` is deliberate. A path the fork renamed away (`warpify_page.rs` →
`heddlify_page.rs`) is a path upstream must not be allowed to reintroduce, and rename
detection is a similarity heuristic whose results move with git version and config.

### The same reasoning applies to reading each commit's paths

The per-commit path list is `git show -m --name-only --no-renames`. Both flags earn their
place, and both were measured rather than assumed:

**`--no-renames`.** Plain `git show --name-only` reports a rename as its **destination
only**. So an upstream commit moving `app/src/drive/panel.rs` to `app/src/elsewhere.rs`
prints one path, `COLLISION_RE` never sees `app/src/drive/`, and a guarded subsystem walks
out of its own guard into the bucket a human reads with intent to accept. Reproduced on a
throwaway repo: one path with detection on, both paths with it off.

**`-m`.** A merge's combined diff lists only paths that differ from *all* parents, so a
merge that takes one side wholesale reports little or nothing. Measured on this repo's own
merge `5d1035fa5`: **7 paths without `-m`, 37 with it**. Upstream has **0** merge commits
across its entire history (`git rev-list --count --merges upstream/master`) because it
squash-merges, so nothing today depends on this — it is here because a monthly tool
outlives the assumption that upstream never changes its merge policy. Root commits are
unaffected either way (`-m` still lists all 4982 paths of `0dbd3d56`).

Neither flag changes any classification on the current range: no commit among the 118
contains a rename or is a merge, and bucket membership is byte-identical before and after.
That is what makes them cheap to add now rather than after they matter.

One prefix rule survives alongside the derived set, because the derived set cannot see
files upstream *adds* inside a tree this fork deleted wholesale:

```
crates/warp_multi_agent*        removed here; upstream still adds files to it
```

### Collision paths

Each entry is a deliberate decision that an upstream change would silently undo:

```
app/src/drive/                  the account gate that hid Drive from every user, and the
                                ten other files the restoration reworked
app/src/settings_view/privacy_page.rs   the de-branding rework
app/src/autoupdate/             consent-gated updates, notarisation and team checks
crates/warp_core/src/channel/   endpoint and channel configuration
script/heddle/                  the gates themselves
.github/workflows/              CI, privacy gate, release
```

The Drive entry was `app/src/drive/settings.rs` in the first draft — one file of an
eleven-file rework, which left `app/src/drive/panel.rs` and the rest landing in the
bucket a human reads with intent to accept. The guard is only as wide as the change it
guards.

The rest of the de-branding rework — `app/src/settings/heddlify_key_migration*.rs` and
the renamed `heddlify_page.rs` — is not listed. Those paths do not exist upstream, so no
upstream commit can touch them; listing them would be decoration. The rename *source*,
`warpify_page.rs`, is covered by the derived removed set.

## A sync pass

```
1.  script/heddle/upstream-review        → fetches, then four buckets, candidates last
2.  read candidates; cherry-pick wanted ones individually
3.  lefthook run gate                    → the ratchets get their say
4.  script/heddle/upstream-review --advance <sha>   ← the sha step 1 printed
5.  commit .upstream-sync with the picks
```

**The marker is bound to the reviewed sha, not to the remote's current tip.** Step 4 is a
second invocation; with fetch-on-start, a bare `--advance` would re-resolve
`upstream/master` and write whatever landed in the meantime — recording as evaluated
commits that appeared in no report anyone read. So `--advance` requires the sha, the report
prints it as its last line for copy-paste, and advancing never fetches. It refuses a sha
that is not on upstream's history, and refuses one that is not the tip the report it just
printed covers, both with exit 2. The marker can only be set to a value a human had on
screen.

Fetch-on-start narrowed that window; it did not close it. Only binding the write to the
reviewed sha closes it, and the two changes together are what make the pass safe — either
alone is not enough.

**The script fetches; the runbook does not.** The report is only ever as current as the
last fetch, and `--advance` turns it into a tracked, permanent "evaluated through here"
decision — so a forgotten fetch buries real commits in a file that re-running does not
recover. Detecting staleness instead was considered and rejected: there is no reliable
local signal for it (a ref's mtime is not a fetch time once refs are packed), and a guard
that cannot observe what it claims to check is the vacuous-check pattern this repo keeps
finding. `--no-fetch` reports against the last fetch when that is what you want; a failed
fetch is exit 2, never a quietly short report.

**The marker advances past rejected commits too.** Rejections are decisions; resurfacing
them makes each pass grow rather than shrink.

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

Fixture path lists, asserting classification. The derived removed set is passed in as
data, so the test needs no fixture repository and no dependency on what upstream has
actually deleted today:

| Fixture touches | Expected |
|---|---|
| `crates/warp_multi_agent_client/…` | `auto-reject` |
| a path in the derived removed set | `auto-reject` |
| `app/src/drive/panel.rs` | `collision` |
| `crates/warp_terminal/…` | `candidate` |
| `crates/cloud_objects/…`, `app/src/ai/blocklist/…` (still shipped) | `candidate` |
| upstream docs only | `ignore` |
| **both a collision and a candidate path** | **`collision`** |
| **a path in neither HEAD nor the derived set (upstream ADDS it)** | **`candidate`** |

The last two rows are the ones that matter. The first is precedence: get it backwards and
an upstream Drive change lands in the bucket read with intent to accept. The second is the
"existed at the merge base" half of the removal rule: get it wrong and everything new
upstream is written off, which reads to a human as "nothing to review".

### The second half of the self-test, and what neither half covers

The sourcing guard returns before the report body, so sourcing the script cannot reach the
enumeration, the report, or the marker write at all. Those are tested by a second section
that runs the script as a subprocess against a disposable repository under `$TMPDIR`, with
a `git` shim on `PATH` that breaks one call on demand:

The fixture's shape is the point. Every commit in it exists to make one specific way of
being wrong fail:

```
A (merge base)  keep.rs, gone.rs, drive/panel.rs, drive/index.rs   ← the marker starts here
 ├─ upstream/master:
 │    B  adds app/src/feature.rs                        → candidate
 │    C  modifies app/src/gone.rs                       → auto-reject (derived)
 │    E  RENAMES drive/panel.rs → app/src/moved_out.rs  → collision
 │    G  modifies drive/index.rs, on a side branch
 │    F  MERGE of G                                     → collision
 └─ D (HEAD)     DELETES app/src/gone.rs                ← the fork side
```

D makes the *derivation* testable — a file genuinely deleted between the merge base and
HEAD, which section 1 cannot exercise because it injects the set by hand. E and F cover the
two path-extraction flags above.

| Case | Expected |
|---|---|
| nothing broken | exit 0; `AUTO-REJECT (1)`, `COLLISION (3)`, `CANDIDATE (1)`, **and each named commit in its own bucket**; advance command printed |
| `git fetch` exits non-zero | exit 2, message naming `--no-fetch`, **marker untouched** |
| default run (fetch succeeds) | exit 0, full report |
| `git rev-list` exits non-zero | exit 2, message, **marker untouched** |
| `git rev-list` truncates but exits **0** | exit 2, message, **marker untouched** |
| `git show` exits non-zero for one commit | exit 2, message, **marker untouched** |
| `--advance` with no sha, or with a flag | exit 2, message, **marker untouched** |
| `--advance` with an unresolvable sha | exit 2, message, **marker untouched** |
| `--advance` with a sha off upstream's history | exit 2, message, **marker untouched** |
| `--advance` with a sha that is not the report's tip | exit 2, message, **marker untouched** |
| `--advance` with the marker read-only | exit 2, message, **marker untouched** |
| `--advance <reviewed tip>` | exit 0, marker becomes that sha |

Every failure case asserts all three of exit status, stderr, and marker — any one alone
would let the defect through. The last row is not decoration: a guard that blocks
everything is not a guard.

**Bucket counts are not asserted alone.** An earlier version of the clean-run case checked
only `AUTO-REJECT (1)` and `CANDIDATE (1)`, which cannot fail the way the case is named: a
derivation that put `feature.rs` in the removed set instead of `gone.rs` swaps two commits,
preserves both counts, and passes. Demonstrated by mutation — with the set swapped, all
three count assertions still pass and only the two by-name assertions fail. The suite now
names which commit must be in which bucket.

**Known limit, deliberate.** Neither section verifies that the report's *contents* are
right — only that the buckets are as expected for a four-commit fixture, that the counts
are internally consistent, and that failures are loud. A green self-test is evidence the
classifier and the failure paths behave; it is not evidence that any particular real
upstream commit was bucketed correctly. That check is the human reading pass.

An earlier version of this section claimed the derivation could not be covered here at all.
That was wrong: the fixture just needed a deletion in it.

## Recorded uncertainty

**The zero-security finding was wrong, twice over.** The first draft reported **0**
commits mentioning `security|CVE|vulnerab|overflow|panic|unsound|RUSTSEC` and built the
urgency argument on it. Re-run with `git log -Ei --grep`, the real figures are **11** over
the corrected range and **114** over the range the first draft thought it was measuring.
Among the 11 is `43a41099d fix: update cmov to 0.5.4 to resolve CVE-2026-50185`. A row
reading "0" was not a weak signal; it was a broken measurement, and it is not knowable
from the draft how it was produced.

Message-grepping remains the same shape of check that has produced wrong answers in this
repo before, so the corrected number is a floor, not a census: upstream may fix security
bugs without saying so.

Where those 11 land: 7 `candidate`, 1 `collision`, 3 `ignore`. The three ignored are two
lockfile-only bumps (`2fe6a4f56`, `06eedd6fc` — root `Cargo.lock`, outside `app/` and
`crates/`) and `43a41099d`, which despite its CVE subject changes **no files at all** and
so has nothing to classify. Lockfile bumps being invisible to this tool is deliberate,
and it is the one place where the independent argument carries the weight: Rust security
issues arrive predominantly through dependencies, and `cargo deny check advisories`
already runs on every build against this fork's own lockfile, which is the graph that
actually ships. Upstream's bumps are not the mechanism by which Heddle learns about
advisories.

**The removed-surface count above was wrong in both earlier drafts.** It first read 635,
measured over the pathspec `crates/cloud_objects app/src/ai app/src/drive` — which counts
all of `app/src/ai` rather than just `app/src/ai/blocklist/`, and counts `app/src/drive`,
which this fork did not remove. Drive was *restored*, not removed: an always-false account
gate had hidden it from every user, and it is listed in this document's own collision path
list, not the removed one. It was then corrected to 406 — still measured over the wrong
1799-commit range, and still measured with a hand-written path list that covered 4 of 160
removed files. The figure in the table above is the AUTO-REJECT bucket the tool actually
produces, over the real range, from the derived set.

Both errors are instructive on the same point: every wrong number here came from a
measurement whose inputs were never re-derived, only re-stated.

**Not consulted:** Codex was asked to critique this design and both attempts ran too long
to be useful — it is fast on concrete diffs and slow on open-ended design questions. It
should review this spec as a written artefact instead, which is where it has repeatedly
found real defects.
