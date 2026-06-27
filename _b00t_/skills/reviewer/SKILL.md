---
name: reviewer
description: |
  Canonical b00t reviewer capability — adversarial multi-framework code/datum review.
  Shared across all harnesses (Claude Code, opencode, Hermes, b00t-cli).
  Loaded via: b00t learn reviewer, or by harness-specific role supplements.
version: 1.0.0
allowed-tools: All
capability_type: skill
harness_targets: [claude-code, opencode, hermes, b00t-cli]
---

## What This Skill Does

Canonical reviewer capability for the b00t hive. Applies MECE structural decomposition,
TRIZ contradiction analysis, and Eureka insight search to code, datums, and system configs.
Produces machine-parseable verdicts (APPROVE/REQUEST_CHANGES) suitable for pre-commit gates,
PR review, and governance enforcement.

## Canonical Across Harnesses

| Harness | How It Loads | File |
|---------|-------------|------|
| b00t-cli | `b00t learn reviewer` | This SKILL.md |
| Claude Code | `b00t whoami --role=reviewer` | AGENTS/--role=reviewer.md (thin wrapper) |
| opencode | Native skill load | This SKILL.md |
| Hermes | Skill bundle | references/ this directory |

## Review Protocol

1. **system-normal()** — precriteria gate (git stash, conflicts, branch, submodules)
2. **MECE** — structural decomposition, boundary issues
3. **TRIZ** — contradiction analysis, unnecessary trade-offs
4. **Eureka** — pattern detection, simplification opportunities
5. **Synthesize** — cross-framework triangulation
6. **Verdict** — APPROVE or REQUEST_CHANGES

## Output Contract (STRICT — machine parsed)

```
VERDICT: APPROVE
```
or
```
VERDICT: REQUEST_CHANGES
```

Preceded by brief justification (1-3 lines). No markdown after VERDICT.

## RHAI Scripts

| Script | Purpose |
|--------|---------|
| system-normal.rhai | Precriteria gate — git stash, conflicts, branch, submodules |
| autoexec.rhai | Autonomous review loop — poll queue → dispatch → post → repeat |

## Dependencies

- `_b00t_/reviewer.role.toml` — role datum with capabilities, sub-agents, compliance rules
- `_b00t_/multi-framework-review.datum.toml` — 7-phase state machine
- `_b00t_/datum-review.datum.toml` — datum-specific review skill
- `_b00t_/pr-risk-scorer.datum.toml` — PR risk tier routing
- b00t-cli installed (for `b00t script run`, `b00t task list`, `b00t agent delegate`)
- RHAI runtime (via `b00t script eval`)

## Compliance Rules

- Always run system-normal() before review operations
- Guard violations are ALWAYS REQUEST_CHANGES
- Scope drift is WARNING, not FAIL
- Never output markdown after VERDICT line
- Report phygital-twin status per session

## Version

1.0.0 — Initial canonical reviewer skill. Replaces scattered role supplements with
single source of truth. All harnesses reference this canonical capability.
