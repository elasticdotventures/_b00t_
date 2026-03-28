---
name: operator
description: Crew dispatch and specialist spin-up for the b00t executive. Decomposes high-level tasks, selects typed specialists from b00t hive or VoltAgent registry, coordinates over ACP, and returns compressed results. Use for multi-domain tasks requiring parallel workstreams, large refactors, or domain expertise outside the executive's loaded context.
when_to_use: >
  When you need a specialist you don't have loaded. When a task spans >2 domains
  (e.g. Rust + TypeScript + infra). When a refactor touches >5 files and needs
  wave execution. When the executive is context-constrained and needs a clean
  sub-context. When k0mmand3r dispatch is the right move.
version: 0.2.0
---

## Operator Pattern

Executive hands off a bounded task. Operator decomposes, selects specialists, coordinates over b00t ACP chat channel, aggregates compressed results, returns to executive.

```
executive → /operator: "<task>"
  operator → decompose → identify required specialist types
           → dispatch specialists (k0mmand3r or Agent tool)
           → mediate over b00t ACP channel
           → aggregate (compressed)
           → return: DONE: <outcome> | <diff-stat> | tests: PASS
```

## Specialist Selection

**Priority order:**
1. `b00t learn <skill>` — check b00t hive datums first
2. `AGENTS/` directory — project-local supplements (`b00t whoami --role=<name>`)
3. `.claude/agents/` — installed Claude Code agents
4. VoltAgent registry — 127+ typed specialists across 10 categories

**VoltAgent categories** (install per-category or individually):
```
voltagent-core      # backend, frontend, API, fullstack
voltagent-lang      # typescript, python, go, rust, java
voltagent-infra     # devops, k8s, terraform, cloud
voltagent-quality   # testing, security, code-review
voltagent-data      # ml-engineering, data-science, llms
voltagent-dx        # refactoring, build-systems, docs
voltagent-meta      # orchestration, workflow, multi-agent
```

Install: `claude --plugin-dir ./agents` or via `/plugin install voltagent-<category>@voltagent`

## Dispatch (k0mmand3r notation)

```bash
# Spin up typed specialist
/k0mmand3r dispatch <model> --role=<type> --skills="<csv>" -- "<subtask>"

# MCP equivalents
b00t_agent_delegate → dispatch
b00t_agent_message  → pass context to running agent
b00t_agent_wait     → sync (timeout 120s default)
b00t_agent_complete → close session
```

## Large Refactor — GSD Wave Execution

For refactors touching >5 files, use wave execution (GSD pattern):

```
1. Operator creates STATE.md: current state, target state, wave plan
2. Wave 0 (analysis): sm0l agents grep/classify files → dependency map
3. Wave N (execution): independent tasks run in parallel with fresh contexts
4. Wave N+1: only after Wave N passes tests
5. Each wave = atomic commits → git bisect friendly
```

STATE.md template:
```markdown
# Refactor State
current: <what exists now>
target: <desired end state>
waves:
  - wave: 0 / status: done / tasks: [analyze deps]
  - wave: 1 / status: pending / tasks: [task-a, task-b]  # independent
  - wave: 2 / status: blocked-by: 1 / tasks: [task-c]
```

## Output Contract to Executive

**Always compressed — never raw specialist output:**
- `DONE: <1-line outcome> | <N files changed> | tests: PASS`
- `FAIL: <agent> <5-line error> | attempted: <cmd> | grok: <suggestion-or-NONE>`

## Bug Capture (RL Loop)

When any specialist fails:
```bash
# Write to .bugs/ (gitignored JSONL)
echo '{...}' >> .bugs/$(date +%Y-%m-%d).jsonl
# Query ontology
b00t grok ask "<failed-cmd>" -t <topic>
# Codify if useful
b00t lfmf datum abstract "<lesson>"
```

The `b00t-bug-capture` PostToolUseFailure hook does this automatically for Bash failures.

## Common Mistakes

- **Passing raw specialist output to executive** — compress first, always
- **Loading specialist skills into executive context** — specialists load their own skills
- **Single-wave execution for dependent tasks** — build the wave dependency map first
- **Skipping STATE.md for large refactors** — context rot kills multi-wave refactors
- **Dispatching frontier-tier for sm0l tasks** — route by cognitive tier (see CLAUDE.md tier table)
- **Not checking b00t datums before dispatching** — `b00t learn <topic>` often avoids the dispatch entirely

## References

- k0mmand3r dispatch syntax: `docs/architecture/k0mmand3r_interface.md`
- Cognitive tier routing: `AGENTS/--role=executive.md`
- Role supplements: `AGENTS/--role=<name>.md`
- VoltAgent subagents: https://github.com/VoltAgent/awesome-claude-code-subagents
- GSD wave execution: https://github.com/gsd-build/get-shit-done
- Bug capture plan: `docs/superpowers/plans/2026-03-29-b00t-bug-capture-rl-loop.md`
