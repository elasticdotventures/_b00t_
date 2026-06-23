---
name: five-whys
type: skill
hint: Root cause analysis — ask 'why?' 5 times; stops at systemic cause not symptom
version: 1.0.0
tags: [transferable, five-whys, root-cause, rca, why, symptom, systemic, cause, analysis]
tier: sm0l
complexity: 2
description: |-
  Five Whys (Toyota): iterate "why?" until you reach a systemic/actionable root cause.
---

## What

Five Whys (originating from Toyota) is a root cause analysis technique where you iterate "why?" until you reach a systemic, actionable root cause. Each answer becomes the subject of the next "why?" — for example: Bug in prod → why? → test missed it → why? → no test for this path → why? → developer didn't know the path existed → why? → no onboarding doc → why? → we never wrote it. Root cause: missing documentation process.

Fix the root, not the symptom — patching the bug without fixing the process guarantees recurrence. The number 5 is a heuristic, not a law. Stop when you reach something you can actually change. Watch for multiple branches — one "why" may have two answers, diagram both.

## When to Use

Use Five Whys during incident post-mortems, debugging sessions, process improvement, and any time you need to find the underlying systemic cause of a problem rather than treating surface symptoms.

## How

1. State the problem clearly.
2. Ask "why did this happen?" and write down the answer.
3. For each answer, ask "why?" again.
4. Repeat until you reach a systemic cause you can actually change.
5. Document the chain and implement corrective actions at the root level, not the symptom level.
6. Beware stopping at the first visible cause ("it was a bug") and calling it done.

<!-- b00t:map v1
summary: five-whys — iterate why? to reach systemic root cause not surface symptom
tags: transferable, root-cause, rca, why, systemic, analysis
tier: sm0l
cmds: b00t learn five-whys
complexity: 2
-->
