---
name: compound-engineering
description: "8-phase agile workflow — strategy→ideate→brainstorm→plan→work→review→compound→pulse. State machine with FOL-guarded transitions, gh-issues backlog, executable just harness. Inspired by everyinc/compound-engineering-plugin."
version: 1.1.0
platforms: [linux, macos]
---

# Compound Engineering Skill

b00t-native compound engineering workflow with 8 phases, a state machine with FOL-guarded transitions, GitHub Issues backlog integration, and an executable just recipe harness.

## When to Use

When starting a new project, feature, or iteration. The workflow guides from strategy through execution to learning and pulse reporting.

## Phases

| # | Phase | Produces | Command |
|---|-------|----------|---------|
| 1 | Strategy | `STRATEGY.md`, epic issue | `just compound-engineering ce-phase-strategy` |
| 2 | Ideate | `_b00t_/ideation/ranked-ideas.md` | `just compound-engineering ce-phase-ideate` |
| 3 | Brainstorm | `_b00t_/requirements/<feature>.md` | `just compound-engineering ce-phase-brainstorm feature=<name>` |
| 4 | Plan | plan doc + gh issues for tasks | `just compound-engineering ce-phase-plan feature=<name>` |
| 5 | Work | commits, PR | `just compound-engineering ce-phase-work` |
| 6 | Code Review | `_b00t_/reviews/<feature>-review.md` | `just compound-engineering ce-phase-review` |
| 7 | Compound | LFM lessons, grok digests | `just compound-engineering ce-phase-compound` |
| 8 | Product Pulse | `_b00t_/pulse-reports/<date>.md` | `just compound-engineering ce-phase-pulse` |

## Quick Reference

```bash
just compound-engineering status              # Show current phase + backlog
just compound-engineering ce-advance          # Advance to next phase
just compound-engineering ce-phase-strategy   # Execute strategy phase
just compound-engineering ce-phase-brainstorm feature=auth-refactor  # Brainstorm with feature name
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

GitHub Issues with `compound-engineering` label serve as the global backlog. Use `gh issue list --label compound-engineering` to view.

## Pitfalls

### gh label must exist before issue creation
`gh issue create --label compound-engineering` fails silently if the label doesn't exist. Run `gh label create compound-engineering --color 0366d6` first, or create issues without `--label` and add labels post-creation.

### Phase advance is manual — FOL guards are declarative only
The state machine advances via `just compound-engineering ce-advance`. FOL guards (e.g., `guard: ∃ STRATEGY.md`) are documented but NOT programmatically enforced. The agent MUST verify artifacts exist before advancing.

### Tests should gate work → review transition
Don't advance from work until `cargo test` passes on the feature branch. Future: add test-gate to the advance recipe.

### State file requires absolute path in just recipes
Justfile recipes in modules run from unpredictable working directories. Always use `B00T_ROOT="$(git rev-parse --show-toplevel)"` — never relative paths.

## Dogfood Example

See `references/australian-tax-capability-dogfood.md` for a complete 8-phase walkthrough: Australian tax legislation ingestion pipeline, 5 gh issues (#504-508), 4 passing tests, full cycle strategy→pulse→strategy.

## Datum

Full configuration: `_b00t_/compound-engineering.datum.toml`
Harness recipes: `_b00t_/compound-engineering.just`
