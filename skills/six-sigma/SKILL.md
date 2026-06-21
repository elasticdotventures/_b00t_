---
name: six-sigma
type: skill
hint: DMAIC — Define, Measure, Analyze, Improve, Control; reduce variance via data-driven decisions
version: 1.0.0
tags: [transferable, dmaic, six-sigma, variance, quality, data-driven, control, defect, sigma]
tier: sm0l
complexity: 4
description: |-
  Six Sigma: reduce defects to 3.4 per million by eliminating variance.
---

## What

Six Sigma aims to reduce defects to 3.4 per million by eliminating variance through the DMAIC cycle. The five phases are: (1) Define — what is the problem? what is the target state? who is affected? (2) Measure — collect baseline data. No guessing, no anecdote. (3) Analyze — find root cause using fishbone, 5-whys, or Pareto analysis. Do not fix symptoms. (4) Improve — implement targeted solution. Pilot first, measure impact. (5) Control — lock in the gain. Add monitoring and set triggers to catch regression.

The key rule: never skip Measure — improving an unmeasured process is theater. In code, DMAIC maps to: define acceptance criteria → measure test coverage → analyze failure modes → improve code → add CI gates.

## When to Use

Apply DMAIC when improving a process, reducing error rates, or addressing quality issues. Use it whenever you need a data-driven approach to process improvement rather than intuition-based changes.

## How

1. Define the problem, target state, and affected parties.
2. Measure baseline data objectively.
3. Analyze root causes using fishbone, 5-whys, or Pareto.
4. Improve with a targeted solution — pilot first, measure impact.
5. Control: lock in gains with monitoring and regression guards.
6. Never skip Measure — improving an unmeasured process is theater.

<!-- b00t:map v1
summary: six-sigma — DMAIC cycle to reduce process variance with data-driven decisions
tags: transferable, dmaic, quality, variance, control
tier: sm0l
cmds: b00t learn six-sigma
complexity: 4
-->
