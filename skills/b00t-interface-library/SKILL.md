---
name: b00t-interface-library
description: >
  Design and implement a Rust interface library in l3dg3rr that acts as a
  feature-configurable lifecycle manager for b00t processes. The library
  compliantly implements init → operate → terminate → lifecycle maintenance
  of miscellaneous process surfaces (MCP servers, daemons, sidecars) with
  deterministic governance controls. Uses the autoresearch pattern
  (karpathy/autoresearch): agent reads program.md, iterates on the library,
  experiments autonomously.
version: 1.0.0
allowed-tools: Read, Write, Edit, Grep, Glob, Bash, Task
---

## Overview

The b00t interface library (`crates/b00t-iface/`) is the Rust analog of
`train.py` in karpathy/autoresearch. It is the single file the agent modifies
and improves. The "research loop" is:

```
program.md (this skill) ──→ agent
  ↓ iterates on
b00t-iface/src/lib.rs (single-file interface library)
  ↓ tested by
cargo test -p b00t-iface   (5-minute eval budget)
  ↓ keep/discard based on
test results + coverage + lint pass
```

## Core Trait: `ProcessSurface`

The library provides one primary trait and one lifecycle manager:

```rust
/// A process surface is any executable that can be started, checked,
/// and stopped by the b00t hive. Examples: MCP servers, sidecars,
/// daemons, one-shot scripts.
pub trait ProcessSurface {
    type Config: Deserialize;
    type Error: std::error::Error;
    type Handle;

    /// Declare what this surface needs before it can run.
    fn requirements(&self) -> Vec<Requirement>;

    /// Init: validate config, resolve dependencies, acquire resources.
    fn init(&mut self, config: Self::Config) -> Result<(), Self::Error>;

    /// Operate: start the process, return a handle for lifecycle control.
    fn operate(&self) -> Result<Self::Handle, Self::Error>;

    /// Terminate: graceful shutdown, resource release, audit record.
    fn terminate(handle: Self::Handle) -> Result<AuditRecord, Self::Error>;

    /// Lifecycle maintenance: health check, restart policy, log rotation.
    fn maintain(&self) -> MaintenanceAction;
}
```

## Governance Controls

Each surface declares a `GovernancePolicy`:

```rust
pub struct GovernancePolicy {
    /// Which agents/roles are allowed to start this surface.
    pub allowed_starters: Vec<AgentRole>,
    /// Maximum runtime before forced re-evaluation.
    pub max_ttl: Duration,
    /// Whether this surface can be restarted automatically.
    pub auto_restart: bool,
    /// Crash threshold before surface is quarantined.
    pub crash_budget: u32,
}
```

## Autoresearch Loop

The agent follows this loop autonomously:

1. **Read** this skill and the current `crates/b00t-iface/src/lib.rs`
2. **Modify** `lib.rs` — add a new surface impl, improve governance, fix invariant
3. **Run** `cargo test -p b00t-iface 2>&1 | tail -20` (the 5-minute eval)
4. **Keep** if tests pass AND new coverage > 80%
5. **Discard** if tests fail — revert with `git checkout -- crates/b00t-iface/`
6. **Log** the experiment outcome in `EXPERIMENTS.md`
7. **Repeat** until user halts or 100 experiments reached

## First Experiment

The first surface to implement: the **datum file watcher** — a daemon that
watches `_b00t_/datums/` for changes and re-validates the invariant schema.
This proves the init → operate → terminate → maintain cycle with a real b00t
resource.

## Interfaces

The library decomposes into:

| Module | Purpose | Status |
|--------|---------|--------|
| `surface` | `ProcessSurface` trait + `LifecycleManager` | Design |
| `governance` | `GovernancePolicy`, agent roles, crash budget | Design |
| `datum_watcher` | Datum file watcher surface | First target |
| `audit` | `AuditRecord` type for termination events | Design |
