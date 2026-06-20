# Ralph Agent Instructions

## Overview

Ralph is an autonomous AI agent loop that runs AI coding tools (Amp or Claude Code) repeatedly until all PRD items are complete. Each iteration is a fresh instance with clean context.

## Commands

```bash
# Run the flowchart dev server
cd flowchart && npm run dev

# Build the flowchart
cd flowchart && npm run build

# Run Ralph with Amp
./ralph.sh [max_iterations]

# Run Ralph with Claude Code
./ralph.sh --agent claude [max_iterations]
```

## Key Files

- `ralph.sh` - The bash loop that spawns fresh AI instances (supports `--agent amp`, `--agent claude`, or `--agent codex`)
- `prompt.md` - Instructions given to each AMP instance
-  `CLAUDE.md` - Instructions given to each Claude Code instance
- `prd.json.example` - Example PRD format
- `flowchart/` - Interactive React Flow diagram explaining how Ralph works

## Flowchart

The `flowchart/` directory contains an interactive visualization built with React Flow. It's designed for presentations - click through to reveal each step with animations.

To run locally:
```bash
cd flowchart
npm install
npm run dev
```

## Patterns

- Each iteration spawns a fresh AI instance (Amp or Claude Code) with clean context
- Memory persists via git history, `progress.txt`, and `prd.json`
- Stories should be small enough to complete in one context window
- Always update AGENTS.md with discovered patterns for future iterations
- For uv-managed tooling, prefer `uv run python -m <tool>` (e.g., mypy, pytest) so commands use the project `.venv` and find dev dependencies

## Task Type Dispatch

When the OODA loop assigns you a task, the **task title prefix** determines the correct recipe to call:

| Task title prefix      | Action                                          |
|------------------------|-------------------------------------------------|
| `research-soul: <T>`  | `just research-soul topic=<T>` — ingest raw sources into datum |
| `review-soul: <T>`    | `just review-soul topic=<T>` — pi LLM semantic quality review  |
| `bug: <desc>`         | Implement fix; run tests; commit               |
| `feat: <desc>`        | Implement feature; run tests; commit           |
| (user story text)     | Standard OODA story implementation cycle       |

**review-soul tasks** are auto-queued by `b00t learn` when Stage 1 (keyword-overlap) AND Stage 2 (grok vector) both reject the result. The pi reviewer runs locally at ch0nky tier and may escalate to `research-soul` if the datum quality is below threshold (score < 3/5).

When running a `just` recipe, ensure you are in the b00t repo root (where `justfile` lives), not the ralph submodule directory.
