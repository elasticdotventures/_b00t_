# SDD-NNN: {Title}

> **Status:** DRAFT | **Confidence:** 50% | **Iteration:** 1
> **Stage Gates:** 2/5 passed | **Last Updated:** YYYY-MM-DD

---

## 1. Problem Statement

{What are we solving? Why does it matter?}

## 2. Questions

{Open questions that block confidence increase. Answered questions move below.}

### ❓ Unresolved
- Q1: ...
- Q2: ...

### ✅ Resolved (increases confidence +5% each)
- Q: ... → A: ...

## 3. Specification

### 3.1 Interface Contract
```
{Input → Processing → Output with types, formats, edge cases}
```

### 3.2 Integration Points
| Existing Component | How It's Used | Modification Needed |
|---|---|---|
| `component.toml` | ... | ... |

### 3.3 Fallback Chain
{What happens when the primary approach fails?}

### 3.4 Termination Conditions
{When does this stop? Timeout, error threshold, resource limit?}

### 3.5 Debug Levels
| --debug N | Output |
|---|---|
| 0 | Off |
| 1 | {lifecycle} |
| 2 | {verbose} |
| 3 | {trace/rotel} |

### 3.6 Confidence Tracker

| Iteration | Change | Confidence | Rationale |
|---|---|---|---|
| 1 | Initial spec | 50% | Architecture defined, untested |
| 2 | Gate 1-2 pass | 65% | Basic path verified |
| 3 | Gate 3-4 pass | 80% | Edge cases covered |
| 4 | Full loop working | 95% | Production ready |

## 4. Stage Gates

### Gate N: Name ✅🔴⬜
- [ ] Acceptance criterion 1
- [ ] Acceptance criterion 2
- **Validation:** {How to verify pass/fail}

### Gate N+1: Name 🔴
- [ ] ...

## 5. Retrospective
{After each iteration: what worked, what broke, confidence adjustment}

### Iteration N
- **Attempted:** ...
- **Result:** PASS/FAIL
- **Confidence Change:** +/- N%
- **Root Cause (if fail):** ...
- **Spec Updates:** ...

---

### b00t:map v1
# summary: SDD-{NNN} — {one-line}
# tags: tag1, tag2
# tier: ch0nky|frontier
# cmds: b00t <command> --flag=value
# complexity: N
# confidence: NN%
