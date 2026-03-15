---
name: deslop
description: Remove AI-generated artifacts from code. Three-phase certainty-graded cleanup. Use after any AI implementation session or before PR creation.
disable-model-invocation: false
---

# /deslop — AI Artifact Removal

Three-phase pipeline: deterministic first, heuristic second, AI last. Never removes without certainty grade.

## Steps

1. Identify scope (file, directory, or git diff). # output: scope_paths[]
2. Run Phase 1 (HIGH). # output: high_findings[]
3. Run Phase 2 (MEDIUM). # output: medium_findings[]
4. Run Phase 3 (LOW). # output: low_findings[]
5. Apply certainty-grade to all findings.
6. AUTO-REMOVE all HIGH findings after displaying list. ← confirmation optional via `--dry-run`
7. Surface MEDIUM findings for Operator review. ← **HUMAN GATE**
8. Present LOW findings with reasoning; apply only if Operator confirms.
9. Commit cleanup: `git commit -m "chore: deslop AI artifacts"` # output: commit_sha

## Phase 1: HIGH Certainty (Regex — auto-remove)

```
console\.log\(          → JS/TS debug output
print\(                 → Python debug (non-test files)
println!\(              → Rust debug (non-test files)
debugger;               → JS/TS breakpoint
\bpdb\.set_trace\(\)    → Python debugger
# TODO(?!.*#[0-9]+)     → TODO without issue reference
# FIXME(?!.*#[0-9]+)    → FIXME without issue reference
# HACK                  → Hack marker
# XXX                   → Unresolved marker
"placeholder"           → Literal placeholder string
"example\.com"          → Placeholder domain in non-config files
\.unwrap\(\)            → Rust panic-on-none (outside #[cfg(test)])
except:\s*pass          → Python silent exception swallow
```

Respect file-level `# deslop:ignore` annotation to skip a file.
Skip `tests/`, `*_test.*`, `*.spec.*`, `*_spec.*`.

## Phase 2: MEDIUM Certainty (Heuristic — surface for review)

- Commented-out code blocks: 3+ consecutive `//` or `#` lines that parse as valid code
- Verbosity patterns: function docstrings longer than the function body
- Over-explanation comments: comments that restate what the next line obviously does
  ex: `# increment i by 1` before `i += 1`
- Duplicate log statements: same message logged in consecutive lines
- Dead imports: `import X` where `X` never appears in file body

## Phase 3: LOW Certainty (AI judgment — human gate)

- Placeholder logic: functions that only `return None` / `pass` / `throw new Error("not implemented")`
- Scaffolding that was never filled in
- Overengineered abstractions added speculatively ("we might need this later")
- Excessive type casting or conversion that AI commonly adds defensively

## Flags

```
/deslop                    # Run on git diff (staged + unstaged)
/deslop --path <path>      # Specific file or directory
/deslop --dry-run          # Show findings only, no changes
/deslop --phase <1|2|3>    # Run specific phase only
/deslop --skip-low         # Skip Phase 3 (faster)
```

## Output

```
deslop report: src/
━━━━━━━━━━━━━━━
[HIGH]   removed 4 artifacts automatically
  ✓ console.log() at src/app.ts:12
  ✓ TODO (no issue) at src/utils.ts:88
  ✓ debugger; at src/debug.ts:3
  ✓ .unwrap() at src/main.rs:45

[MEDIUM] 2 findings — awaiting review
  ⚠ commented-out block at src/parser.ts:55-62
  ⚠ dead import `useState` at src/foo.tsx:2

[LOW]    1 finding — human gate
  ? scaffold fn `processPayment()` returns None — was this implemented?
```

## Integration

Runs automatically at IMPLEMENT phase of `/next-task`.
Can be invoked standalone pre-PR.
