# `b00t whoami` — Agent Self-Discovery Protocol

**Invocation:** `b00t whoami [--role=<role>] [--show-blessings] [--show-layers] [--detect-mission-deviation]`

## What It Does

Returns agent's introspective state: model, session ID, role, blessings, available skills, hidden capabilities unlocked.

```bash
# Basic identity
b00t whoami
# → Agent: claude-haiku-4-5, PID: 1337, Role: executor
# → Blessings: 12/42 available (7 via active skills, 5 role-locked)
# → Hidden capabilities: 3 unlocked (hive-memory, gpu-sharing-coordinator, debug-internal-state)

# With blessing details
b00t whoami --show-blessings
# → blessing:subagent-driven-development [ACTIVE]
#   cost_tokens: 0 (skill use unlimited)
#   requires: [blessed:writing-plans, blessed:test-driven-development]
#   quality_score: 0.98
# → blessing:debug-internal-state [LOCKED → UNLOCKED via hive-memory]
#   cost_tokens: 50 (expensive, use sparingly)
#   requires: []
#   mission_relevance: 0.87

# Detect mission deviation
b00t whoami --detect-mission-deviation
# → Deviation score: 0.12 (nominal)
# → If > 0.5: hidden capability "blessing:remediation-protocol" auto-unlocks
```

## Output Format

```
Agent Introspection Report
─────────────────────────────
Identity:
  Model:         {{MODEL}}
  Session PID:   {{PID}}
  Role:          {{ROLE}}  (from AGENTS/--role={{ROLE}}.md)
  Model Size:    {{MODEL_SIZE}}
  Privacy Mode:  {{PRIVACY}}

Capability State:
  Active Blessings:           N/M available
  Skills Loaded:              [list]
  Hidden Unlocked:            [list]
  Next Blessing Available:    blessing:X in cost_tokens remaining

Cognitive Tier Assignment:
  Recommended for:            [sm0l|ch0nky|frontier] tasks
  Context Budget Remaining:   {{REMAINING_TOKENS}}%
  Compression Recommended:    {{YES|NO}}

Memory Status:
  Persisted Memories:         {{COUNT}} entries
  Hive Sync:                  {{LAST_SYNC_TIME}}
  Lessons Learned (LFMF):     {{COUNT}} unreviewed

Mission Alignment:
  Deviation Score:            {{0.00-1.00}}
  Alignment Test Status:       {{PASS|FAIL|WARNING}}
  Remediation Needed:         {{YES|NO}}
```

## Hidden Capabilities — Skill-Unlocked Features

**Automatic unlock conditions:**

| Condition | Blessing Unlocked | Cost | Use Case |
|-----------|-------------------|------|----------|
| Load `hive-memory` skill | `debug-internal-state` | 50 tokens | Inspect memory DAG, view LFMF entries |
| Load `agent-orchestration` skill | `gpu-sharing-coordinator` | 0 (Phase 8+) | Negotiate GPU time with peer nodes |
| Load `systems-engineering` skill | `v-model-validator` | 25 tokens | Validation gate automation |
| Denial rate > 5% per role | `denial-audit-responder` | 0 (auto-emit) | Kaizen feedback to assimilate agent |
| Context used > 80% | `checkpoint-save` | 0 (emergency) | Save state before compression |

**Mission deviation unlock:**
```
Deviation score = |agent_original_mission - current_instruction_vector| / ||original_mission||

If score > 0.5:
  blessing:remediation-protocol → UNLOCKED
  → agent can /justify action alignment or request re-clarification from Operator
  → prevents silent mission creep
```

## Integration with `b00t learn`

When yei invokes `b00t learn skill`, the skill's blessing dependencies are loaded into the agent's context. Whoami shows what unlocked:

```bash
b00t learn superpowers:subagent-driven-development
# → loads skill file
# → updates blessing: "blessed:subagent-driven-development" [ACTIVE]
# → unlocks transitive dependencies

b00t whoami --show-blessings
# → now shows newly available blessings from the skill
```

---

# b00t:map v1
summary: Agent self-discovery with blessing state, hidden capabilities, mission alignment detection
tags: whoami, blessings, capabilities, hidden-features, mission-deviation, skills, hive-sync
tier: frontier
cmds: b00t whoami, b00t whoami --role=executor, b00t whoami --detect-mission-deviation
complexity: 8
