# Apalis Integration Analysis

## Executive Summary

Replacing TaskMaster-AI with apalis (https://github.com/apalis-dev/apalis) would make b00t fully self-contained in Rust while providing superior task orchestration capabilities.

## Current State

### TaskMaster-AI Issues
- ❌ Python dependency (external to b00t ecosystem)
- ❌ File-based JSON storage (tasks.json, no Redis/DB option)
- ❌ Complex dependency tree (uv, taskmaster packages)
- ❌ Ralph delegation requires Python runtime
- ❌ Not integrated with b00t datum ontology

### B00t Job System (Already Exists)
- ✅ DAG execution with topological sorting
- ✅ Git-based checkpoints for state persistence
- ✅ Agent spawning (codex, claude, amp)
- ✅ K0mmander script integration
- ✅ `.job.toml` datum format
- ⚠️ No runtime execution engine (just definitions)

## Apalis Capabilities

### Core Features
- **Type-safe Rust**: Async function handlers, no macros
- **Multiple backends**: Redis, PostgreSQL, MySQL, SQLite, in-memory, AMQP
- **Workflow orchestration**: DAG execution via `apalis-workflow`
- **Tower ecosystem**: Middleware, observability, extensibility
- **Web UI**: `apalis-board` for monitoring
- **Retry/timeout**: Built-in error handling
- **Distributed**: Multi-worker execution

### Comparison Matrix

| Feature | TaskMaster-AI | B00t Jobs | Apalis | Hybrid |
|---------|--------------|-----------|--------|--------|
| Language | Python | Rust (config only) | Rust | Rust |
| Storage | JSON files | Git + files | Redis/DB/Memory | Git + Redis |
| DAG execution | ✅ | ✅ (definition) | ✅ | ✅ |
| Checkpoints | ❌ | ✅ (git) | ❌ | ✅ (git) |
| Agent spawning | ✅ | ✅ | ⚠️ (custom) | ✅ |
| Observability | ❌ | ❌ | ✅ (board) | ✅ |
| Distributed | ❌ | ❌ | ✅ | ✅ |
| Self-contained | ❌ | ✅ | ✅ | ✅ |

## Proposed Hybrid Architecture

### Layer 1: Workflow Definition (B00t Job System)
- Keep `.job.toml` datum format for workflow definitions
- Skills generate jobs (e.g., `/prd` skill → `.job.toml`)
- Git tracks workflow state via checkpoints
- Datum ontology for discovery

### Layer 2: Execution Engine (Apalis)
- Apalis executes jobs defined in `.job.toml`
- Backend choice: Redis (distributed) or SQLite (local)
- `apalis-workflow` for DAG orchestration
- Agent tasks delegate to b00t agent system

### Layer 3: Progressive Disclosure (Skills)
- Skills provide `/prd` and other workflow generators
- Lightweight metadata for discovery (~100 tokens)
- On-demand instruction loading (<5k tokens)
- Example-based learning

## Integration Design

### Job → Apalis Workflow Mapping

```rust
// B00t job definition (.job.toml)
[[b00t.job.steps]]
name = "tests"
description = "Run test suite"
checkpoint = "tests-passed"
[b00t.job.steps.task]
type = "bash"
command = "cargo test --workspace"

// Apalis task handler
async fn execute_bash_task(cmd: BashTask) -> Result<(), Error> {
    // Execute bash command
    // Create git checkpoint on success
    // Store result in apalis backend
}
```

### New Crate Structure

```
b00t-cli/
├── src/
│   ├── datum_job.rs       # Job definitions (existing)
│   ├── datum_skill.rs     # Skills (just added)
│   ├── job_executor.rs    # NEW: Apalis integration
│   └── job_state.rs       # Git checkpoint logic (existing)
```

### Apalis Backend Configuration

```toml
# _b00t_/apalis.config.toml
[b00t]
name = "apalis"
type = "config"

[b00t.config.apalis]
backend = "redis"  # or "sqlite", "memory"
redis_url = "redis://localhost:6379"
workers = 4
max_retries = 3
timeout_ms = 300000

[b00t.config.apalis.features]
board_ui = true    # Enable apalis-board web UI
prometheus = true  # Metrics endpoint
sentry = false     # Error tracking
```

## Migration Path

### Phase 1: Apalis Foundation (This Sprint)
1. ✅ Skill datum type implemented
2. ➡️ Add apalis crates to dependencies
3. ➡️ Create `job_executor.rs` with apalis integration
4. ➡️ Implement `.job.toml` → apalis workflow compiler

### Phase 2: /prd Skill (Next Sprint)
1. Create `/prd` skill for PRD → `.job.toml` conversion
2. Skills generate apalis-compatible workflows
3. Test end-to-end: PRD → job → execution → checkpoints

### Phase 3: Ralph Migration (Future)
1. Update ralph to use apalis instead of taskmaster CLI
2. Ralph delegates to `b00t-cli job run` (apalis backend)
3. Remove Python taskmaster dependency

### Phase 4: Enhanced Features (Future)
1. Enable apalis-board web UI
2. Add Prometheus metrics
3. Support distributed execution via Redis
4. Multi-backend selection (SQLite for local, Redis for cluster)

## Benefits

### Immediate
- ✅ Self-contained Rust codebase (no Python)
- ✅ Better observability (apalis-board UI)
- ✅ Multiple storage backends (Redis, SQLite, memory)
- ✅ Distributed execution capability

### Long-term
- ✅ Aligned with b00t datum ontology
- ✅ Git-based state + database persistence
- ✅ Skills pattern for workflow generation
- ✅ Tower middleware ecosystem access
- ✅ Production-ready monitoring

## Risks & Mitigations

### Risk 1: Apalis 1.0.0-rc.3 (Release Candidate)
**Mitigation**: Pin to specific version, monitor releases, contribute fixes upstream

### Risk 2: Git Checkpoints + Apalis State
**Mitigation**: Use apalis for execution state, git for workflow snapshots (separation of concerns)

### Risk 3: Migration Effort
**Mitigation**: Incremental migration, keep TaskMaster-AI working during transition

## Decision Points

### ✅ Recommended: Hybrid Architecture
- B00t jobs for definitions (`.job.toml`)
- Apalis for execution (Redis/SQLite backend)
- Skills for generation (`/prd` skill)
- Remove TaskMaster-AI dependency

### Alternative: Pure B00t Job System
- Build custom execution engine
- More control, but reinventing apalis features
- Higher maintenance burden

### Alternative: Keep TaskMaster-AI
- Maintain Python dependency
- Miss out on Rust ecosystem benefits
- Continue with file-based limitations

## Next Steps

1. **Add Apalis Dependencies** (`Cargo.toml`)
   ```toml
   [dependencies]
   apalis = "1.0.0-rc.3"
   apalis-redis = "1.0.0-rc.3"
   apalis-sql = { version = "1.0.0-rc.3", features = ["sqlite"] }
   ```

2. **Implement Job Executor** (`job_executor.rs`)
   - Parse `.job.toml` definitions
   - Compile to apalis workflows
   - Execute with checkpoints

3. **Create /prd Skill**
   - Skills directory: `_b00t_/skills/prd-to-job/`
   - Generate `.job.toml` from PRD text
   - Example templates

4. **Update Ralph Integration**
   - Replace `uv run ralph` with `b00t-cli job run`
   - Use apalis backend for execution
   - Maintain git checkpoint pattern

---

🥾 Generated via b00t gospel alignment
🦀 Rust-native task orchestration FTW
