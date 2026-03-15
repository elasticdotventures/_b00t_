---
name: model-routing
description: Select the right model for the task. Maps task cognitive tier to optimal model. Reads _b00t_ datums for available models. Prefer local/cheap for deterministic work; frontier for reasoning.
disable-model-invocation: false
---

# Model Routing

NEVER use frontier model for mechanical work. Route by cognitive tier.
Reads routing table from `_b00t_/model-routing.tomllm`. Falls back to hardcoded tiers.

## Cognitive Tiers

| Tier | Tasks | b00t Models (in priority order) |
|------|-------|--------------------------------|
| `sm0l` | grep, lint, classify, route, test pass/fail | haiku, local sm0l |
| `ch0nky` | implement, refactor, debug, code review | qwen3-coder-local (RTX 3090), sonnet |
| `frontier` | architecture, security, novel design, planning | opus, sonnet |

## Steps

1. Classify the task type. # output: task_type
2. Map task_type → cognitive tier. # output: tier (sm0l|ch0nky|frontier)
3. Read available models from `b00t learn model-routing` or datum file. # output: available_models[]
4. Select best available model for tier (prefer local, fallback frontier). # output: selected_model
5. Check resource gate: `b00t hive status` — ensure RAM/GPU available. # output: resource_ok
6. If resource gate fails: escalate one tier up or queue. # output: model_or_queue
7. Return `{model, tier, rationale}` for caller to invoke.

## Task → Tier Mapping

**sm0l** (Haiku / local 3B):
- Running tests, checking lint output
- Classifying/routing messages
- Extracting structured data from well-defined input
- File diffing, counting, summarizing short text
- Executing known shell commands

**ch0nky** (qwen3-coder-local → Sonnet fallback):
- Writing or refactoring code
- Debugging with stack traces
- Multi-file code review
- Translating between languages/formats
- Implementing skills from SKILL.md spec

**frontier** (Opus → Sonnet fallback):
- System architecture decisions
- Security threat modeling
- Novel algorithm design
- Planning complex multi-step workflows
- Evaluating ambiguous requirements

## Output Contract to Executive Context

Executive context is costly. Sub-agents MUST return compressed summaries:

| Tier | Max output to executive |
|------|------------------------|
| `sm0l` | `PASS` or `FAIL: <name> <5 lines>` |
| `ch0nky` | diff + test result (no full file dumps) |
| `frontier` | structured decision with rationale |

## Resource Awareness

Before invoking ch0nky/frontier check hive:
```bash
b00t hive status  # output: RAM free, GPU VRAM free, active profile
```

Anti-pattern: running vLLM (qwen3-coder, 20GB VRAM) + HuggingFace download simultaneously on 24GB.

## Integration

Used by `/next-task` at each phase to select model.
Used by `b00t-mcp` agent delegation.
Reads datum: `_b00t_/model-routing.tomllm`
