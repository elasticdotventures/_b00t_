# Implementation Plan: ATO Legislation Ingestion Pipeline
# Plan date: 2026-06-21 | Phase: plan | Compound Engineering Epoch 1

## Architecture

Leverage existing b00t infrastructure — zero new crates:
```
ATO Legislation API
    ↓ FetchNode (String → DocumentSource)
    ↓ ChunkNode (DocumentSource → Vec<SemanticChunk>)
    ↓ EvidenceNode (Vec<SemanticChunk> → Vec<Evidence>)
    ↓ RequirementsNode (Vec<Evidence> → Vec<Requirement>)
    ↓ FullPipelineResult (JSON → NoSQL → b00t-admin /api/admin/pipeline)
```

## Tasks

### Task 1: ATO API Client (sm0l, ~50 lines)
**File**: `b00t-c0re-lib/src/ato_client.rs`
- Implement `AtoClient::fetch_legislation(act: &str) -> DocumentSource`
- Use reqwest to fetch from legislation.gov.au API
- Parse XML/HTML into DocumentSource struct
- Handle rate limiting (1 req/3s)
- **Test**: `cargo test -p b00t-c0re-lib -- ato_client`

### Task 2: Legislation Chunker (ch0nky, ~80 lines)
**File**: `b00t-c0re-lib/src/pipeline_nodes.rs` (extend ChunkNode)
- Add `LegislationChunker` variant to ChunkNode
- Split by section headers (regex: `^\d+[-A-Z]+\s`)
- Preserve hierarchical structure (Part > Division > Section)
- **Test**: `cargo test -p b00t-c0re-lib -- legislation_chunker`

### Task 3: ATO Datum Config (sm0l, ~30 lines)
**File**: `_b00t_/ato-legislation.cli.toml`
- Register legislation acts as b00t datums
- Define fetch URLs, update intervals
- **Test**: `b00t-cli detect ato-legislation`

### Task 4: Integration Test (ch0nky, ~100 lines)
**File**: `b00t-c0re-lib/tests/ato_pipeline_test.rs`
- Compose FetchNode → ChunkNode → EvidenceNode → RequirementsNode
- Execute with mock ITAA 1997 sample
- Verify provenance pointers reference exact sections
- Verify FOL postconditions hold
- **Test**: `cargo test -p b00t-c0re-lib --test ato_pipeline_test`

### Task 5: Admin Dashboard Update (sm0l, ~40 lines)
**File**: `b00t-admin/src/main.rs`
- Add ATO pipeline to /api/admin/processes
- Add legislation-specific health check
- **Test**: `curl localhost:31337/api/admin/processes | jq '.pipeline'`

## File Map

| Task | File | Lines | Tier | Depends On |
|------|------|-------|------|------------|
| 1 | `b00t-c0re-lib/src/ato_client.rs` (new) | ~50 | sm0l | — |
| 2 | `b00t-c0re-lib/src/pipeline_nodes.rs` (modify) | ~80 | ch0nky | Task 1 |
| 3 | `_b00t_/ato-legislation.cli.toml` (new) | ~30 | sm0l | — |
| 4 | `b00t-c0re-lib/tests/ato_pipeline_test.rs` (new) | ~100 | ch0nky | Tasks 1-2 |
| 5 | `b00t-admin/src/main.rs` (modify) | ~40 | sm0l | Tasks 1-2 |

## Gh Issues

Each task becomes a GitHub issue with label `compound-engineering`:
