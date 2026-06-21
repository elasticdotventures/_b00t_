---
name: compound-engineering
description: "8-phase agile workflow — strategy→ideate→brainstorm→plan→work→review→compound→pulse. State machine with FOL-guarded transitions, gh-issues backlog, executable just harness. Inspired by everyinc/compound-engineering-plugin."
version: 1.0.0
platforms: [linux, macos]
---

# Compound Engineering Skill

b00t-native compound engineering workflow with 8 phases, a state machine with FOL-guarded transitions, GitHub Issues backlog integration, and an executable just recipe harness.

## When to Use

When starting a new project, feature, or iteration. The workflow guides from strategy through execution to learning and pulse reporting.

## Phases

| # | Phase | Produces | Command |
|---|-------|----------|---------|
| 1 | Strategy | `STRATEGY.md`, epic issue | `just ce-phase-strategy` |
| 2 | Ideate | `_b00t_/ideation/ranked-ideas.md` | `just ce-phase-ideate` |
| 3 | Brainstorm | `_b00t_/requirements/<feature>.md` | `just ce-phase-brainstorm feature=<name>` |
| 4 | Plan | plan doc + gh issues for tasks | `just ce-phase-plan feature=<name>` |
| 5 | Work | commits, PR | `just ce-phase-work` |
| 6 | Code Review | `_b00t_/reviews/<feature>-review.md` | `just ce-phase-review` |
| 7 | Compound | LFM lessons, grok digests | `just ce-phase-compound` |
| 8 | Product Pulse | `_b00t_/pulse-reports/<date>.md` | `just ce-phase-pulse` |

## Quick Reference

```bash
just ce-status                                    # Show current phase + backlog
just ce-advance                                   # Advance to next phase
just ce-phase-strategy                            # Execute current phase
just ce-phase-brainstorm feature=auth-refactor    # Brainstorm with feature name
```

## State Machine

State tracked in `.b00t/compound-engineering-state.json`. Each transition is FOL-guarded:

```
strategy → ideate     guard: ∃ STRATEGY.md
ideate → brainstorm   guard: ∃ ranked-ideas.md
brainstorm → plan     guard: ∃ requirements
plan → work           guard: ∃ plan ∧ ∃ tasks
work → review         guard: ∃ open PR
review → compound     guard: review approved
compound → pulse      guard: learnings documented
pulse → strategy      guard: pulse report exists
```

## Backlog Integration

GitHub Issues with `compound-engineering` label serve as the global backlog.
Issues are auto-labeled per phase. Use `gh issue list --label compound-engineering` to view.

## Datum

Full configuration: `_b00t_/compound-engineering.datum.toml`
Harness recipes: `_b00t_/compound-engineering.just`
