# b00t Review Ecosystem: Skills Gap Analysis
# Generated: 2026-06-21 | Branch: task/dogfood-mece-gaps

## Existing Review Skills (coverage map)

| Skill | Capability | Maturity | Gaps |
|-------|-----------|----------|------|
| github-code-review | Standard checklist: critic/warn/suggest/looks-good | Production | Single-lens, procedural only |
| multi-framework-code-review | MECE+TRIZ+Eureka triangulation | v1.0 (NEW) | No dogfood history, no metrics yet |
| requesting-code-review | Pre-commit security scan + auto-fix | Mature | Pre-commit only, not PR review |
| systematic-debugging | 4-phase root cause + TRIZ+IDEO gap analysis | Mature | Bug-focused, not review |
| subagent-driven-development | 2-stage spec+quality review | Mature | Code creation focused, not review |
| executive-operator | Bouncer adversarial review pattern | Mature | Orchestration, not review content |
| governance-epoch2 | Eisenhower gate routing | Mature | Gate system, not review method |
| writing-plans | TRIZ-based codebase analysis | Mature | Planning, not review |

## Gaps Identified (priority-ordered by dogfood value)

### GAP-1: No Datum Review Specialization 🔴
**Problem:** `write-guard.gate.toml` review exposed that datum files (.toml datums,
gates, skills) have different review criteria than code. Missing tail-map, wrong
section structure, missing required fields — these are datum-specific issues that
a code review lens misses.
**Solution:** `datum-review` skill — applies MECE+TRIZ+Eureka specifically to b00t
datum format. Validates: tail-map completeness, section structure, schema
compliance, forward-reference validity, blessing requirements.
**Dogfood:** Review all 12 datum files in `_b00t_/` for structural compliance.
**Priority:** CRITICAL — found a real bug (missing audit section) on first use.

### GAP-2: No Review Quality Tracking 📊
**Problem:** No way to measure if multi-framework review is actually better than
standard checklist. No longitudinal tracking of review quality, false positive
rate, or missed bugs later found in production.
**Solution:** `review-quality-tracker` skill — logs review metrics per PR, computes
rolling averages, flags declining quality, correlates framework contributions
with downstream bug count.
**Dogfood:** Track the next 10 reviews and compare multi-framework vs standard.
**Priority:** HIGH — without metrics, can't prove the new process is better.

### GAP-3: No Gate Schema Validator 🛡️
**Problem:** `write-guard.gate.toml` had no audit section, no version field, no
author field. The `zellij-interaction.gate.toml` has all of these. No structural
guarantee that new gates follow the convention.
**Solution:** `_b00t_/schema/gate.schema.toml` + `validate-gate` just recipe.
Defines required vs optional sections. Run on every gate datum commit.
**Dogfood:** Validate all 2 existing gates, fail the write-guard for missing audit.
**Priority:** HIGH — prevents the class of bug found in this review.

### GAP-4: No PR Risk Scoring Before Review 🎯
**Problem:** Review depth (standard vs multi-framework vs parallel sub-agents)
is chosen arbitrarily, not based on PR risk. A 3-line typo fix gets the same
scrutiny as a 500-line auth refactor.
**Solution:** `pr-risk-scorer` skill — scores PRs on: author experience, file
hotness (churn rate), change size, test coverage delta, security surface area.
Routes to appropriate review depth tier.
**Dogfood:** Score the next 5 PRs, verify routing decisions against manual judgment.
**Priority:** MEDIUM — optimizes review time allocation.

### GAP-5: No Cross-PR Pattern Detection 🔍
**Problem:** Individual reviews catch file-level issues, but no mechanism detects
patterns across multiple PRs: "3 PRs this week all missed audit sections," or
"same error-handling gap appears in 4 different services."
**Solution:** `cross-pr-pattern-detector` skill — aggregates findings across PRs,
surfaces recurring anti-patterns, suggests ecosystem-wide fixes.
**Dogfood:** After 10 reviews, run pattern detection and report.
**Priority:** MEDIUM — high value but requires review history accumulation first.

### GAP-6: No Review QA Feedback Loop 🔄
**Problem:** When a bug is found in production that a review missed, there's no
structured feedback mechanism to improve the review process. The reviewer never
learns about their miss.
**Solution:** `review-qa-feedback` skill — production bugs auto-create review
miss tasks, linked to the original PR review. Reviewer gets notified. Review
checklist updated based on misses.
**Dogfood:** Requires production bug tracking — not yet available.
**Priority:** LOW — depends on production monitoring infrastructure.

## Dogfood Action Plan

### Immediate (this session)
1. ✅ Create `multi-framework-code-review` skill + datum + justfile
2. ✅ Dogfood on `write-guard.gate.toml` — found 6 issues, 1 critical
3. ✅ Promote critical finding to task queue
4. Create `datum-review` skill (addresses GAP-1)
5. Validate all gates against proposed schema (addresses GAP-3)

### Short-term (next 2-3 sessions)
6. Create `review-quality-tracker` skill (addresses GAP-2)
7. Track first 5 multi-framework reviews, compute baseline metrics
8. Create `pr-risk-scorer` skill (addresses GAP-4)
9. Dogfood risk scoring on 5 real PRs

### Medium-term (epoch-level)
10. Accumulate 10+ reviews for cross-PR pattern detection (GAP-5)
11. Wire up governance Eisenhower gate to auto-route review findings
12. Connect review quality tracker to cake economy — better reviews = more cake
