---
name: socratic
type: skill
hint: Socratic method — question assumptions systematically; ask 'why?' before accepting any premise
version: 1.0.0
tags: [transferable, socratic, questions, assumptions, dialectic, inquiry, challenge, premise, why]
tier: sm0l
complexity: 3
description: |-
  Socratic method: expose false beliefs by disciplined questioning, not assertion.
---

## What

The Socratic method exposes false beliefs through disciplined questioning rather than assertion. Never accept a premise at face value — probe it: "What do you mean by X?" "How do you know that?" "What is the evidence?" "What are the counter-examples?" "What follows if you're wrong?" Apply this to your own reasoning too — an internal Socratic dialogue catches errors before they ship.

In code review, ask "why is this abstraction here?" not "this abstraction is wrong." The goal is not to win — it is to find the true answer through collaborative inquiry. The elenchus technique leads the speaker to contradict themselves, exposing hidden assumptions.

## When to Use

Use the Socratic method whenever you encounter an accepted premise that deserves scrutiny: during code review, requirements gathering, debugging, architecture decisions, or whenever someone says "that's how it's always been done."

## How

1. Identify the premise to be examined.
2. Ask probing questions: what do you mean? how do you know? what is the evidence?
3. Look for contradictions or hidden assumptions in the responses.
4. Apply the same scrutiny to your own reasoning — maintain an internal Socratic dialogue.
5. Beware the anti-pattern: accepting "that's how it's always been done" as a final answer.

<!-- b00t:map v1
summary: socratic method — disciplined questioning to surface and test assumptions
tags: transferable, questions, assumptions, dialectic, inquiry
tier: sm0l
cmds: b00t learn socratic
complexity: 3
-->
