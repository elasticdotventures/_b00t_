---
name: certainty-grade
description: Apply HIGH/MEDIUM/LOW certainty grading to all agent findings and recommendations. Use to gate human review, auto-fix, or autonomous action.
disable-model-invocation: false
---

# Certainty Grade Framework

All agent findings MUST include a certainty grade. Grade determines action path.

## Grades

| Grade | Basis | Action |
|-------|-------|--------|
| `HIGH` | Deterministic: regex match, static check, type error, compiler output | Auto-fix permitted |
| `MEDIUM` | Heuristic: code ratio analysis, pattern frequency, structural smell | Flag for review |
| `LOW` | AI judgment: semantic meaning, intent, contextual inference | Human gate required |

## Steps

1. Identify the finding type. # output: finding_type (deterministic|heuristic|semantic)
2. Assign grade:
   - `HIGH` if finding_type == deterministic (provable without AI)
   - `MEDIUM` if finding_type == heuristic (pattern-based, probabilistic)
   - `LOW` if finding_type == semantic (requires AI reasoning)
   # output: certainty (HIGH|MEDIUM|LOW)
3. Attach grade to finding in output. Format: `[HIGH] <finding>`, `[MEDIUM] <finding>`, `[LOW] <finding>`
4. For batched findings, group by certainty. # output: grouped_findings

## Action Gates

```
HIGH  → auto-apply fix if safe, log result
MEDIUM → surface to user, suggest fix, await confirmation
LOW   → block action, require explicit human approval, explain reasoning
```

## Output Format

```
[HIGH]   console.log() at src/foo.ts:42 — remove (debug artifact)
[MEDIUM] function doThing() lacks error handling — review
[LOW]    variable naming may be misleading given domain context — human review
```

## Integration

Used by: `/next-task`, `/code-quality`, all agents that produce findings.
b00t MUST grade all MCP tool outputs before acting on them.
