# PRD-001: b00t Agentic LLM Orchestration System

> **Status:** DRAFT | **Author:** Operator + {{_B00T_AGENT}} | **Date:** 2026-05-03
> **Scope:** Generic crawl pipeline, adversarial agent loop (A/B/R), hermes context plugin, b00t * catch-all, rotel tracing
> **Target:** b00t v0.8.x | **Priority:** P0

---

## Executive Summary

Build a unified agentic orchestration layer for b00t that enables:
1. **Generic crawl** — fetch → BinaryDocument → ledg3rr shape detection → parse → classify → ingest
2. **Adversarial agent loop** — AgentA (research/write) → AgentB (verify/bounce) → AgentR (retrospective/vote) after failures
3. **hermes context engine plugin** — replace lossy summarization with structured b00t datum retrieval
4. **`b00t *` catch-all** — single entry point with k0mmand3r classification dispatch
5. **rotel observability** — OpenTelemetry spans with `DebugLevel` enum (0-4) tracing every lifecycle stage

Design principle: **no new files where possible** — tie together existing b00t-mcp, b00t-cli, hermes, l3dg3rr, and crawl4ai/firecrawl infrastructure.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Agent Context                            │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │            hermes Context Engine (b00t plugin)          │ │
│  │                                                         │ │
│  │  should_compress() → compress() → datum retrieval       │ │
│  │  get_tool_schemas() → b00t_grep                         │ │
│  └────────────────────┬────────────────────────────────────┘ │
│                       │ struct context                       │
│                       ▼                                      │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              ralph b00t.sh Outer Loop                    │ │
│  │                                                         │ │
│  │  Attempt 1-2: AgentA → AgentB → Accept/Reject           │ │
│  │  Attempt 3+: AgentA → AgentB → AgentR → <|VOTE:CC##|>   │ │
│  └────────────────────┬────────────────────────────────────┘ │
│                       │ dispatch                             │
│                       ▼                                      │
│  ┌─────────────────────────────────────────────────────────┐ │
│  │              k0mmand3r Classifier                        │ │
│  │                                                         │ │
│  │  b00t("*") → route to: learn|task|grok|hive|crawl|cli    │ │
│  │  Unknown verb → guidance message with available handlers │ │
│  └────────────────────┬────────────────────────────────────┘ │
└───────────────────────┼─────────────────────────────────────┘
                        │
    ┌───────────────────┼────────────────────┐
    ▼                   ▼                    ▼
┌────────┐        ┌──────────┐        ┌──────────┐
│crawl   │        │datum     │        │rotel     │
│fetch → │        │store     │        │tracing   │
│Binary →│        │(git blob)│        │(spans)   │
│leadg3rr│◄───────│classify  │◄───────│inject    │
│parse → │        │ingest    │        │context   │
└────────┘        └──────────┘        └──────────┘
```

---

# b00t:map v1
# summary: PRD-001 — Unified agentic orchestration for b00t LLM lifecycle, observability, and context management
# tags: b00t, orchestration, agents, context-engine, crawl, ledg3rr, rotel, hermes, k0mmand3r, ralph
# tier: frontier
# cmds: b00t whoami, b00t task list, cd ~/.b00t/prd
# complexity: 10
