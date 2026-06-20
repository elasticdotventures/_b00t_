# PRD-007: ledgrrr Viz Dashboard — Gap Closure

**Status:** Draft
**Author:** Hermes Agent (executive operator)
**Date:** 2026-05-14
**Priority:** P0 (blocking UX validation)

## 1. Problem Statement

The ledgrrr viz dashboard at `http://fung1:8080/live` has 7 known gaps between the Python simulation layer and the canonical Rust implementation. These gaps prevent the dashboard from serving as a reliable visualization and validation tool for the ledgrrr process pipeline.

## 2. Scope

Close all 7 gaps identified in the gap analysis. Each gap has an owner sub-agent, a test suite, and an acceptance criterion defined by a separate verification agent reading the same PRD.

## 3. Requirements

### Gap 1: Rhai DSL in Graph Output

**Current state:** `dump-type-graph` outputs type nodes and `AdvancesTo`/`ValidatedBy` relationships, but does not include the `RhaiDsl` scripts from `HasVisualization::viz_spec()`.

**Required:** Extend `TypeRelationshipGraph` to carry `rhai_dsl: Option<String>` on each `TypeNode`. Update `seed()` to populate it from the `HasVisualization` impls. Update `dump-type-graph` to output it. Update the dashboard to display the Rhai script for the currently selected step.

**Acceptance:** Each pipeline step returned by `/api/catalog` includes a `rhai_dsl` field with the script string. The dashboard sidebar shows the Rhai script when a step is selected. Test agent verifies by parsing the API response.

### Gap 2: Ephemeral Server State

**Current state:** `LiveProcess` resets on server restart. Ticks don't survive.

**Required:** Add a JSON log file backend. On each tick, append the receipt to `~/.b00t/ledgrrr-viz-log.jsonl`. On server start, replay the log to restore state. Add `GET /api/log` to return the raw JSONL. Add `POST /api/prune` to truncate.

**Acceptance:** After killing and restarting the server, `/api/state` returns the same authorized steps as before the restart. Test agent verifies by ticking, killing, restarting, and checking state.

### Gap 3: CDP Endpoint Authenticity

**Current state:** `/json/version` returns fake metadata. VizObserver tests always return `CDP_UNAVAILABLE`.

**Required:** Make the CDP endpoint respond to real CDP commands. At minimum: `Page.enable`, `Page.captureScreenshot`, `Runtime.evaluate` (to query Cytoscape node count). Implement a WebSocket handler on port 19222 that speaks the CDP protocol. When no real WebView2 is available, return a synthetic screenshot (a PNG of the current Cytoscape graph server-side rendered).

**Acceptance:** `curl http://localhost:19222/json/version` returns real-looking CDP metadata. A WebSocket connection to `ws://localhost:19222/devtools/page/ledgrrr-1` accepts CDP commands. `Page.captureScreenshot` returns a valid PNG. Test agent verifies with a WebSocket client.

### Gap 4: Single Pipeline Limitation

**Current state:** Only the tax pipeline from `seed()` exists. Other pipelines (governance, deploy, etc.) were removed as drift.

**Required:** Define a second pipeline in Rust — at minimum a `GovernancePipeline<S>` type-state machine with `HasVisualization` impls. Must have at least 3 states and 2 transitions, one of which is an error/retry path. Register it in `TypeRelationshipGraph::seed()`.

**Acceptance:** `/api/catalog` returns 2 pipelines. The second has different steps and edges than the first. Test agent verifies by parsing the API response.

### Gap 5: Hardcoded Debug Binary Path

**Current state:** `DUMP_BINARY = VENDOR_DIR / "target/debug/dump-type-graph"`. Won't resolve in release builds.

**Required:** Search for the binary in order: 1) `$PATH` for `dump-type-graph`, 2) `vendor/ledgrrr/target/release/dump-type-graph`, 3) `vendor/ledgrrr/target/debug/dump-type-graph`, 4) fallback to embedded JSON. Add a `just dump-type-graph` recipe that builds and runs it.

**Acceptance:** Server starts without errors even when `target/debug/` doesn't exist (uses fallback). When the binary is present, it's used. Test agent verifies by renaming the debug dir and restarting.

### Gap 6: No Service Supervision

**Current state:** PID file daemonization with no restart, no logs, no systemd.

**Required:** Add a `systemd --user` unit file at `~/.config/systemd/user/ledgrrr-viz-serve.service`. Add `just ledgrrr-viz-install-service`, `just ledgrrr-viz-enable`, `just ledgrrr-viz-status`. The service auto-restarts on crash and logs to journald.

**Acceptance:** `systemctl --user status ledgrrr-viz-serve` shows active. Killing the process causes systemd to restart it within 5 seconds. Test agent verifies by killing the process and checking the new PID.

### Gap 7: API Authentication

**Current state:** No auth. Anyone on the LAN can tick/reset.

**Required:** Add a simple bearer token check. Read `LEDGRRR_VIZ_TOKEN` from environment. If set, all `/api/*` routes except `/json/*` require `Authorization: Bearer <token>` header. The landing page and dashboard HTML pass the token. Add `POST /api/auth` that returns a short-lived token on valid credentials.

**Acceptance:** Without token, `/api/tick` returns 401. With valid token, it works. Test agent verifies with and without the header.

## 4. Sub-agent Dispatch Plan

| Gap | Owner | Dependencies | Verification |
|-----|-------|-------------|-------------|
| G1: Rhai DSL | sub-agent A | None | sub-agent V reads PRD §G1, tests after A |
| G2: Persistence | sub-agent B | None | sub-agent V reads PRD §G2, tests after B |
| G3: CDP | sub-agent C | None | sub-agent V reads PRD §G3, tests after C |
| G4: Second pipeline | sub-agent D | None | sub-agent V reads PRD §G4, tests after D |
| G5: Binary path | sub-agent E | None | sub-agent V reads PRD §G5, tests after E |
| G6: Service | sub-agent F | None | sub-agent V reads PRD §G6, tests after F |
| G7: Auth | sub-agent G | None | sub-agent V reads PRD §G7, tests after G |

Verification agent V runs all acceptance tests against the running server after all G1-G7 sub-agents complete.

## 5. Test Protocol

Each verification test:
1. Reads the corresponding PRD section for acceptance criteria
2. Executes the test (API call, process manipulation, etc.)
3. Reports PASS/FAIL with evidence
4. Failure blocks the sub-agent from being marked complete

## 6. Success Criteria

- All 7 gaps closed
- All acceptance tests pass
- Dashboard continues to serve at `http://fung1:8080/live`
- No regressions in existing functionality (tick, catalog, state, graph)
