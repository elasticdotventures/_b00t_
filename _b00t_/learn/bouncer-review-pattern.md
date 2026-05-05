---
author: b00t hive
topic: bouncer-review-pattern
version: 1.0.0
---

# Bouncer-Review Pattern Workflow

Two-agent peer review pattern: Agent A develops, Agent B reviews critically.

## Workflow

1. **Operator** writes `context.md` with task description
2. **Agent A** (developer): research + write tests + stubs
3. **Ralph loop**: small steps, pass >= 1 test, then all
4. Each pass: context reset + reload. Operator untouched
5. A signals ready (`a_ready=true` in state file)
6. **Agent B** (reviewer): same prompt, critical review, deeper ecosystem awareness
7. B reviews → A addresses → B validates → repeat
8. Stop: both happy OR operator override

## State File

Persistent workflow state at `.bouncer-state.toml`:

```toml
[meta]
created = "2026-05-05T01:00:00Z"
context_file = "context.md"

[state]
phase = "research"        # research|development|review|approved|escalated
turn = 0                   # development loop counter
max_turns = 5              # max iterations before escalation
agent_a = "developer"
agent_b = "reviewer"

[results]
agent_a_ready = false      # A signals done
agent_b_approved = false   # B approves
agent_a_fixes = 0          # fix count
review_round = 0           # review cycle counter
```

Phase transitions:
- `research` → `development` (initial setup done)
- `development` → `review` (a_ready=true)
- `review` → `development` (if B rejects, a_ready=false for fixes)
- `review` → `approved` (B approves)
- `development` → `escalated` (max_turns hit)
- `review` → `escalated` (3+ review rounds)

## Usage

```bash
# Start or resume the workflow
b00t script run bouncer-review context.md

# Check state
cat .bouncer-state.toml

# Manual overrides (operator)
# Set agent_a_ready = true in .bouncer-state.toml to force review
# Set agent_b_approved = true to approve
# Delete .bouncer-state.toml to restart fresh
```

## Agent B Review Criteria

Agent B checks (critical reviewer hat):
1. Test coverage: Are edge cases covered?
2. Code quality: Idiomatic, no duplication, proper error handling
3. Ecosystem awareness: Does this duplicate existing functionality? Is there a library?
4. Security: Injection vectors, credential exposure, bounds
5. Performance: Unnecessary allocations, N+1 queries
6. Maintainability: Clear naming, documentation, single responsibility

## Integration

- Backed by `_b00t_/scripts/bouncer-review.rhai` Rhai state machine
- Uses `b00t agent delegate` for Agent A/B dispatch
- State file is git-ignored (transient per session)
- Works with `b00t up --role developer --max-iter N` for Ralph loop
