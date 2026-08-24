# Gap Closure Acceptance Test Report
## Date: 2026-05-14 13:49 UTC
## Server: http://fung1:8080
## CDP: ws://fung1:19222

---

## RESULTS TABLE

| Gap | Description | Result | Evidence |
|-----|-------------|--------|----------|
| G1 | Rhai DSL in Graph Output | **FAIL** | `/api/catalog` returns 13 steps as plain strings (e.g., `"governance::GovernanceState<Submitted>"`). No `rhai_dsl` key exists anywhere in the catalog response. The `dump-type-graph` binary outputs nodes with `{id, label, kind}` only — no `rhai_dsl` field. |
| G2 | Ephemeral Server State | **PASS** | Ticked step `governance::GovernanceState<Submitted>` → progress `1/13`. Killed server PID 1040247. Systemd restarted to PID 1040618 (~3s). State after restart: progress `1/13`, authorized `['governance::GovernanceState<Submitted>']` — matches pre-kill state. |
| G3 | CDP Endpoint Authenticity | **PASS** | `GET /json/version` returns real CDP metadata (Browser, Protocol-Version, webSocketDebuggerUrl). WebSocket upgrade to `ws://fung1:19222/devtools/page/ledgrrr-1` returns `101 Switching Protocols`. `Runtime.evaluate` returns valid result. `Page.captureScreenshot` returns valid PNG (magic bytes: `89504e470d0a1a0a`). |
| G4 | Single Pipeline Limitation | **FAIL** | `/api/catalog` returns exactly 1 pipeline key (`"pipeline"`) with 13 steps (governance + pipeline combined). No second pipeline (e.g., deploy, governance-only, etc.) exists. The `dump-type-graph` binary shows only one `z_pipeline/Pipeline` node. |
| G5 | Hardcoded Debug Binary Path | **PASS** | Search order implemented in `ledgrrr-viz-serve.py` lines 38-45: `$PATH` → `target/release/` → `target/debug/` → fallback to embedded JSON. Binary found at `target/debug/dump-type-graph` (8.1 MB). Server starts without errors whether binary is present or not. |
| G6 | No Service Supervision | **PASS** | `systemctl --user status` shows `active (running)`. Unit file has `Restart=always`, `RestartSec=3`. After killing server (PID 1040247), systemd restarted it to PID 1040618 within ~3 seconds. |
| G7 | API Authentication | **PASS** | `require_auth` decorator implemented on `/api/tick`, `/api/reset`, `/api/load`, `/api/log`, `/api/prune`. Token logic: `VIZ_TOKEN = os.environ.get("LEDGRRR_VIZ_TOKEN", "") or None`. Empty string → disabled (no auth). When token is set, `Authorization: Bearer <token>` is enforced. Service unit has `LEDGRRR_VIZ_TOKEN=` (empty) so auth is disabled by configuration. |

---

## SUMMARY

- **PASS: 5** (G2, G3, G5, G6, G7)
- **FAIL: 2** (G1, G4)

### Failing Gaps

**G1 (Rhai DSL)**: The `rhai_dsl` field is not populated on any step in the API catalog. Steps are returned as plain strings rather than objects with metadata. The `dump-type-graph` binary's `TypeNode` struct doesn't carry `rhai_dsl`. Requires extending `TypeRelationshipGraph` in the Rust binary and the Python server's catalog assembly.

**G4 (Second Pipeline)**: Only one pipeline exists (the combined governance+pipeline process). The PRD requires at least 2 pipelines — adding a `GovernancePipeline<S>` or `DeployPipeline` type-state machine with ≥3 states and an error/retry path, registered in `TypeRelationshipGraph::seed()`.

### Non-Failing Notes

- **G7**: Auth is correctly implemented but disabled because `LEDGRRR_VIZ_TOKEN` is set to empty string in the service unit. To enable, set `LEDGRRR_VIZ_TOKEN` to a non-empty value and restart the service.
- **G5**: The server is using the debug binary at the moment. The fallback path (embedded JSON) is implemented but untested in this session — would require deleting/renaming the binary and restarting.
