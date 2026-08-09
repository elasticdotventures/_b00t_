#!/bin/bash
# check-submodule-drift.sh — submodule pin drift sync gate (#923)
# 🤓 Rebasing without `git submodule update` leaves submodules pinned to
#    stale commits, producing compile errors that look like real upstream
#    breakage but aren't. This script is the single source of truth for
#    detecting + (optionally) safely fixing that drift — both `just doctor`
#    and `b00t doctor check` call it. It's a standalone bash script (not
#    Rust) on purpose: a pre-cargo gate that itself requires compiling
#    b00t-cli is a chicken-and-egg risk (see justfile's viz-entangle
#    comment: "cargo run fails on b00t repo due to git worktree structure").
#
# Classification per .gitmodules path:
#   OK             — HEAD matches recorded gitlink.
#   UNINIT         — .gitmodules lists it, but no .git inside (never
#                    initialized). NOT drift, never a failure.
#   ORPHANED       — .gitmodules stanza but no matching git tree entry
#                    (stale entry). Skipped, never a failure.
#   BROKEN         — .git exists but `rev-parse HEAD` fails (issue #924's
#                    "gutted gitdir" shape). Classified separately, never
#                    touched, never a failure here.
#   DRIFTED+CLEAN  — HEAD != recorded pin, no uncommitted TRACKED changes.
#                    Safe to auto-sync via `git submodule update`. Counted
#                    as a failure UNLESS --fix successfully resolves it.
#   DRIFTED+DIRTY  — HEAD != recorded pin, tracked changes present. Report
#                    only — NEVER touched, regardless of --fix. Always
#                    counted as a failure.
#
# Orthogonal to the above: for any submodule with a `branch = ` declared in
# .gitmodules, each entry also carries a `branch_status` field (2026-08-09,
# after the vendor/ledgrrr incident — see `b00t lfmf ledgrrr-sync`):
#   n/a      — no branch= declared for this path.
#   unknown  — branch= declared, but refs/remotes/origin/<branch> isn't
#              fetched locally. Never fetches over the network itself (this
#              runs on every `b00t doctor check`; a network call per
#              submodule would be slow and, per the same day's WSL/WARP MTU
#              incident, can hang indefinitely) — not a failure, just
#              inconclusive. Run `git -C <path> fetch origin <branch>` first.
#   ok       — the RECORDED pin is an ancestor of origin/<branch>.
#   stale    — the recorded pin is NOT reachable from origin/<branch>: the
#              declared branch moved (force-push/rebase) out from under a
#              pin that used to be on it, or the pin was bumped straight to
#              a side branch that was never merged into the declared one.
#              This is exactly how vendor/ledgrrr silently stranded 34
#              commits until a manual audit caught it. Counted as a failure
#              — there is no mechanical fix (see the doc comment above);
#              reconcile upstream, don't just re-bump the pin.
#
# "Dirty" means uncommitted changes to TRACKED files only
# (`git status --porcelain --untracked-files=no`), NOT any untracked file.
# `git submodule update` only moves the checked-out ref via `git checkout`,
# which never touches untracked files and refuses if it would clobber
# uncommitted tracked changes — so untracked noise is irrelevant to safety.
#
# Usage:
#   check-submodule-drift.sh              # report only
#   check-submodule-drift.sh --fix         # auto-sync drifted+clean submodules
#   check-submodule-drift.sh --json        # machine-readable array
#   check-submodule-drift.sh --verbose     # also print OK entries
#
# Exit code: 0 if zero unresolved drift after any --fix attempt, else 1.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# _b00t_/scripts/ is always two levels below repo root — resolve via the
# script's own location, not `git rev-parse --show-toplevel` (needs no
# work-tree assumptions; this repo uses bare-repo + multiple worktrees).
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

FIX=false
JSON=false
VERBOSE=false

for arg in "$@"; do
    case "$arg" in
        --fix) FIX=true ;;
        --json) JSON=true ;;
        --verbose) VERBOSE=true ;;
        *)
            echo "check-submodule-drift.sh: unknown argument: $arg" >&2
            exit 2
            ;;
    esac
done

cd "$REPO_ROOT"

FAIL_COUNT=0
DIRTY_COUNT=0
UNFIXED_CLEAN_COUNT=0
STALE_BRANCH_COUNT=0
ITEMS=()
HUMAN_LINES=()

# Emit one compact JSON object for a single submodule's classification.
# Uses jq -n to build it (handles quoting/escaping of non-ASCII paths like
# python.🐍/DALLE2-pytorch correctly, avoids hand-rolled JSON escaping).
emit_json() {
    local path="$1" recorded="$2" checked_out="$3" status="$4" action="$5" branch_status="$6" branch_detail="$7"
    jq -nc \
        --arg path "$path" \
        --arg recorded "$recorded" \
        --arg checked_out "$checked_out" \
        --arg status "$status" \
        --arg action "$action" \
        --arg branch_status "$branch_status" \
        --arg branch_detail "$branch_detail" \
        '{path:$path,
          recorded: (if $recorded == "" then null else $recorded end),
          checked_out: (if $checked_out == "" then null else $checked_out end),
          status: $status,
          action: $action,
          branch_status: $branch_status,
          branch_detail: $branch_detail}'
}

# Record one submodule's classification: builds the JSON item, tracks
# failure counts, and queues a human-readable line (OK lines only shown
# with --verbose).
record() {
    local path="$1" recorded="$2" checked_out="$3" status="$4" action="$5" is_fail="$6" branch_status="${7:-n/a}" branch_detail="${8:-}"
    ITEMS+=("$(emit_json "$path" "$recorded" "$checked_out" "$status" "$action" "$branch_status" "$branch_detail")")
    if [[ "$is_fail" == "1" ]]; then
        FAIL_COUNT=$((FAIL_COUNT + 1))
    fi
    if [[ "$status" != "ok" || "$branch_status" == "stale" || "$VERBOSE" == "true" ]]; then
        HUMAN_LINES+=("  $(printf '%-14s' "$status") $path  recorded=${recorded:-<none>} checked_out=${checked_out:-<none>}  -- $action")
        if [[ "$branch_status" == "stale" ]]; then
            HUMAN_LINES+=("                 ⚠️  branch_status=stale -- $branch_detail")
        fi
    fi
}

# Declared-branch ancestry check (2026-08-09, see doc comment at top).
# Deliberately local-only: never fetches. Sets globals BRANCH_STATUS /
# BRANCH_DETAIL for the caller to pass into record().
check_branch_ancestry() {
    local path="$1" recorded="$2"
    BRANCH_STATUS="n/a"
    BRANCH_DETAIL=""
    local branch
    branch=$(git config --file .gitmodules --get "submodule.$path.branch" 2>/dev/null || true)
    [[ -z "$branch" ]] && return

    if ! git -C "$path" rev-parse -q --verify "refs/remotes/origin/$branch" >/dev/null 2>&1; then
        BRANCH_STATUS="unknown"
        BRANCH_DETAIL="origin/$branch not fetched locally in the submodule -- run: git -C $path fetch origin $branch"
        return
    fi

    if git -C "$path" merge-base --is-ancestor "$recorded" "refs/remotes/origin/$branch" 2>/dev/null; then
        BRANCH_STATUS="ok"
        BRANCH_DETAIL="recorded pin is an ancestor of origin/$branch"
    else
        BRANCH_STATUS="stale"
        BRANCH_DETAIL="recorded pin is NOT reachable from origin/$branch -- the declared branch moved (force-push/rebase?) and stranded this pin, or it was bumped straight to an unmerged side branch. Reconcile upstream (merge/rebase both lineages together, verify, then re-bump) -- do not just fast-forward the pin, that silently drops whatever the pin has that the declared branch doesn't. See: b00t lfmf ledgrrr-sync"
        STALE_BRANCH_COUNT=$((STALE_BRANCH_COUNT + 1))
    fi
}

while IFS= read -r path; do
    [[ -z "$path" ]] && continue

    rec=$(git ls-tree HEAD -- "$path" | awk '{print $3}')
    if [[ -z "$rec" ]]; then
        # ORPHANED — stale .gitmodules entry, no matching git tree entry.
        record "$path" "" "" "orphaned" "skip (stale .gitmodules entry, no matching git tree entry)" 0
        continue
    fi

    if [[ ! -e "$path/.git" ]]; then
        # UNINIT — never initialized. Not drift.
        record "$path" "$rec" "" "uninit" "skip (never initialized — run: git submodule update --init -- $path)" 0
        continue
    fi

    if ! act=$(git -C "$path" rev-parse HEAD 2>/dev/null); then
        # BROKEN — issue #924's "gutted gitdir" shape. Never touched here.
        record "$path" "$rec" "" "broken" "skip (cannot resolve HEAD inside submodule — see #924)" 0
        continue
    fi

    # Declared-branch ancestry — orthogonal to the recorded-vs-checked-out
    # comparison below, so computed once here and threaded into every
    # record() call in this iteration regardless of which status it gets.
    check_branch_ancestry "$path" "$rec"
    branch_is_fail=0
    [[ "$BRANCH_STATUS" == "stale" ]] && branch_is_fail=1

    if [[ "$rec" == "$act" ]]; then
        # OK — HEAD matches recorded pin.
        record "$path" "$rec" "$act" "ok" "none" "$branch_is_fail" "$BRANCH_STATUS" "$BRANCH_DETAIL"
        continue
    fi

    # Drifted: HEAD != recorded pin. Only "dirty" if TRACKED files changed —
    # untracked noise (several vendor submodules carry thousands of
    # untracked files) must NOT make this un-fixable.
    dirty=$(git -C "$path" status --porcelain --untracked-files=no)
    if [[ -n "$dirty" ]]; then
        # DRIFTED+DIRTY — report only, NEVER touched regardless of --fix.
        DIRTY_COUNT=$((DIRTY_COUNT + 1))
        record "$path" "$rec" "$act" "drifted_dirty" "manual resolution required (tracked changes present) — NOT touched" 1 "$BRANCH_STATUS" "$BRANCH_DETAIL"
        continue
    fi

    # DRIFTED+CLEAN — safe to auto-sync.
    if [[ "$FIX" == "true" ]]; then
        if git submodule update -- "$path" >/dev/null 2>&1; then
            record "$path" "$rec" "$act" "drifted_fixed" "synced to recorded pin ($act -> $rec)" "$branch_is_fail" "$BRANCH_STATUS" "$BRANCH_DETAIL"
        else
            UNFIXED_CLEAN_COUNT=$((UNFIXED_CLEAN_COUNT + 1))
            record "$path" "$rec" "$act" "drifted_clean" "sync attempted but failed" 1 "$BRANCH_STATUS" "$BRANCH_DETAIL"
        fi
    else
        UNFIXED_CLEAN_COUNT=$((UNFIXED_CLEAN_COUNT + 1))
        record "$path" "$rec" "$act" "drifted_clean" "sync available — safe, run with --fix" 1 "$BRANCH_STATUS" "$BRANCH_DETAIL"
    fi
done < <(git config --file .gitmodules --get-regexp '\.path$' 2>/dev/null | cut -d' ' -f2- || true)

if [[ "$JSON" == "true" ]]; then
    if [[ "${#ITEMS[@]}" -eq 0 ]]; then
        echo "[]"
    else
        printf '%s\n' "${ITEMS[@]}" | jq -s '.'
    fi
else
    echo "🥾 b00t doctor — submodule pin drift check"
    if [[ "${#HUMAN_LINES[@]}" -gt 0 ]]; then
        printf '%s\n' "${HUMAN_LINES[@]}"
    fi
    if [[ "$FAIL_COUNT" -eq 0 ]]; then
        echo "PASS: 0 drifted submodules"
    else
        echo "FAIL: $FAIL_COUNT drifted submodules (dirty: $DIRTY_COUNT, clean-unfixed: $UNFIXED_CLEAN_COUNT, stale-branch-pin: $STALE_BRANCH_COUNT)"
    fi
fi

[[ "$FAIL_COUNT" -eq 0 ]]
