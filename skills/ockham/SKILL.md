---
name: ockham
type: skill
hint: Occam's razor — prefer the simplest explanation that fits the evidence; complexity is a liability
version: 1.0.0
tags: [transferable, occam, ockham, razor, simplicity, parsimony, complexity, explanation, minimal]
tier: sm0l
complexity: 2
description: |-
  Occam's Razor (William of Ockham): among competing explanations, prefer the one with fewest assumptions.
---

## What

Occam's Razor (William of Ockham) states: among competing explanations that fit the evidence equally well, prefer the one with the fewest assumptions. "Entities must not be multiplied beyond necessity" (entia non sunt multiplicanda praeter necessitatem). In reasoning, if two hypotheses fit the evidence equally well, choose the simpler one. In code, if two implementations are correct, choose the one with fewer moving parts.

The complexity test: "does removing this component break anything?" If no, remove it. In debugging, the simple explanation (misconfiguration, off-by-one, nil pointer) is almost always right. Resist exotic multi-system conspiracy theories when a single mundane cause fits all evidence. Caveat: Occam's razor selects between equally fitted explanations — do not use it to reject a complex truth when the evidence actually requires it.

## When to Use

Apply Occam's Razor whenever choosing between competing explanations, designs, or implementations. Use it to resist over-engineering and speculative abstraction layers added "just in case."

## How

1. List competing explanations that fit the evidence equally well.
2. Count the assumptions each requires.
3. Prefer the explanation with fewest assumptions.
4. In code, apply the complexity test: would removing this component break anything?
5. Beware over-engineering and adding abstraction layers "just in case."

<!-- b00t:map v1
summary: ockham — occam's razor: prefer fewest assumptions; complexity is a liability
tags: transferable, simplicity, parsimony, occam, razor, minimal
tier: sm0l
cmds: b00t learn ockham
complexity: 2
-->
