# PRD-008: ledgrrr Viz Dashboard — Gap Closure II

**Status:** Draft  
**Date:** 2026-05-15  
**Priority:** P1 (polish)

## 1. Problem Statement

PRD-007 closed 7 infrastructure gaps. 8 additional polish and correctness gaps remain between the Python simulation layer and the canonical Rust implementation.

## 2. Requirements

### G8: Auth Token Wiring

**Current:** `LEDGRRR_VIZ_TOKEN` env var exists in code but is never set. Auth decorator is dead code — the systemd unit has `Environment=LEDGRRR_VIZ_TOKEN=` (empty).

**Required:** 
- Add the systemd unit's `Environment` file to `~/.b00t/_b00t_/viz-auth.env` with a generated default token
- The systemd unit reads `EnvironmentFile=%h/.b00t/_b00t_/viz-auth.env`
- `just ledgrrr-viz-gen-token` generates a random token and writes it to the env file
- `LEDGRRR_VIZ_TOKEN` defaults to empty string → auth disabled. When file exists → auth enabled.

### G9: Display Rhai DSL in Dashboard

**Current:** `/api/catalog` returns `steps_detail` with `rhai_dsl` strings, but the HTML dashboard never renders them.

**Required:** 
- When a user clicks a step in the sidebar, show the Rhai DSL in a detail panel below the step list
- The detail panel shows: step ID, full type path, Rhai DSL script (monospace, syntax-highlighted with basic coloring)
- No external dependencies — use a `<pre>` block with simple CSS coloring for Rhai keywords
- The first step is selected by default on page load

### G10: just dump-type-graph Recipe

**Current:** No just recipe exists. Must know the exact cargo invocation.

**Required:** 
- Add `dump-type-graph` recipe to `vendor/ledgrrr/ledgrrr.just` that builds and runs the binary
- Add `dump-type-graph-to-api` recipe that runs the binary and POSTs the output to the viz server's `/api/load-graph` endpoint (new endpoint accepting raw TypeRelationshipGraph JSON)

### G11: arc_kit_au Rhai Scripts

**Current:** The evidence pipeline has 0/3 steps with `rhai_dsl`. The `HasVisualization` impls in `arc-kit-au` crate are missing their Rhai scripts.

**Required:** 
- Add `HasVisualization` impls for `Classification`, `ModelProposal`, and `OperatorApproval` in the `arc-kit-au` crate (or their visualization equivalents)
- Each impl must have a meaningful `RhaiDsl` script reflecting the evidence traceability domain
- Update `gen.rs` seed to pass the new rhai_dsl strings

### G12: Auth Returns Correct Status Code

**Current:** When `LEDGRRR_VIZ_TOKEN` is set but no Bearer header is provided, the server returns `400 Bad Request`. The PRD-007 spec'd `401 Unauthorized`.

**Required:**
- `require_auth` decorator returns `401 Unauthorized` with JSON body `{"error": "missing or invalid auth token"}` when no/invalid token
- `400 Bad Request` is reserved for malformed requests (missing params)

### G13: JSONL Log Rotation

**Current:** `~/.b00t/ledgrrr-viz-log.jsonl` grows unbounded with every tick. No size limit, no auto-prune.

**Required:**
- Add `MAX_LOG_SIZE = 10 * 1024 * 1024` (10 MB) constant
- On each `append()`, if log file exceeds `MAX_LOG_SIZE`, truncate to the last 1000 entries
- Log rotation must not block the tick request (offload to a background thread or check before/after)
- Add log file size to `/api/state` response as `log_file_size_bytes`

### G14: Active Process Name in API

**Current:** `/api/active` is defined but returns minimal info. The dashboard doesn't use it.

**Required:**
- `/api/active` returns the full active pipeline metadata (name, icon, description, step count)
- Add `process_name` and `process_description` to `/api/state` response
- Dashboard top bar shows the active pipeline name and icon

### G15: Governance Consent Gate Validation

**Current:** The governance pipeline has `PolicyChecked → Consented` and `PolicyChecked → Submitted (denied)` transitions defined as types, but the simulation allows any step to be ticked regardless of preconditions.

**Required:**
- Add a `gates` field to the catalog's `steps_detail` for each step listing its preconditions (which steps must be authorized before this step can tick)
- In the Python `LiveProcess.tick()`, check preconditions: if the step has a `validated_by` edge from a step that isn't authorized, return `{"ok": false, "error": "precondition not met: X must be authorized first"}`
- The governance `consent` step checks that `policy-check` was authorized with `"allowed"` — but since our simulation doesn't track edge labels in the log, simplify: `consent` requires `policy-check` to be authorized first.

## 3. Sub-agent Dispatch

| Gap | Owner | Dependencies | 
|-----|-------|-------------|
| G8: Auth token wiring | sub-agent A | None |
| G9: Rhai DSL display | sub-agent B | None |
| G10: just recipes | sub-agent C | None |
| G11: arc_kit_au scripts | sub-agent D | None |  
| G12: 401 status | sub-agent E | G8 (needs token to test) |
| G13: Log rotation | sub-agent F | None |
| G14: Process name API | sub-agent G | None |
| G15: Governance gates | sub-agent H | None |

Verification agent V runs ALL acceptance tests after G8-G15 complete.

## 4. Success Criteria

- All 8 gaps closed
- All acceptance tests pass
- Dashboard continues to serve at `http://fung1:8080/live`
- No regressions from PRD-007 closures
