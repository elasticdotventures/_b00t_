---
name: bayesian
type: skill
hint: Bayesian updating — priors + evidence = posterior; update beliefs proportionally to evidence strength
version: 1.0.0
tags: [transferable, bayesian, prior, posterior, evidence, probability, update, inference, belief]
tier: ch0nky
complexity: 5
description: |-
  Bayesian reasoning: beliefs are probabilities; evidence updates them proportionally.
---

## What

Bayesian reasoning treats beliefs as probabilities that are updated proportionally as new evidence arrives. The core formula — P(H|E) = P(E|H) × P(H) / P(E) — in plain terms means: posterior = likelihood × prior / normalizer. You start with a prior (what you believed before), observe evidence, and update. Strong evidence produces large updates; weak evidence produces small updates; confirming noise produces tiny updates.

The key insight is that you must have a prior. "I have no opinion" is still a 50/50 prior — state it explicitly. A common error is base rate neglect — ignoring P(H) and overweighting P(E|H). In debugging, ask "what is my prior that this module has a bug vs that module?" and start there. In decision-making, each new data point is an update, not a verdict.

## When to Use

Apply Bayesian reasoning whenever you need to update a belief based on new information, assess probabilities under uncertainty, or avoid base rate neglect. Use it in debugging triage, risk assessment, and any situation where evidence quality varies.

## How

1. State your prior belief explicitly as a probability.
2. Observe new evidence and assess its strength/likelihood.
3. Update: posterior = (likelihood × prior) / normalizer.
4. Repeat with each new piece of evidence, treating each as an incremental update, not a final verdict.
5. Beware the anti-pattern: treating every piece of new information as equally significant regardless of quality.

<!-- b00t:map v1
summary: bayesian — update beliefs proportionally to evidence using prior + likelihood
tags: transferable, probability, inference, evidence, prior, posterior
tier: ch0nky
cmds: b00t learn bayesian
complexity: 5
-->
