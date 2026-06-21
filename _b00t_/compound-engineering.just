# Compound Engineering Harness — b00t native workflow
# Inspired by everyinc/compound-engineering-plugin
# 8-phase agile state machine with gh-issues backlog
# Datum: _b00t_/compound-engineering.datum.toml
# Namespaced: just ce-<recipe>

set shell := ["bash", "-cu"]
set unstable

# ── Status ─────────────────────────────────────────────────────────────────

# Show current project phase and backlog
status:
    #!/bin/bash
    set -euo pipefail
    STATE_FILE=".b00t/compound-engineering-state.json"
    echo "🥾 Compound Engineering — Project Status"
    echo "──────────────────────────────────────────"
    if [ -f "$STATE_FILE" ]; then
        echo "Current phase: $(jq -r '.current_phase' "$STATE_FILE")"
        echo "Started:       $(jq -r '.started_at' "$STATE_FILE")"
        echo "Last advance:  $(jq -r '.last_advance' "$STATE_FILE")"
        echo "Phase history: $(jq -r '.phase_history | join(" → ")' "$STATE_FILE")"
    else
        echo "Current phase: strategy (initial)"
        echo "No state file — run 'just ce-advance' after completing a phase"
    fi
    echo ""
    echo "📋 Backlog:"
    gh issue list --label compound-engineering --limit 5 --json title,labels,state --jq '.[] | "  \(.state | ascii_upcase): \(.title)"' 2>/dev/null || echo "  (gh CLI not configured)"

# ── Phase Execution ────────────────────────────────────────────────────────

# Phase 1: Strategy — define target problem, approach, users, metrics
ce-phase-strategy:
    #!/bin/bash
    set -euo pipefail
    echo "🎯 Phase 1: Strategy"
    echo "Define: target problem, approach, users, key metrics"
    echo ""
    echo "Output: STRATEGY.md (repo root)"
    echo "Backlog: creates epic issue with label 'compound-engineering'"
    echo ""
    echo "Run: b00t plan --template strategy"
    echo "Then: just ce-advance"

# Phase 2: Ideate — generate ranked feature ideas
ce-phase-ideate:
    #!/bin/bash
    set -euo pipefail
    echo "💡 Phase 2: Ideate"
    echo "Generate and rank feature ideas via grok dual-backend"
    echo ""
    echo "Run: b00t grok ask 'product feature ideas' --topic ideation"
    echo "Output: _b00t_/ideation/ranked-ideas.md"
    echo "Then: just ce-advance"

# Phase 3: Brainstorm — produce requirements doc
ce-phase-brainstorm feature="":
    #!/bin/bash
    set -euo pipefail
    FEATURE="{{feature}}"
    echo "🧠 Phase 3: Brainstorm"
    echo "Feature: ${FEATURE:-<specify with feature=name>}"
    echo ""
    echo "Run: b00t plan --template brainstorm --feature ${FEATURE:-feature-name}"
    echo "Output: _b00t_/requirements/${FEATURE:-feature-name}.md"
    echo "Then: just ce-advance"

# Phase 4: Plan — detailed implementation plan
ce-phase-plan feature="":
    #!/bin/bash
    set -euo pipefail
    FEATURE="{{feature}}"
    echo "📐 Phase 4: Plan"
    echo "Feature: ${FEATURE:-<specify with feature=name>}"
    echo ""
    echo "Run: b00t plan --feature ${FEATURE:-feature-name}"
    echo "  → Creates _b00t_/plans/<feature>-plan.md"
    echo "  → Creates gh issues for each task"
    echo "Run: b00t task list"
    echo "Then: just ce-advance"

# Phase 5: Work — execute tasks
ce-phase-work:
    #!/bin/bash
    set -euo pipefail
    echo "🔨 Phase 5: Work"
    echo ""
    echo "Run: b00t task next"
    echo "  → Opens worktree, executes task"
    echo "  → Commits, pushes to branch"
    echo "Run: b00t task done <task-id>"
    echo "Then: just ce-advance"

# Phase 6: Code Review — multi-agent adversarial review
ce-phase-review:
    #!/bin/bash
    set -euo pipefail
    echo "🔍 Phase 6: Code Review"
    echo ""
    echo "Dispatching 3 review agents (security, correctness, coherence)..."
    echo "Run: b00t code-review --mode adversarial"
    echo "Output: _b00t_/reviews/<feature>-review.md"
    echo "Then: just ce-advance"

# Phase 7: Compound — document learnings
ce-phase-compound:
    #!/bin/bash
    set -euo pipefail
    echo "🧪 Phase 7: Compound"
    echo ""
    echo "Document learnings to make future work easier:"
    echo "  1. b00t lfmf <tool> '<topic>: <body>'"
    echo "  2. b00t grok digest -t <topic> '<content>'"
    echo "  3. git add _b00t_/learn/"
    echo "Then: just ce-advance"

# Phase 8: Product Pulse — time-windowed report
ce-phase-pulse window="7d":
    #!/bin/bash
    set -euo pipefail
    WINDOW="{{window}}"
    echo "📊 Phase 8: Product Pulse (${WINDOW} window)"
    echo ""
    echo "Run: b00t doctor check --json > _b00t_/pulse-reports/$(date +%Y-%m-%d).json"
    echo "  → Also query /api/admin/health?window=${WINDOW}"
    echo "Output: _b00t_/pulse-reports/$(date +%Y-%m-%d).md"
    echo "Then: just ce-advance (loops back to strategy)"

# ── State Machine ──────────────────────────────────────────────────────────

# Advance to next phase (FOL-guarded)
ce-advance:
    #!/bin/bash
    set -euo pipefail
    STATE_FILE=".b00t/compound-engineering-state.json"
    PHASES=("strategy" "ideate" "brainstorm" "plan" "work" "review" "compound" "pulse")

    # Read current phase
    if [ -f "$STATE_FILE" ]; then
        CURRENT=$(jq -r '.current_phase' "$STATE_FILE")
        # Find next phase
        for i in "${!PHASES[@]}"; do
            if [ "${PHASES[$i]}" = "$CURRENT" ]; then
                NEXT="${PHASES[$((i + 1))]:-${PHASES[0]}}"
                break
            fi
        done
    else
        CURRENT="<none>"
        NEXT="strategy"
    fi

    echo "🥾 Advancing: $CURRENT → $NEXT"

    # Update state
    NOW=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    if [ -f "$STATE_FILE" ]; then
        jq --arg phase "$NEXT" --arg now "$NOW" \
            '.current_phase = $phase | .last_advance = $now | .phase_history += [$phase]' \
            "$STATE_FILE" > "${STATE_FILE}.tmp" && mv "${STATE_FILE}.tmp" "$STATE_FILE"
    else
        jq -n --arg phase "$NEXT" --arg now "$NOW" \
            '{current_phase: $phase, started_at: $now, last_advance: $now, phase_history: [$phase]}' \
            > "$STATE_FILE"
    fi

    echo "✅ Phase: $NEXT"
    echo "   State file: $STATE_FILE"
    echo "   Run: just ce-phase-$NEXT"
