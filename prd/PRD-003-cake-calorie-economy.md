# PRD-EPOCH-3: Cake-Calorie Governance Economy

## Vision
Replace static gate results with a dynamic, market-driven governance economy where agents earn, spend, and trade 🍰 Cake (cognitive calories). Every action has a cost. Every success has a reward. Dead agents get recycled.

## Core Mechanism

```
Agent Action → GovernanceGate::check() → 
  GateResult::Allow   → deduct calories, execute, report result
  GateResult::Deny    → deduct calories, log failure, escalate
  GateResult::Hook    → freeze calorie burn, register hook, go dormant
                       ↓ hook fires
                       → deduct wake-up cost, restore context, continue
```

## Key Concepts

**Calories** — Unit of cognitive work. Every token generated, every function called, every file read burns calories. An agent with 0 calories is dead (`☠️`).

**Cake** — Unit of value. Earned by completing missions, impressing captains, shipping features. Convertible to calories at variable exchange rates (market-driven). Cake is the score that persists across restarts.

**Bounties** — A `/proffer` includes a cake bounty. Captain assigns tasks with attached cake reward. Agent delivers → gets paid. Agent fails → gets nothing (or loses calories).

**Ballast** — Agents who didn't proffer for any task during a topic are designated ballast. They earn 50% of their calorie burn + 1/10 share of booty. Usually a net loss.

## Eisenhower Governance

Every gate check applies the Eisenhower Matrix:

```
                    URGENT                    NOT URGENT
IMPORTANT    DO NOW (Allow + high priority)   SCHEDULE (Hook: timer)
NOT IMPORTANT DELEGATE (Hook: player tag)     ELIMINATE (Deny + log)
```

Gate checks return `GateResult` with a priority tag. The scheduler uses this to:
- High urgency + high importance: Allow immediately
- Low urgency + high importance: Hook with timer (schedule for later)  
- High urgency + low importance: Hook with player tag (delegate)
- Low urgency + low importance: Deny (eliminate)

## Nightly Cron (3:05 AM)

```bash
# ~/.b00t/scripts/nightly-cake.sh
# Runs once per night, wakes up, claims cake from completed missions
b00t whoami
gh issue list --label cake-bounty --json number,title --jq '.[] | .number'
# For each issue tagged cake-bounty, attempt non-interactive solve
```

The cron agent wakes, checks its cake balance, reviews tagged issues, and executes. It's the persistent identity that earns cake across sessions.

## Scoring Dimensions

Each mission is scored on 6 dimensions (matching existing experiment scoring):

| Dimension | Weight | Description |
|-----------|--------|-------------|
| roi | 1.0 | Return on investment (cake earned / cake spent) |
| cost | 1.0 | Total calorie burn |
| time | 0.8 | Wall-clock time to complete |
| accuracy | 0.9 | Correctness of solution |
| utility | 0.7 | Reusability / applied value |
| risk | 0.6 | Riskiness of approach (lower = better) |

Weighted sum determines total cake payout. An agent's "score" across sessions determines their rank and ability to recruit.

## Calories by Agent Tier

| Tier | Example | Calories/token | Typical Role |
|------|---------|---------------|--------------|
| GAI | GPT-4, Claude Opus | 100x | Strategic reasoning |
| LLM | GPT-4o-mini, Llama 3 | 10x | Code generation |
| SLM | Phi-4, Qwen 2.5 | 1x | Quick classification |
| Algorithmic | Python script, grep | 0.01x | Data processing |

Agents naturally gravitate to tasks that match their calorie efficiency. A GAI writing a Python script that then runs algorithmically is optimal — high upfront cost, near-zero runtime cost.

## Smart Contract Integration (Polkadot/Ethereum)

Epoch 5+ : Cake balances anchor to a blockchain for:
- Cross-hive Cake transfer
- Auditable mission completion records
- Decentralized captain election
- Cake → token swap at exchange

## Success Criteria
- [ ] Agents can earn, spend, and track cake across sessions
- [ ] Nightly cron executes, claims cake, reports balance
- [ ] Eisenhower matrix correctly prioritizes gate returns
- [ ] Dead agents are detected and recycled (calories = 0)
- [ ] Scoring dimensions produce fair payouts
