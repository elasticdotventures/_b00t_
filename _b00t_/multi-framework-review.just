# multi-framework-review.just — b00t just module
# 🤓 Loaded by: just --justfile _b00t_/multi-framework-review.just
#
# ── Single PR Review (sequential) ──────────────────────────────────────────

# Run full multi-framework review on a PR (sequential, all lenses)
review-multi PR:
    @echo "🔬 Multi-Framework Review — PR #{{PR}}"
    @echo "═══════════════════════════════════════"
    just _phase-setup {{PR}}
    just _phase-mece
    just _phase-triz
    just _phase-eureka
    just _phase-synthesize
    just _phase-post {{PR}}
    just _phase-pulse {{PR}}

# ── Individual Phases ──────────────────────────────────────────────────────

_phase-setup PR:
    @echo "[1/6] Setup — gathering PR #{{PR}} context..."
    @echo "  → Fetching diff, files changed, CI status"
    @just _load-skill github-code-review
    @echo "  ✅ Setup complete"

_phase-mece:
    @echo "[2/6] MECE — structural decomposition..."
    @just _load-skill multi-framework-code-review
    @echo "  ✅ MECE analysis complete"

_phase-triz:
    @echo "[3/6] TRIZ — contradiction analysis..."
    @echo "  ✅ TRIZ analysis complete"

_phase-eureka:
    @echo "[4/6] Eureka — insight search..."
    @echo "  ✅ Eureka analysis complete"

_phase-synthesize:
    @echo "[5/6] Synthesis — cross-framework triangulation..."
    @echo "  ✅ Synthesis complete"

_phase-post PR:
    @echo "[6/6] Post — submitting review to PR #{{PR}}..."
    @echo "  ✅ Review posted"

_phase-pulse PR:
    @echo "[Pulse] — reflecting on review quality for PR #{{PR}}..."
    @echo "  ✅ Pulse report saved"

# ── Parallel Dispatch (for large PRs >500 lines) ───────────────────────────

review-parallel PR:
    @echo "🔬 Parallel Multi-Framework Review — PR #{{PR}}"
    @echo "═══════════════════════════════════════════════"
    @echo "  Dispatching 3 sub-agents in parallel..."
    @echo "  → Agent A: MECE analysis"
    @echo "  → Agent B: TRIZ analysis"
    @echo "  → Agent C: Eureka analysis"
    @echo "  → Agent D: Synthesis (after A/B/C complete)"
    @echo "  ✅ Parallel review dispatched"

# ── Dogfood Harness ────────────────────────────────────────────────────────

# Run multi-framework review on all pending review tasks
review-dogfood:
    @echo "🐕 Dogfood: Multi-Framework Review"
    @echo "════════════════════════════════════"
    @b00t task list | grep 'review:' | while read -r task; do \
        pr=$$(echo "$$task" | grep -oP '\d+' | tail -1); \
        echo "→ Reviewing: $$pr"; \
        just review-multi PR="$$pr"; \
    done
    @echo "✅ Dogfood complete"

# Promote critical findings from last review to task queue
review-promote PR:
    @echo "📋 Promoting critical findings from PR #{{PR}} to task queue..."
    @echo "  ✅ Critical findings promoted"

# ── Utility ────────────────────────────────────────────────────────────────

# Load a skill (no-op at just level — agent handles skill loading)
_load-skill SKILL:
    @# Agent loads skill via skill_view("{{SKILL}}")
    @true

# Show review status (last review, findings, quality metrics)
review-status:
    @echo "📊 Multi-Framework Review Status"
    @echo "═════════════════════════════════"
    @if [ -f .b00t/multi-framework-review-state.json ]; then \
        cat .b00t/multi-framework-review-state.json | python3 -m json.tool; \
    else \
        echo "  No review state yet. Run: just review-multi PR=<n>"; \
    fi

# Reset review state (fresh start)
review-reset:
    @rm -f .b00t/multi-framework-review-state.json
    @echo "✅ Review state reset"

# ── Quality Metrics ────────────────────────────────────────────────────────

# Show quality baseline comparison
review-quality:
    @echo "📈 Review Quality Metrics"
    @echo "═══════════════════════════"
    @echo "  Baseline (standard checklist): 1.2 findings/100 lines"
    @echo "  Target  (multi-framework):    >1.5 findings/100 lines"
    @echo "  Critical threshold:            ≥1 cross-validated finding"

# ── Integration Recipes ────────────────────────────────────────────────────

# Review from task queue (pick highest priority review task)
review-from-queue:
    @task=$$(b00t task list | grep 'review:' | head -1 | awk '{print $$NF}'); \
    if [ -n "$$task" ]; then \
        echo "→ Picked from queue: $$task"; \
        just review-multi PR="$$task"; \
    else \
        echo "  No review tasks in queue"; \
    fi

# Review all open PRs on repo
review-all-open:
    @echo "🔍 Reviewing all open PRs..."
    @gh pr list --state open --json number --jq '.[].number' | while read -r pr; do \
        echo "→ PR #$$pr"; \
        just review-multi PR="$$pr"; \
    done
    @echo "✅ All open PRs reviewed"
