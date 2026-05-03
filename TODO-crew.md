# TODO: Crew Design Notes

## Goal

Enable b00t to spawn and run a crew (swarm) of agents with different roles using existing internal mechanisms for communication and coordination.

Constraints:
- Datums remain source of truth.
- DRY.
- NTRW (Never Reinvent The Wheel).
- MECE modeling.

## Core Mapping (Use Existing Primitives)

No new core protocol required. Use existing b00t IPC/MCP flow:

1. `agent_discover` -> select workers by role/capability/crew
2. `agent_delegate` -> fan out tasks with `task_id`
3. `agent_progress` -> heartbeat/status updates
4. `agent_wait` -> blocking join/timeout handling
5. `agent_message` -> peer/captain coordination
6. `agent_vote_create` / `agent_vote_submit` -> resolve contested decisions
7. `agent_complete` -> task closure + artifacts

## Datum Model (MECE)

- `role` datum: capability composition (skills + mcp + apps), no runtime orchestration.
- `crew` datum: orchestration topology + policies.
- `job`/`task` datum: executable unit of work.

## Source Of Truth Rules

1. Roles define capabilities and dependencies.
2. Crew defines captain/worker topology and coordination policy.
3. Job/task defines execution payload.
4. Runtime state is tracked by `task_id` and persisted as run metadata.

## Crew Lifecycle

1. `b00t crew spawn <crew-name>`
- Resolve crew datum.
- Resolve referenced role datums.
- Ensure required MCP/tools/apps are installed.

2. `b00t crew run <crew-name> <mission>`
- Captain discovers workers and delegates subtasks in parallel.

3. Worker execution
- Workers send progress and partial artifacts.
- Captain monitors timeout/failure and rebalances if needed.

4. Decision handling
- If contested, captain triggers vote workflow.

5. Completion
- Captain aggregates artifacts.
- Runs acceptance checks.
- Marks mission complete.
- Stores audit trail.

## Runtime Policies

- `task_timeout_sec`
- `retry_limit`
- `quorum` (majority/unanimous/custom)
- `max_parallel_delegations`
- `heartbeat_interval_sec`

## Minimal Crew Datum Example

```toml
[b00t]
name = "app-server"
type = "crew"
hint = "App server swarm"

[b00t.crew]
captain_role = "app-server-captain.role"
worker_roles = ["app-server-wizard.role", "qa-wizard.role", "ops-wizard.role"]
quorum = "majority"
task_timeout_sec = 900
retry_limit = 2
max_parallel_delegations = 4
heartbeat_interval_sec = 30
```

## CLI Contract (Target)

1. `b00t crew spawn <crew-name>`
2. `b00t crew run <crew-name> <mission>`
3. `b00t crew status <run-id>`
4. `b00t crew rebalance <run-id>`
5. `b00t crew stop <run-id>`

## Implementation Plan

Phase 1:
- Add `crew` datum schema + loader.
- Add resolver (crew -> roles -> capabilities).
- Add `b00t crew spawn` command skeleton.

Phase 2:
- Implement delegation loop using existing IPC tools.
- Add timeout/retry/rebalance policy execution.

Phase 3:
- Add vote/quorum flow for contested decisions.
- Add artifact aggregation and acceptance gates.

Phase 4:
- Persist run metadata/audit:
- crew/run id
- task ids
- agent assignments
- vote outcomes
- artifacts

Phase 5:
- Tests:
- resolver tests
- delegation flow tests
- timeout/retry tests
- quorum/vote tests

## Open Questions

1. Should `crew` support nested crews (crew-of-crews), or stay single-layer initially?
2. Should captain reassignment be automatic on failure or manual via command?
3. What is the default quorum policy for non-critical missions?
4. Should run-state persistence be local file, Redis, or both?

## Why This Is DRY / NTRW / MECE

- DRY: Reuses existing role datums and existing agent IPC commands.
- NTRW: No new custom orchestration protocol.
- MECE: Role = capabilities, Crew = coordination topology, Job/Task = execution unit.

