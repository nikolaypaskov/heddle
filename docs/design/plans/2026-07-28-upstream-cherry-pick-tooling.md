# Upstream cherry-pick tooling — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A repeatable monthly pass that surfaces the handful of upstream commits worth cherry-picking, out of ~1800, without re-reviewing rejections.

**Architecture:** A tracked marker file records the last evaluated upstream sha. A bash script enumerates commits since that marker and sorts each into one of four buckets by the paths it touches. Classification is a **pure function over a newline-separated path list**, which is what makes the self-test hermetic — it needs no fixture repository and no dependency on upstream's real history.

**Tech Stack:** bash, git. No new dependencies — matching every other gate in `script/heddle/`.

## Global Constraints

- Spec: `docs/design/specs/2026-07-28-upstream-cherry-pick-design.md`
- Exit codes follow the existing gates: **2 = infrastructure failure** (missing remote, missing marker, not a git repo), **0 = report produced**. This script never fails the build; it is a review aid, not a gate.
- Upstream remote is named `upstream` → `https://github.com/warpdotdev/Warp.git`
- Fork point: `0dbd3d56`
- `collision` beats `candidate` when a commit touches both. This precedence is load-bearing.
- Every gate in `script/heddle/` has a `*-selftest` sibling. Three checks in this repo turned out to be vacuous; the self-tests are what caught them.
- Shell style: `set -uo pipefail`, never `set -e` in a script whose job is to capture non-zero exits.

---

### Task 1: Pure path classification with a hermetic self-test

**Files:**
- Create: `script/heddle/upstream-review`
- Create: `script/heddle/upstream-review-selftest`

**Interfaces:**
- Produces: `classify_paths()` — reads newline-separated paths on stdin, echoes exactly one of `ignore`, `auto-reject`, `collision`, `candidate`.
- Produces: sourcing guard, so the self-test can call `classify_paths` without executing the report.

- [ ] **Step 1: Write the failing self-test**

Create `script/heddle/upstream-review-selftest`:

```bash
#!/usr/bin/env bash
#
# Prove the classifier can be WRONG before trusting it to be right.
#
# The load-bearing case is the last one: a commit touching both a collision path and a
# candidate path must classify as `collision`. If that precedence inverts, an upstream
# change to the Drive account gate lands in the bucket a human reads with intent to
# accept -- the most expensive failure this script can have.
set -uo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
# shellcheck source=/dev/null
source "$REPO_ROOT/script/heddle/upstream-review"

fails=0
expect() {
  local label="$1" want="$2"; shift 2
  local got
  got="$(printf '%s\n' "$@" | classify_paths)"
  if [ "$got" != "$want" ]; then
    echo "FAIL [$label]: expected '$want', got '$got'" >&2
    fails=$((fails + 1))
  else
    echo "ok [$label]: $want"
  fi
}

expect "removed surface"        auto-reject "crates/cloud_objects/src/lib.rs"
expect "drive account gate"     collision   "app/src/drive/settings.rs"
expect "terminal core"          candidate   "crates/warp_terminal/src/pty.rs"
expect "upstream docs only"     ignore      "docs/CONTRIBUTING.md"
expect "collision beats candidate" collision "crates/warp_terminal/src/pty.rs" "app/src/autoupdate/mac.rs"
expect "reject beats candidate" auto-reject "crates/warp_terminal/src/pty.rs" "crates/cloud_objects/src/x.rs"

if [ "$fails" -ne 0 ]; then
  echo "SELF-TEST FAILED: $fails case(s). A classifier that cannot be shown to fail is not evidence." >&2
  exit 1
fi
echo "self-test passed"
```

- [ ] **Step 2: Run it to verify it fails**

Run: `chmod +x script/heddle/upstream-review-selftest && ./script/heddle/upstream-review-selftest`
Expected: FAIL — `upstream-review: No such file or directory`

- [ ] **Step 3: Write the minimal classifier**

Create `script/heddle/upstream-review`:

```bash
#!/usr/bin/env bash
#
# Which upstream commits are worth reading?
#
# Upstream has moved ~1800 commits since the fork at 0dbd3d56. Reading all of them
# monthly is not a thing anyone will actually do, so this sorts them and asks a human to
# read one bucket.
#
# Classification is by PATH, which is a proxy for intent and therefore imperfect. It is
# deliberately biased: `collision` and `auto-reject` are broad, so the cost of being
# wrong is skipping something useful rather than accepting something harmful.
#
# See docs/design/specs/2026-07-28-upstream-cherry-pick-design.md.
set -uo pipefail

# Subsystems this fork removed. Upstream work here is never wanted.
REMOVED_RE='^(crates/cloud_objects/|crates/warp_multi_agent|app/src/ai/blocklist/)'

# Subsystems this fork reworked deliberately. OURS WINS -- see the spec for why this is
# a default rather than a judgement call.
COLLISION_RE='^(app/src/drive/settings\.rs|app/src/autoupdate/|crates/warp_core/src/channel/|script/heddle/|\.github/workflows/)'

# Only these trees can contain a change worth taking.
CODE_RE='^(app/|crates/)'

# Pure function: newline-separated paths on stdin -> exactly one bucket on stdout.
# Precedence, highest first: auto-reject, collision, candidate, ignore.
classify_paths() {
  local paths reject=0 collide=0 code=0
  paths="$(cat)"
  while IFS= read -r p; do
    [ -n "$p" ] || continue
    [[ "$p" =~ $REMOVED_RE ]] && reject=1
    [[ "$p" =~ $COLLISION_RE ]] && collide=1
    [[ "$p" =~ $CODE_RE ]] && code=1
  done <<< "$paths"

  if [ "$reject" = 1 ]; then echo auto-reject
  elif [ "$collide" = 1 ]; then echo collision
  elif [ "$code" = 1 ]; then echo candidate
  else echo ignore
  fi
}

# Sourced by the self-test: define the functions and stop.
[ "${BASH_SOURCE[0]}" != "${0}" ] && return 0
```

- [ ] **Step 4: Run the self-test to verify it passes**

Run: `chmod +x script/heddle/upstream-review && ./script/heddle/upstream-review-selftest`
Expected: six `ok` lines, then `self-test passed`

- [ ] **Step 5: Mutation-check the precedence**

Temporarily swap the `reject`/`collide` branches so `collision` is checked first, re-run the self-test, and confirm `reject beats candidate` FAILS. Then revert.

This is the check that matters: without it, "self-test passed" only proves the script ran.

- [ ] **Step 6: Commit**

```bash
git add script/heddle/upstream-review script/heddle/upstream-review-selftest
git commit -m "feat(upstream): classify upstream commits by path, with a hermetic self-test"
```

---

### Task 2: Marker file and commit enumeration

**Files:**
- Create: `.upstream-sync`
- Modify: `script/heddle/upstream-review`

**Interfaces:**
- Consumes: `classify_paths()` from Task 1.
- Produces: a report on stdout, four sections, `candidate` last.

- [ ] **Step 1: Create the marker at the fork point**

```bash
echo "0dbd3d56" > .upstream-sync
```

- [ ] **Step 2: Append enumeration and reporting to `upstream-review`**

Add below the sourcing guard:

```bash
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$REPO_ROOT" || exit 2
MARKER_FILE="$REPO_ROOT/.upstream-sync"

git rev-parse --git-dir >/dev/null 2>&1 || { echo "error: not a git repository" >&2; exit 2; }
git remote get-url upstream >/dev/null 2>&1 || {
  echo "error: no 'upstream' remote. Add it with:" >&2
  echo "       git remote add upstream https://github.com/warpdotdev/Warp.git" >&2
  exit 2
}
[ -f "$MARKER_FILE" ] || { echo "error: $MARKER_FILE is missing" >&2; exit 2; }

MARKER="$(tr -d '[:space:]' < "$MARKER_FILE")"
git cat-file -e "${MARKER}^{commit}" 2>/dev/null || {
  echo "error: marker '$MARKER' is not a commit. Run: git fetch upstream" >&2
  exit 2
}

UPSTREAM_REF="$(git rev-parse --verify --quiet upstream/main || git rev-parse --verify --quiet upstream/master)"
[ -n "$UPSTREAM_REF" ] || { echo "error: cannot resolve upstream default branch" >&2; exit 2; }

declare -a b_reject=() b_collide=() b_candidate=()
ignored=0

while IFS= read -r sha; do
  [ -n "$sha" ] || continue
  bucket="$(git show --name-only --pretty=format: "$sha" | classify_paths)"
  subject="$(git log -1 --format='%h %s' "$sha")"
  case "$bucket" in
    auto-reject) b_reject+=("$subject") ;;
    collision)   b_collide+=("$subject") ;;
    candidate)   b_candidate+=("$subject") ;;
    *)           ignored=$((ignored + 1)) ;;
  esac
done < <(git rev-list --reverse "${MARKER}..${UPSTREAM_REF}")

echo "upstream review: ${MARKER} -> $(git rev-parse --short "$UPSTREAM_REF")"
echo "  ignored (no app/ or crates/ change): $ignored"
echo
# Listed but not read. A rejection nobody sees is indistinguishable from a commit that
# never existed -- and repeated upstream work on a removed surface is worth noticing.
echo "AUTO-REJECT (${#b_reject[@]}) — touches a removed subsystem:"
printf '  %s\n' "${b_reject[@]:-  (none)}"
echo
echo "COLLISION (${#b_collide[@]}) — touches something we reworked; ours wins:"
printf '  %s\n' "${b_collide[@]:-  (none)}"
echo
echo "CANDIDATE (${#b_candidate[@]}) — read these:"
printf '  %s\n' "${b_candidate[@]:-  (none)}"
```

- [ ] **Step 3: Run it against real history**

Run: `git fetch upstream && ./script/heddle/upstream-review | head -20`
Expected: a report whose bucket counts sum to roughly 1798 minus ignored. Sanity-check that `AUTO-REJECT` is in the hundreds — the spec measured 635 commits touching removed surfaces.

- [ ] **Step 4: Verify the infrastructure failures are real**

```bash
mv .upstream-sync /tmp/ && ./script/heddle/upstream-review; echo "exit=$?"   # expect exit=2
mv /tmp/.upstream-sync .
```
Expected: `exit=2` with the missing-marker message, not a silent empty report.

- [ ] **Step 5: Commit**

```bash
git add .upstream-sync script/heddle/upstream-review
git commit -m "feat(upstream): enumerate and bucket commits since the recorded marker"
```

---

### Task 3: Marker advance and the runbook

**Files:**
- Modify: `script/heddle/upstream-review`
- Modify: `docs/HANDOFF.md`

**Interfaces:**
- Consumes: the report from Task 2.
- Produces: `--advance` flag; a documented pass procedure.

- [ ] **Step 1: Add `--advance`**

Insert argument parsing after `cd "$REPO_ROOT"`:

```bash
ADVANCE=false
case "${1:-}" in
  --advance) ADVANCE=true ;;
  "") ;;
  *) echo "usage: $0 [--advance]" >&2; exit 2 ;;
esac
```

And at the end of the script:

```bash
if [ "$ADVANCE" = true ]; then
  # Advances past REJECTED commits too. Rejections are decisions; resurfacing them makes
  # each pass grow rather than shrink, which is how 1798 accumulated.
  git rev-parse "$UPSTREAM_REF" > "$MARKER_FILE"
  echo
  echo "marker advanced to $(git rev-parse --short "$UPSTREAM_REF") — commit .upstream-sync with any picks"
fi
```

- [ ] **Step 2: Verify advance writes the sha and is idempotent**

```bash
./script/heddle/upstream-review --advance | tail -2
./script/heddle/upstream-review | head -3   # expect 0 in every bucket
git checkout .upstream-sync                  # restore for the real first pass
```
Expected: the second run reports empty buckets, proving the marker took effect.

- [ ] **Step 3: Document the pass in `docs/HANDOFF.md`**

Add under a new `## Upstream cherry-picking` heading:

```markdown
Upstream is `warpdotdev/Warp`; `.upstream-sync` records the last evaluated sha.

    git fetch upstream
    script/heddle/upstream-review          # four buckets; read CANDIDATE only
    git cherry-pick <sha>                  # one at a time
    lefthook run gate                      # the ratchets get their say
    script/heddle/upstream-review --advance
    git commit .upstream-sync -m "chore(upstream): evaluated through <sha>"

If a pick trips `gui-branding.baseline` or `gui-surfaces.baseline`, the default is to
DROP THE PICK, not re-record the baseline. Re-recording turns the ratchet into a
formality. Re-record only when the pick genuinely shrinks the surface, and check the
diff shows removals only.

`COLLISION` means upstream touched something this fork reworked deliberately — the Drive
account gate, the update mechanism, the gates. Ours wins; the bucket is listed so
repeated upstream activity there is visible, not so it gets re-litigated each pass.
```

- [ ] **Step 4: Run the full gate**

Run: `./script/heddle/upstream-review-selftest && lefthook run gate`
Expected: self-test passes; gate passes.

- [ ] **Step 5: Commit**

```bash
git add script/heddle/upstream-review docs/HANDOFF.md
git commit -m "feat(upstream): advance the marker past evaluated commits, and document the pass"
```

---

## Self-review

**Spec coverage.** Marker → Task 2. Four buckets → Task 1. Collision precedence → Task 1 Step 1, mutation-checked at Step 5. Removed/collision path lists → Task 1 Step 3, verbatim from the spec. Marker advances past rejections → Task 3 Step 1. Ratchet handling → Task 3 Step 3. Self-test convention → Task 1.

**Placeholders.** None. Every step carries runnable code or an exact command with expected output.

**Type consistency.** `classify_paths` is defined in Task 1 and consumed in Task 2 with the same stdin/stdout contract. `MARKER_FILE`, `UPSTREAM_REF` and `ADVANCE` are introduced before use.

**Known gap, deliberate:** path-based classification is a proxy for intent. An upstream commit can touch `crates/warp_terminal` while being cloud plumbing, or touch a collision path incidentally. The bias is toward over-rejecting, so the failure mode is missing something useful rather than accepting something harmful. If the candidate bucket proves too noisy in the first real pass, tighten `CODE_RE` rather than loosening the other two.
