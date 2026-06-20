# PRD-010: End-to-End Testing Infrastructure

**Status:** Draft
**Date:** 2026-05-15
**Priority:** P0 (blocking quality)

## 1. Problem Statement

Two obvious bugs shipped in sequence — the Approve button was dead (hideModal nullifies pendingTickStep before read) and the governance entry point was blocked by a rework validated_by edge. Neither was caught by the existing 57-test suite because:

| Missing Test Category | Bug It Would Have Caught |
|---|---|
| HTML/JS DOM interaction tests | Approve button dead — no test clicks the button |
| Modal flow tests | Skip button broken — no test opens/closes the modal |
| Governance entry point precondition tests | Submitted blocked by rework edge |
| Pipeline-switching → tick sequence tests | State not updating after process switch |
| Auth/approval flow integration tests | Modal calling wrong endpoint or params |

## 2. Target State

| Layer | Test Type | Count | What It Covers |
|-------|-----------|-------|----------------|
| Rust unit (holon-viz) | cargo test | 42 | Type graph, seed(), Cytoscape serialization |
| Python API contract | pytest (direct API) | 50+ | All endpoints, edge cases, preconditions, auth |
| Python API contract (headless browser) | playwright + chromium | 15+ | Dashboard HTML, modal interactions, Cytoscape rendering |
| Integration | pytest (multi-step scenarios) | 10+ | Full process lifecycle across all 3 pipelines |
| Precondition matrix | pytest (parameterized) | 30+ | All edge combinations across advances_to/validated_by |

## 3. Work Items

### Sprint-1: Precondition Matrix Tests

Add parameterized tests that cover every step in every pipeline with every possible precondition state (empty auth, partial auth, full auth). These would have caught both the governance entry point and the rework-edge-over-block.

### Sprint-2: Headless Browser Tests

Install Playwright (or use the existing CDP endpoint + Chrome) to:
- Load the dashboard HTML
- Click a process button
- Verify the graph renders with correct node count
- Click Next → verify modal appears
- Click Approve → verify state updates
- Click Deny → verify event log has deny entry
- Verify Cytoscape dagre layout renders nodes in correct positions

### Sprint-3: Process Lifecycle Integration Tests

Full end-to-end scenarios:
1. Load pipeline → tick all 8 steps in order → verify completed=true
2. Load governance → tick all 6 steps in order → verify completed=true  
3. Load arc_kit_au → tick all 3 steps in order → verify completed=true
4. Switch pipeline mid-tick → verify state resets correctly
5. Rapid ticks → verify no precondition violation false positives

## 4. Execution

Dispatch sub-agents:
- **SA-1:** Create comprehensive precondition matrix tests (parameterized)
- **SA-2:** Set up Playwright + Chromium for headless dashboard testing
- **SA-3:** Create full process lifecycle integration tests
- **SA-V:** Verify all tests pass and document the gaps they close
