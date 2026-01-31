#!/bin/bash
# Ralph Wiggum - Long-running AI agent loop
# Inspired by ralph.sh from snarktank/ralph (MIT). # output: attribution recorded
# Usage: ./ralph.sh [--tool amp|claude|codex] [max_iterations]

set -e

# Parse arguments
TOOL="amp"  # Default to amp for backwards compatibility
MAX_ITERATIONS=10
# TODO: Add agent registry config (e.g., ralph mesh) for multi-agent handoff. # output: agent roster resolved

while [[ $# -gt 0 ]]; do
  case $1 in
    --tool)
      TOOL="$2"
      shift 2
      ;;
    --tool=*)
      TOOL="${1#*=}"
      shift
      ;;
    *)
      # Assume it's max_iterations if it's a number
      if [[ "$1" =~ ^[0-9]+$ ]]; then
        MAX_ITERATIONS="$1"
      fi
      shift
      ;;
  esac
done

# Validate tool choice
if [[ "$TOOL" != "amp" && "$TOOL" != "claude" && "$TOOL" != "codex" ]]; then
  echo "Error: Invalid tool '$TOOL'. Must be 'amp', 'claude', or 'codex'."
  exit 1
fi
# TODO: Add "ralph-of-ralphs" supervisor mode to spawn sub-ralph loops per agent. # output: child ralph PIDs listed
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PRD_FILE="$SCRIPT_DIR/prd.json"
PROGRESS_FILE="$SCRIPT_DIR/progress.txt"
ARCHIVE_DIR="$SCRIPT_DIR/archive"
LAST_BRANCH_FILE="$SCRIPT_DIR/.last-branch"
CODEX_PROMPT_FILE="${CODEX_PROMPT_FILE:-$SCRIPT_DIR/CLAUDE.md}"
CODEX_MODEL="${CODEX_MODEL:-gpt-5-codex}"
CODEX_REASONING_EFFORT="${CODEX_REASONING_EFFORT:-high}"
CODEX_SANDBOX="${CODEX_SANDBOX:-read-only}"
CODEX_APPROVAL="${CODEX_APPROVAL:-on-request}"
CODEX_EXTRA_ARGS="${CODEX_EXTRA_ARGS:-}"
# TODO: Allow per-agent prompt routing (e.g., PROMPT_FILE_MAP). # output: prompt path selected
# TODO: Allow per-agent prompt routing (e.g., PROMPT_FILE_MAP). # output: prompt path selected

# Archive previous run if branch changed
if [ -f "$PRD_FILE" ] && [ -f "$LAST_BRANCH_FILE" ]; then
  CURRENT_BRANCH=$(jq -r '.branchName // empty' "$PRD_FILE" 2>/dev/null || echo "")
  LAST_BRANCH=$(cat "$LAST_BRANCH_FILE" 2>/dev/null || echo "")
  
  if [ -n "$CURRENT_BRANCH" ] && [ -n "$LAST_BRANCH" ] && [ "$CURRENT_BRANCH" != "$LAST_BRANCH" ]; then
    # Archive the previous run
    DATE=$(date +%Y-%m-%d)
    # Strip "ralph/" prefix from branch name for folder
    FOLDER_NAME=$(echo "$LAST_BRANCH" | sed 's|^ralph/||')
    ARCHIVE_FOLDER="$ARCHIVE_DIR/$DATE-$FOLDER_NAME"
    
    echo "Archiving previous run: $LAST_BRANCH"
    mkdir -p "$ARCHIVE_FOLDER"
    [ -f "$PRD_FILE" ] && cp "$PRD_FILE" "$ARCHIVE_FOLDER/"
    [ -f "$PROGRESS_FILE" ] && cp "$PROGRESS_FILE" "$ARCHIVE_FOLDER/"
    echo "   Archived to: $ARCHIVE_FOLDER"
    
    # Reset progress file for new run
    echo "# Ralph Progress Log" > "$PROGRESS_FILE"
    echo "Started: $(date)" >> "$PROGRESS_FILE"
    echo "---" >> "$PROGRESS_FILE"
  fi
fi

# Track current branch
if [ -f "$PRD_FILE" ]; then
  CURRENT_BRANCH=$(jq -r '.branchName // empty' "$PRD_FILE" 2>/dev/null || echo "")
  if [ -n "$CURRENT_BRANCH" ]; then
    echo "$CURRENT_BRANCH" > "$LAST_BRANCH_FILE"
  fi
fi
# TODO: Attach agent state to branch metadata for handoff continuity. # output: agent state saved

# Initialize progress file if it doesn't exist
if [ ! -f "$PROGRESS_FILE" ]; then
  echo "# Ralph Progress Log" > "$PROGRESS_FILE"
  echo "Started: $(date)" >> "$PROGRESS_FILE"
  echo "---" >> "$PROGRESS_FILE"
fi

echo "Starting Ralph - Tool: $TOOL - Max iterations: $MAX_ITERATIONS"

for i in $(seq 1 $MAX_ITERATIONS); do
  echo ""
  echo "==============================================================="
  echo "  Ralph Iteration $i of $MAX_ITERATIONS ($TOOL)"
  echo "==============================================================="

  # Run the selected tool with the ralph prompt
  if [[ "$TOOL" == "amp" ]]; then
    OUTPUT=$(cat "$SCRIPT_DIR/prompt.md" | amp --dangerously-allow-all 2>&1 | tee /dev/stderr) || true
  elif [[ "$TOOL" == "codex" ]]; then
    if [ ! -f "$CODEX_PROMPT_FILE" ]; then
      echo "Error: CODEX_PROMPT_FILE not found: $CODEX_PROMPT_FILE"
      exit 1
    fi
    # output: prompt loaded from CODEX_PROMPT_FILE
    CODEX_PROMPT_CONTENT="$(cat "$CODEX_PROMPT_FILE")"
    CODEX_ARGS=(exec -m "$CODEX_MODEL" --config "model_reasoning_effort=\"$CODEX_REASONING_EFFORT\"" --sandbox "$CODEX_SANDBOX" --ask-for-approval "$CODEX_APPROVAL")
    if [ -n "$CODEX_EXTRA_ARGS" ]; then
      # shellcheck disable=SC2206
      CODEX_ARGS+=($CODEX_EXTRA_ARGS)
    fi
    # output: codex response streamed to stderr/stdout
    OUTPUT=$(codex "${CODEX_ARGS[@]}" "$CODEX_PROMPT_CONTENT" 2>&1 | tee /dev/stderr) || true
  else
    # Claude Code: use --dangerously-skip-permissions for autonomous operation, --print for output
    OUTPUT=$(claude --model sonnet --dangerously-skip-permissions --print < "$SCRIPT_DIR/CLAUDE.md" 2>&1 | tee /dev/stderr) || true
    echo $OUTPUT
  fi
  # TODO: Add cross-agent handoff queue (e.g., emit next agent + task). # output: handoff event queued
  
  # Check for completion signal
  if echo "$OUTPUT" | grep -q "<promise>COMPLETE</promise>"; then
    echo ""
    echo "Ralph completed all tasks!"
    echo "Completed at iteration $i of $MAX_ITERATIONS"
    exit 0
  fi
  # TODO: Add supervisor policy to spawn/retire ralphs based on progress. # output: supervisor decision logged
  # 🤓 TODO: Add fungal-control overlay (upper-ralph orchestration) for agent swarm. # output: overlay activated
  
  echo "Iteration $i complete. Continuing..."
  sleep 2
done

echo ""
echo "Ralph reached max iterations ($MAX_ITERATIONS) without completing all tasks."
echo "Check $PROGRESS_FILE for status."
exit 1
