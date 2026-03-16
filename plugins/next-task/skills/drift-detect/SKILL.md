---
name: drift-detect
description: Detect documentation-code drift. Deterministic collection (grep/AST) feeds a single LLM semantic analysis call. Reports mismatches with certainty grades.
disable-model-invocation: false
---

# /drift-detect — Documentation-Code Alignment

Principle: code does deterministic collection; AI does one semantic analysis pass.
Avoids multi-agent token waste — single well-prompted synthesis call.

## Steps

1. Identify scope: README, CLAUDE.md, inline docstrings, `docs/`, `.tomllm` comments. # output: doc_files[]
2. Identify code: public API surfaces, CLI commands, MCP tools, data schemas. # output: code_surfaces[]
3. COLLECT deterministic facts from code (no AI). # output: code_facts{}
4. COLLECT documented claims from docs (no AI). # output: doc_claims{}
5. DIFF: find claims in docs not evidenced in code_facts. # output: raw_drift[]
6. Single LLM call: semantic analysis of raw_drift to grade and explain. # output: graded_drift[]
7. Group by certainty-grade; present report.

## Phase 1: Deterministic Collection (Code → Facts)

Extract without AI:
```
CLI commands     → grep `#\[command\]` / `clap::Command` / argparse / click decorators
MCP tools        → grep `#[tool]` / tool schema definitions
Public functions → AST: all `pub fn`, `export function`, `def` without leading `_`
Config keys      → grep TOML/JSON schema keys in datum files
Environment vars → grep `env!()`, `os.getenv`, `process.env.`
API endpoints    → grep route decorators (`@app.get`, `#[get]`, `router.get`)
```

## Phase 2: Deterministic Collection (Docs → Claims)

Extract without AI:
```
README commands  → grep ` ``` ` blocks containing CLI invocations
Documented envs  → grep `ENV_VAR_NAME` pattern in .md files
Skill triggers   → grep activation phrases in SKILL.md files
Datum fields     → parse TOML keys from .tomllm files
```

## Phase 3: Single LLM Semantic Analysis

Pass code_facts and doc_claims to one analysis call:
```
Prompt: "Given these documented claims and these code facts, identify:
1. Claims in docs not supported by code (doc leads code — stale)
2. Code surfaces not mentioned in docs (code leads docs — undocumented)
3. Semantic mismatches where names/behavior differ
Grade each finding HIGH/MEDIUM/LOW certainty."
```

## Drift Categories

| Type | Description | Grade |
|------|-------------|-------|
| `stale-doc` | Docs describe removed/changed feature | HIGH if name mismatch, MEDIUM if semantic |
| `undocumented` | Code exists, no docs | MEDIUM |
| `mismatch` | Docs and code describe different behavior | LOW (AI judgment) |
| `version-skew` | Docs reference old version numbers/flags | HIGH |

## Flags

```
/drift-detect                  # Full repo scan
/drift-detect --path <dir>     # Scope to directory
/drift-detect --focus docs     # Only stale-doc findings
/drift-detect --focus code     # Only undocumented findings
/drift-detect --fix            # Auto-stub missing docs (MEDIUM certainty only)
```

## Output

```
drift-detect report: .b00t/
━━━━━━━━━━━━━━━━━━━━━━━━━━━
[HIGH]   2 stale documentation findings
  ✗ README references `b00t run` — command removed in cli v0.8
  ✗ CLAUDE.md documents `--model-size` flag — not in current clap schema

[MEDIUM] 3 undocumented code surfaces
  ⚠ `b00t hive activate` — no README entry
  ⚠ MCP tool `b00t_agent_vote_create` — not in docs
  ⚠ env var `VLLM_DTYPE` — not in .env.example

[LOW]    1 semantic mismatch
  ? SKILL.md says deslop "auto-removes"; code-quality rules say "surfaces for review" — reconcile

Total: 6 findings | Auto-fixable stubs: 3 | Stale to remove: 2 | Human review: 1
```

## Integration

Runs at REVIEW phase of `/next-task`.
Invoked standalone post-implementation to check docs kept current.
