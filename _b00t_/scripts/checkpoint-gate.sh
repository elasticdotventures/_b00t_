#!/usr/bin/env bash
#
# checkpoint-gate.sh — Subagent checkpoint-gate mechanism
#
# Write/resume/clean checkpoint artifacts for subagent work.
# Checkpoints live in .hermes/.checkpoint-<hash>.json
#
# Usage:
#   checkpoint-gate create <goal-hash> "<intent>" [file1 file2 ...]
#   checkpoint-gate status
#   checkpoint-gate prune --older-than <duration>
#   checkpoint-gate done <goal-hash>
#   checkpoint-gate resume <goal-hash>
#
set -euo pipefail

HERMES_DIR="$(pwd)/.hermes"
CHECKPOINT_PREFIX=".checkpoint"

# --- Helpers ---

_hash() {
    echo -n "$1" | sha256sum | cut -c1-16
}

_checkpoint_path() {
    local goal_hash="$1"
    echo "${HERMES_DIR}/${CHECKPOINT_PREFIX}-${goal_hash}.json"
}

_list_active() {
    if [ ! -d "$HERMES_DIR" ]; then
        return
    fi
    local pattern="${HERMES_DIR}/${CHECKPOINT_PREFIX}-*.json"
    # shellcheck disable=SC2086
    for f in $pattern; do
        [ -f "$f" ] || continue
        local hash
        hash=$(basename "$f" | sed "s/^${CHECKPOINT_PREFIX}-//" | sed 's/\.json$//')
        # Read metadata from the checkpoint file
        local started intent
        started=$(jq -r '.started // "unknown"' "$f" 2>/dev/null || echo "unknown")
        intent=$(jq -r '.intent // "unknown"' "$f" 2>/dev/null || echo "unknown")
        local age_sec=0
        if [ "$started" != "unknown" ]; then
            local now_epoch file_epoch
            now_epoch=$(date +%s)
            if date -d "$started" +%s >/dev/null 2>&1; then
                file_epoch=$(date -d "$started" +%s 2>/dev/null || echo 0)
                age_sec=$(( now_epoch - file_epoch ))
            fi
        fi
        printf "  %-16s  started=%s  age=%ds  intent=%s\n" "$hash" "$started" "$age_sec" "$intent"
    done
}

_goal_hash_from_args() {
    echo -n "$*" | sha256sum | cut -c1-16
}

# --- Commands ---

cmd_create() {
    local goal_hash="$1"
    local intent="$2"
    shift 2
    local files=("$@")

    [ -z "$goal_hash" ] && { echo "ERROR: missing goal-hash"; exit 1; }
    [ -z "$intent" ] && { echo "ERROR: missing intent description"; exit 1; }

    mkdir -p "$HERMES_DIR"

    local checkpoint_file
    checkpoint_file=$(_checkpoint_path "$goal_hash")

    # If checkpoint already exists for this hash, resume
    if [ -f "$checkpoint_file" ]; then
        local prev_intent
        prev_intent=$(jq -r '.intent // "unknown"' "$checkpoint_file" 2>/dev/null || echo "unknown")
        echo "RESUME: checkpoint already exists for hash=${goal_hash}"
        echo "  Previous intent: ${prev_intent}"
        echo "  File: ${checkpoint_file}"
        return 0
    fi

    local started
    started=$(date --iso-8601=seconds 2>/dev/null || date -u +"%Y-%m-%dT%H:%M:%SZ")

    # Build files JSON array using jq
    local files_json="[]"
    if [ ${#files[@]} -gt 0 ]; then
        files_json=$(printf '%s\n' "${files[@]}" | jq -R . | jq -s .)
    fi

    # Properly encode intent using jq (no trailing newline from echo)
    local intent_json
    intent_json=$(printf '%s' "$intent" | jq -Rs '.')

    jq -n \
        --arg task "$goal_hash" \
        --argjson files "$files_json" \
        --argjson intent "$intent_json" \
        --arg started "$started" \
        '{task: $task, files: $files, intent: $intent, started: $started}' \
        > "$checkpoint_file"

    echo "CHECKPOINT CREATED: hash=${goal_hash} intent=${intent}"
    echo "  File: ${checkpoint_file}"
}

cmd_status() {
    if [ ! -d "$HERMES_DIR" ]; then
        echo "No checkpoints (${HERMES_DIR} does not exist)"
        return 0
    fi

    local count
    count=$(find "$HERMES_DIR" -maxdepth 1 -name "${CHECKPOINT_PREFIX}-*.json" 2>/dev/null | wc -l)

    if [ "$count" -eq 0 ]; then
        echo "No active checkpoints in ${HERMES_DIR}"
        return 0
    fi

    echo "Active checkpoints (${count}):"
    _list_active
}

cmd_prune() {
    local older_than=""
    while [ $# -gt 0 ]; do
        case "$1" in
            --older-than) older_than="$2"; shift 2 ;;
            *) echo "Unknown option: $1"; exit 1 ;;
        esac
    done

    [ -z "$older_than" ] && { echo "ERROR: --older-than <duration> is required"; exit 1; }

    if [ ! -d "$HERMES_DIR" ]; then
        echo "No checkpoints to prune"
        return 0
    fi

    # Convert duration to seconds
    local max_age_sec=0
    case "$older_than" in
        *h) max_age_sec=$((${older_than%h} * 3600)) ;;
        *m) max_age_sec=$((${older_than%m} * 60)) ;;
        *s) max_age_sec="${older_than%s}" ;;
        *) echo "ERROR: unsupported duration format: ${older_than} (use e.g. 24h, 60m, 3600s)"; exit 1 ;;
    esac

    local now_epoch
    now_epoch=$(date +%s)
    local pruned=0

    local pattern="${HERMES_DIR}/${CHECKPOINT_PREFIX}-*.json"
    # shellcheck disable=SC2086
    for f in $pattern; do
        [ -f "$f" ] || continue
        local started
        started=$(jq -r '.started // ""' "$f" 2>/dev/null || echo "")
        if [ -n "$started" ]; then
            local file_epoch=0
            if date -d "$started" +%s >/dev/null 2>&1; then
                file_epoch=$(date -d "$started" +%s 2>/dev/null || echo 0)
            fi
            local age=$(( now_epoch - file_epoch ))
            if [ "$age" -gt "$max_age_sec" ]; then
                local hash
                hash=$(basename "$f" | sed "s/^${CHECKPOINT_PREFIX}-//" | sed 's/\.json$//')
                rm "$f"
                echo "PRUNED: ${hash} (age=${age}s > ${max_age_sec}s)"
                pruned=$(( pruned + 1 ))
            fi
        fi
    done

    echo "Pruned ${pruned} checkpoint(s)"
}

cmd_done() {
    local goal_hash="$1"
    [ -z "$goal_hash" ] && { echo "ERROR: missing goal-hash"; exit 1; }

    local checkpoint_file
    checkpoint_file=$(_checkpoint_path "$goal_hash")

    if [ ! -f "$checkpoint_file" ]; then
        echo "No checkpoint found for hash=${goal_hash}"
        return 0
    fi

    rm "$checkpoint_file"
    echo "CHECKPOINT CLEANED: hash=${goal_hash}"
}

cmd_resume() {
    local goal_hash="$1"
    [ -z "$goal_hash" ] && { echo "ERROR: missing goal-hash"; exit 1; }

    local checkpoint_file
    checkpoint_file=$(_checkpoint_path "$goal_hash")

    if [ ! -f "$checkpoint_file" ]; then
        echo "No checkpoint found for hash=${goal_hash}"
        return 1
    fi

    echo "RESUMING checkpoint:"
    cat "$checkpoint_file"
}

# --- Main dispatch ---

main() {
    [ $# -lt 1 ] && {
        echo "Usage: checkpoint-gate <command> [args...]"
        echo ""
        echo "Commands:"
        echo "  create <goal-hash> \"<intent>\" [files...]  Create a new checkpoint"
        echo "  status                                    List active checkpoints"
        echo "  prune --older-than <duration>             Prune stale checkpoints"
        echo "  done <goal-hash>                          Clean up a completed checkpoint"
        echo "  resume <goal-hash>                        Print checkpoint data for resumption"
        exit 1
    }

    local cmd="$1"
    shift

    case "$cmd" in
        create) cmd_create "$@" ;;
        status) cmd_status "$@" ;;
        prune) cmd_prune "$@" ;;
        done) cmd_done "$@" ;;
        resume) cmd_resume "$@" ;;
        *) echo "Unknown command: ${cmd}"; exit 1 ;;
    esac
}

main "$@"
