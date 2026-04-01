---
name: code-quality
description: Validate code quality using certainty-graded rules. Detect AI artifacts, anti-patterns, and b00t violations. Reports auto-fixable vs review-required findings.
disable-model-invocation: false
---

# Code Quality Rules

Uses certainty-grade framework. Runs deterministic checks first (code), AI analysis second (reasoning).

## Steps

1. Run HIGH certainty checks (no AI needed). # output: high_findings[]
2. Run MEDIUM certainty checks (heuristics). # output: medium_findings[]
3. Run LOW certainty checks (AI judgment). # output: low_findings[]
4. Apply certainty-grade to each finding.
5. Group: auto-fixable (HIGH) vs needs-review (MEDIUM) vs human-gate (LOW).
6. Report summary with counts.

## HIGH Certainty Rules (Deterministic)

**AI Artifact Detection:**
- `console.log(` / `print(` / `println!(` in non-test code
- `TODO`, `FIXME`, `HACK`, `XXX` comments without issue reference
- `debugger;` statements
- Placeholder values: `"TODO"`, `"FIXME"`, `"placeholder"`, `"example.com"`
- Commented-out code blocks (3+ consecutive commented lines)

**b00t Violations:**
- Direct `pip install` (MUST use `uv pip install`)
- `docker run` without podman (MUST use `podman`)
- Raw templates read without `b00t learn`
- Hardcoded API keys or secrets (entropy check)

**Language-Specific:**
- `unwrap()` in Rust production code (outside tests)
- `except: pass` in Python
- `any` type in TypeScript without justification comment

## MEDIUM Certainty Rules (Heuristic)

**Documentation Ratios:**
- Functions >20 lines without docstring/comments
- Public API without documentation
- File doc ratio < 10% (comment lines / total lines)

**Code Smell Patterns:**
- Functions >50 lines (single responsibility violation)
- Nesting depth >4 (cognitive complexity)
- Duplicate code blocks (>10 identical lines across files)
- Magic numbers without named constants

**b00t Alignment:**
- Missing `# 🤓` tribal knowledge on non-idiomatic patterns
- Missing error handling at system boundaries
- Tests that mock internal code (MUST mock at system boundary only)

## LOW Certainty Rules (AI Judgment)

- Variable/function naming clarity relative to domain
- Algorithm choice vs alternative approaches
- Missing edge case handling (contextual)
- Architectural concerns (tight coupling, abstraction leaks)

## Output Format

```
Code Quality Report: <file/scope>
━━━━━━━━━━━━━━━━━━━━━━━━━━━━
[HIGH]   3 findings — auto-fixable
  ✗ console.log() at src/app.ts:12
  ✗ TODO without issue at lib/utils.rs:88
  ✗ unwrap() in production at src/main.rs:45

[MEDIUM] 2 findings — needs review
  ⚠ process_data() at src/etl.py:100 — 67 lines, consider split
  ⚠ public fn without docs at src/api.rs:23

[LOW]    1 finding — human gate
  ? naming: `handle_thing()` may be ambiguous in payment context

Auto-fix available: 3 | Review required: 2 | Human approval: 1
```

## Integration

Invoke via: `/next-task` (pre-merge gate), inline during implementation.
All fixes apply certainty-grade before executing.
