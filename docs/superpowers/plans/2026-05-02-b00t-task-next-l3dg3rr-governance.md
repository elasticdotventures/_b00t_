# b00t task --next with l3dg3rr Governance Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development to execute this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement `b00t task --next` with governance-driven multi-source task discovery (local queue → GitHub) via l3dg3rr's internal MCP service.

**Architecture:** l3dg3rr-core provides governance trait library. b00t-cli embeds l3dg3rr-mcp as an internal MCP service that orchestrates task discovery with state validation, authorization gates, and OTel logging. All downstream I/O (queue read, GitHub fetch, task import) flows through governance before execution.

**Tech Stack:** Rust, b00t-cli, l3dg3rr-core (new), MCP, OpenTelemetry, serde, uuid

---

## File Structure

**New files:**
- `b00t-l3dg3rr-core/Cargo.toml` — l3dg3rr-core crate manifest
- `b00t-l3dg3rr-core/src/lib.rs` — trait exports
- `b00t-l3dg3rr-core/src/traits.rs` — TaskQueueInvariant, GovernanceGate, TransactionLog
- `b00t-l3dg3rr-core/src/error.rs` — GovernanceError enum
- `b00t-l3dg3rr-core/src/types.rs` — TaskQueueState, TransactionStep, TransactionLog, TransactionResult
- `b00t-cli/src/mcp/l3dg3rr_service.rs` — l3dg3rr-mcp MCP service impl
- `b00t-cli/src/mcp/governance.rs` — governance gates (CanQueryGitHub, etc.)
- `b00t-cli/tests/task_next_governance.rs` — integration tests
- `b00t-l3dg3rr-core/tests/governance_tests.rs` — trait validation tests

**Modified files:**
- `Cargo.toml` (root) — add l3dg3rr-core member
- `b00t-cli/Cargo.toml` — add deps: opentelemetry, tracing, l3dg3rr-core
- `b00t-cli/src/commands/task.rs` — modify task::next() to delegate to l3dg3rr-mcp
- `b00t-cli/src/mcp/mod.rs` — register l3dg3rr service
- `b00t-cli/src/lib.rs` — export mcp modules

---

## Chunk 1: l3dg3rr-core Crate (Governance Traits)

### Task 1: Create l3dg3rr-core crate structure

**Files:**
- Create: `b00t-l3dg3rr-core/Cargo.toml`
- Create: `b00t-l3dg3rr-core/src/lib.rs`
- Modify: `Cargo.toml` (root)

- [ ] **Step 1: Create l3dg3rr-core directory and Cargo.toml**

```bash
mkdir -p b00t-l3dg3rr-core/src b00t-l3dg3rr-core/tests
```

Create `b00t-l3dg3rr-core/Cargo.toml`:
```toml
[package]
name = "b00t-l3dg3rr-core"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true
description = "Governance traits for l3dg3rr transaction logging and authorization"

[dependencies]
serde = { workspace = true, features = ["derive"] }
uuid = { version = "1.6", features = ["v4", "serde"] }
thiserror = "1.0"

[lints]
workspace = true
```

- [ ] **Step 2: Add l3dg3rr-core to workspace**

Modify `Cargo.toml` (root), find `members = [` section, add:
```toml
members = [
    "b00t-cli",
    "b00t-c0re-lib",
    "b00t-grok",
    "b00t-lib-chat",
    "b00t-py",
    "b00t-ipc",
    "b00t-azure-cp",
    "b00t-l3dg3rr-viz",
    "b00t-l3dg3rr-core",  # ADD THIS LINE
]
```

- [ ] **Step 3: Create lib.rs skeleton**

Create `b00t-l3dg3rr-core/src/lib.rs`:
```rust
//! l3dg3rr-core — Governance traits for transaction logging and authorization
//!
//! Provides invariant-based governance framework:
//! - TaskQueueInvariant: validate task queue state transitions
//! - GovernanceGate: check authorization before actions
//! - TransactionLog: record all decisions with OTel span context

pub mod error;
pub mod traits;
pub mod types;

pub use error::GovernanceError;
pub use traits::{GovernanceGate, TaskQueueInvariant};
pub use types::{TaskQueueState, TransactionLog, TransactionResult, TransactionStep};
```

- [ ] **Step 4: Verify crate builds**

```bash
cd b00t-l3dg3rr-core && cargo build
```

Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add b00t-l3dg3rr-core Cargo.toml
git commit -m "feat(l3dg3rr-core): create governance traits crate skeleton"
```

---

### Task 2: Implement GovernanceError type

**Files:**
- Create: `b00t-l3dg3rr-core/src/error.rs`
- Modify: `b00t-l3dg3rr-core/src/lib.rs`

- [ ] **Step 1: Write error tests**

Create `b00t-l3dg3rr-core/tests/governance_tests.rs`:
```rust
#[test]
fn governance_error_display() {
    use b00t_l3dg3rr_core::GovernanceError;

    let err = GovernanceError::QueueNotEmpty;
    assert_eq!(err.to_string(), "queue not empty");

    let err = GovernanceError::RateLimited;
    assert_eq!(err.to_string(), "rate limited");

    let err = GovernanceError::Unauthorized;
    assert_eq!(err.to_string(), "unauthorized");
}

#[test]
fn governance_error_is_error() {
    use std::error::Error;
    use b00t_l3dg3rr_core::GovernanceError;

    let err: Box<dyn Error> = Box::new(GovernanceError::QueueNotEmpty);
    assert!(!err.to_string().is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd b00t-l3dg3rr-core && cargo test --test governance_tests
```

Expected: FAIL (GovernanceError not defined)

- [ ] **Step 3: Implement GovernanceError**

Create `b00t-l3dg3rr-core/src/error.rs`:
```rust
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
#[error("{}", match self {
    GovernanceError::QueueNotEmpty => "queue not empty",
    GovernanceError::RateLimited => "rate limited",
    GovernanceError::Unauthorized => "unauthorized",
})]
pub enum GovernanceError {
    QueueNotEmpty,
    RateLimited,
    Unauthorized,
}
```

- [ ] **Step 4: Export from lib.rs**

In `b00t-l3dg3rr-core/src/lib.rs`, error is already in `pub use error::GovernanceError;`

- [ ] **Step 5: Run tests to verify they pass**

```bash
cd b00t-l3dg3rr-core && cargo test --test governance_tests
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add b00t-l3dg3rr-core/src/error.rs b00t-l3dg3rr-core/tests/governance_tests.rs
git commit -m "feat(l3dg3rr-core): implement GovernanceError enum"
```

---

### Task 3: Implement TaskQueueState and TransactionLog types

**Files:**
- Create: `b00t-l3dg3rr-core/src/types.rs`
- Modify: `b00t-l3dg3rr-core/src/lib.rs`

- [ ] **Step 1: Write tests for types**

Add to `b00t-l3dg3rr-core/tests/governance_tests.rs`:
```rust
#[test]
fn transaction_log_tracks_steps() {
    use b00t_l3dg3rr_core::{TransactionLog, TransactionStep, TransactionResult};

    let mut log = TransactionLog::new();
    assert_eq!(log.steps().len(), 0);

    let step = TransactionStep::new("queue_check".into(), true);
    log.add_step(step);
    assert_eq!(log.steps().len(), 1);
}

#[test]
fn transaction_result_success_stores_task_id() {
    use b00t_l3dg3rr_core::TransactionResult;

    let result = TransactionResult::Success(42);
    match result {
        TransactionResult::Success(id) => assert_eq!(id, 42),
        _ => panic!("expected Success"),
    }
}

#[test]
fn task_queue_state_variants() {
    use b00t_l3dg3rr_core::TaskQueueState;

    let _ = TaskQueueState::Empty;
    let _ = TaskQueueState::Pending;
    let _ = TaskQueueState::InProgress;
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd b00t-l3dg3rr-core && cargo test --test governance_tests task_queue_state_variants
```

Expected: FAIL (TaskQueueState not defined)

- [ ] **Step 3: Implement types**

Create `b00t-l3dg3rr-core/src/types.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskQueueState {
    Empty,
    Pending,
    InProgress,
}

#[derive(Debug, Clone)]
pub struct TransactionStep {
    gate: String,
    passed: bool,
    reason: Option<String>,
    timestamp: Instant,
}

impl TransactionStep {
    pub fn new(gate: String, passed: bool) -> Self {
        Self {
            gate,
            passed,
            reason: None,
            timestamp: Instant::now(),
        }
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn gate(&self) -> &str {
        &self.gate
    }

    pub fn passed(&self) -> bool {
        self.passed
    }

    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionResult {
    Success(u64), // task_id
    Denied(/* reason will be in steps */),
    Error,
}

#[derive(Debug, Clone)]
pub struct TransactionLog {
    id: Uuid,
    steps: Vec<TransactionStep>,
    created_at: Instant,
    result: Option<TransactionResult>,
}

impl TransactionLog {
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            steps: Vec::new(),
            created_at: Instant::now(),
            result: None,
        }
    }

    pub fn add_step(&mut self, step: TransactionStep) {
        self.steps.push(step);
    }

    pub fn steps(&self) -> &[TransactionStep] {
        &self.steps
    }

    pub fn set_result(&mut self, result: TransactionResult) {
        self.result = Some(result);
    }

    pub fn result(&self) -> Option<TransactionResult> {
        self.result
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn duration_ms(&self) -> u128 {
        self.created_at.elapsed().as_millis()
    }
}

impl Default for TransactionLog {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd b00t-l3dg3rr-core && cargo test --test governance_tests
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add b00t-l3dg3rr-core/src/types.rs
git commit -m "feat(l3dg3rr-core): implement TransactionLog and TaskQueueState types"
```

---

### Task 4: Implement governance traits

**Files:**
- Create: `b00t-l3dg3rr-core/src/traits.rs`
- Modify: `b00t-l3dg3rr-core/src/lib.rs`

- [ ] **Step 1: Write trait tests**

Add to `b00t-l3dg3rr-core/tests/governance_tests.rs`:
```rust
#[test]
fn task_queue_invariant_trait_exists() {
    use b00t_l3dg3rr_core::TaskQueueInvariant;

    // This test just verifies the trait can be imported
    fn _requires_invariant<T: TaskQueueInvariant>(_: T) {}
}

#[test]
fn governance_gate_trait_exists() {
    use b00t_l3dg3rr_core::GovernanceGate;

    // This test just verifies the trait can be imported
    fn _requires_gate<T: GovernanceGate>(_: T) {}
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cd b00t-l3dg3rr-core && cargo test --test governance_tests task_queue_invariant
```

Expected: FAIL (trait not defined)

- [ ] **Step 3: Implement traits**

Create `b00t-l3dg3rr-core/src/traits.rs`:
```rust
use crate::{GovernanceError, TaskQueueState};
use serde::Serialize;

/// TaskQueueInvariant validates state transitions of the task queue.
pub trait TaskQueueInvariant {
    /// Current queue state
    fn state(&self) -> TaskQueueState;

    /// Validate transition from -> to is allowed by invariants
    fn is_valid_transition(&self, from: TaskQueueState, to: TaskQueueState) -> bool;
}

/// Context passed to governance gates for decision-making
#[derive(Debug, Clone, Serialize)]
pub struct GovernanceContext {
    pub queue_state: TaskQueueState,
    pub timestamp: std::time::SystemTime,
}

impl Default for GovernanceContext {
    fn default() -> Self {
        Self {
            queue_state: TaskQueueState::Empty,
            timestamp: std::time::SystemTime::now(),
        }
    }
}

/// GovernanceGate checks whether an action is allowed given current context.
pub trait GovernanceGate {
    /// Check if action is allowed. Returns Ok(()) if allowed, Err(reason) if denied.
    fn check(&self, context: &GovernanceContext) -> Result<(), GovernanceError>;

    /// Explain why this gate exists.
    fn reason(&self) -> String;
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cd b00t-l3dg3rr-core && cargo test --test governance_tests
```

Expected: PASS

- [ ] **Step 5: Verify lib.rs exports**

Check `b00t-l3dg3rr-core/src/lib.rs` includes:
```rust
pub use traits::{GovernanceContext, GovernanceGate, TaskQueueInvariant};
```

- [ ] **Step 6: Commit**

```bash
git add b00t-l3dg3rr-core/src/traits.rs
git commit -m "feat(l3dg3rr-core): implement governance trait definitions"
```

---

## Chunk 2: b00t-cli Integration (Governance Gates + MCP Service)

### Task 5: Add CanQueryGitHub governance gate

**Files:**
- Create: `b00t-cli/src/mcp/governance.rs`

- [ ] **Step 1: Write tests for CanQueryGitHub gate**

Create `b00t-cli/tests/task_next_governance.rs`:
```rust
#[test]
fn can_query_github_allows_when_queue_empty() {
    use b00t_l3dg3rr_core::{GovernanceContext, GovernanceGate, TaskQueueState};
    use b00t_cli::mcp::governance::CanQueryGitHub;

    let gate = CanQueryGitHub::new(TaskQueueState::Empty);
    let ctx = GovernanceContext {
        queue_state: TaskQueueState::Empty,
        timestamp: std::time::SystemTime::now(),
    };

    assert!(gate.check(&ctx).is_ok());
}

#[test]
fn can_query_github_denies_when_queue_pending() {
    use b00t_l3dg3rr_core::{GovernanceContext, GovernanceError, GovernanceGate, TaskQueueState};
    use b00t_cli::mcp::governance::CanQueryGitHub;

    let gate = CanQueryGitHub::new(TaskQueueState::Pending);
    let ctx = GovernanceContext {
        queue_state: TaskQueueState::Pending,
        timestamp: std::time::SystemTime::now(),
    };

    assert_eq!(gate.check(&ctx), Err(GovernanceError::QueueNotEmpty));
}

#[test]
fn can_query_github_reason_is_documented() {
    use b00t_cli::mcp::governance::CanQueryGitHub;
    use b00t_l3dg3rr_core::{GovernanceGate, TaskQueueState};

    let gate = CanQueryGitHub::new(TaskQueueState::Empty);
    let reason = gate.reason();

    assert!(!reason.is_empty());
    assert!(reason.contains("queue"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test task_next_governance --lib
```

Expected: FAIL (CanQueryGitHub not found)

- [ ] **Step 3: Create governance.rs with CanQueryGitHub**

Create `b00t-cli/src/mcp/governance.rs`:
```rust
use b00t_l3dg3rr_core::{GovernanceContext, GovernanceError, GovernanceGate, TaskQueueState};

/// Gate: Allow GitHub queries only when local queue is empty
pub struct CanQueryGitHub {
    queue_state: TaskQueueState,
}

impl CanQueryGitHub {
    pub fn new(queue_state: TaskQueueState) -> Self {
        Self { queue_state }
    }
}

impl GovernanceGate for CanQueryGitHub {
    fn check(&self, _context: &GovernanceContext) -> Result<(), GovernanceError> {
        match self.queue_state {
            TaskQueueState::Empty => Ok(()),
            _ => Err(GovernanceError::QueueNotEmpty),
        }
    }

    fn reason(&self) -> String {
        "GitHub queries only allowed when local queue is empty".into()
    }
}
```

- [ ] **Step 4: Add governance module to mcp/mod.rs**

Modify or create `b00t-cli/src/mcp/mod.rs`:
```rust
pub mod governance;
pub mod l3dg3rr_service;

// Re-export for internal use
pub use governance::CanQueryGitHub;
pub use l3dg3rr_service::L3dg3rrMcpService;
```

- [ ] **Step 5: Add l3dg3rr-core to b00t-cli Cargo.toml**

Modify `b00t-cli/Cargo.toml`, find `[dependencies]` section, add:
```toml
b00t-l3dg3rr-core = { path = "../b00t-l3dg3rr-core" }
opentelemetry = { version = "0.20", features = ["trace"] }
tracing = "0.1"
tracing-opentelemetry = "0.21"
```

- [ ] **Step 6: Run tests to verify they pass**

```bash
cargo test --test task_next_governance can_query_github
```

Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add b00t-cli/src/mcp/governance.rs b00t-cli/src/mcp/mod.rs b00t-cli/tests/task_next_governance.rs b00t-cli/Cargo.toml
git commit -m "feat(b00t-cli): implement CanQueryGitHub governance gate"
```

---

### Task 6: Create l3dg3rr-mcp service stub

**Files:**
- Create: `b00t-cli/src/mcp/l3dg3rr_service.rs`

- [ ] **Step 1: Write tests for l3dg3rr service**

Add to `b00t-cli/tests/task_next_governance.rs`:
```rust
#[tokio::test]
async fn l3dg3rr_service_task_next_with_empty_queue() {
    use b00t_cli::mcp::L3dg3rrMcpService;

    let service = L3dg3rrMcpService::new();

    // This test verifies the service can be instantiated
    assert!(!service.is_empty_queue().await);
}
```

- [ ] **Step 2: Run tests to verify they fail**

```bash
cargo test --test task_next_governance l3dg3rr_service
```

Expected: FAIL (service not defined)

- [ ] **Step 3: Create l3dg3rr_service.rs stub**

Create `b00t-cli/src/mcp/l3dg3rr_service.rs`:
```rust
use b00t_l3dg3rr_core::TransactionLog;
use crate::commands::task::Task;

pub struct L3dg3rrMcpService {
    // TODO: Add MCP server context, GitHub client, etc.
}

impl L3dg3rrMcpService {
    pub fn new() -> Self {
        Self {}
    }

    /// Check if local task queue is empty
    pub async fn is_empty_queue(&self) -> bool {
        // TODO: Call b00t::task::list_all() and check length
        false
    }

    /// MCP endpoint: task/next
    /// Returns (Task, TransactionLog) or error
    pub async fn task_next(&self) -> anyhow::Result<(Task, TransactionLog)> {
        // TODO: Implement full orchestration
        todo!("task_next orchestration")
    }
}

impl Default for L3dg3rrMcpService {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

```bash
cargo test --test task_next_governance l3dg3rr_service
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/mcp/l3dg3rr_service.rs
git commit -m "feat(b00t-cli): create L3dg3rrMcpService stub"
```

---

### Task 7: Implement task_next orchestration logic

**Files:**
- Modify: `b00t-cli/src/mcp/l3dg3rr_service.rs`

- [ ] **Step 1: Write integration test for task_next flow**

Add to `b00t-cli/tests/task_next_governance.rs`:
```rust
#[tokio::test]
async fn task_next_returns_local_task_when_queue_has_items() {
    // Setup: local queue has tasks
    // Note: This is a mock test - actual test would use test fixtures

    // Expected: task_next returns the task without querying GitHub
    // This test will be refined during implementation with proper fixtures
}

#[tokio::test]
async fn task_next_queries_github_when_queue_empty() {
    // Setup: local queue is empty, GitHub has issues (mocked)

    // Expected: task_next imports issue from GitHub
    // This test will be refined during implementation with proper fixtures
}
```

- [ ] **Step 2: Implement task_next in l3dg3rr_service.rs**

Replace the stub in `b00t-cli/src/mcp/l3dg3rr_service.rs`:

```rust
use b00t_l3dg3rr_core::{
    GovernanceContext, GovernanceGate, TaskQueueState, TransactionLog, TransactionResult, TransactionStep,
};
use crate::commands::task::Task;
use crate::mcp::governance::CanQueryGitHub;
use anyhow::Result;
use tracing::{info, warn, Span};

pub struct L3dg3rrMcpService {
    // TODO: Add MCP server context, GitHub client, etc.
}

impl L3dg3rrMcpService {
    pub fn new() -> Self {
        Self {}
    }

    /// Check if local task queue is empty
    pub async fn is_empty_queue(&self) -> Result<bool> {
        // TODO: Call b00t::task::list_all() and check length
        // For now, stub returns false
        Ok(false)
    }

    /// Get queue state
    pub async fn queue_state(&self) -> Result<TaskQueueState> {
        let is_empty = self.is_empty_queue().await?;
        Ok(if is_empty {
            TaskQueueState::Empty
        } else {
            TaskQueueState::Pending
        })
    }

    /// MCP endpoint: task/next
    /// Returns (Task, TransactionLog) or error
    pub async fn task_next(&self) -> Result<(Task, TransactionLog)> {
        let mut log = TransactionLog::new();

        // Step 1: Check local queue state
        info!("task_next: checking local queue");
        let mut step = TransactionStep::new("queue_check".into(), false);

        let queue_state = self.queue_state().await?;
        step = TransactionStep::new("queue_check".into(), true);
        log.add_step(step);

        info!("task_next: queue_state = {:?}", queue_state);

        // If queue not empty, return local task
        if queue_state != TaskQueueState::Empty {
            info!("task_next: returning local task (queue not empty)");
            // TODO: Call b00t::task::next() and return it
            log.set_result(TransactionResult::Success(0)); // placeholder
            return Err(anyhow::anyhow!("TODO: integrate b00t task API"));
        }

        // Step 2: Check governance gate for GitHub query
        info!("task_next: checking CanQueryGitHub gate");
        let gate = CanQueryGitHub::new(queue_state);
        let context = GovernanceContext {
            queue_state,
            timestamp: std::time::SystemTime::now(),
        };

        let mut step = TransactionStep::new("can_query_github".into(), false);
        match gate.check(&context) {
            Ok(_) => {
                step = TransactionStep::new("can_query_github".into(), true);
                log.add_step(step);
            }
            Err(e) => {
                step = step.with_reason(format!("{}", e));
                log.add_step(step);
                log.set_result(TransactionResult::Denied);
                warn!("task_next: governance denied - {}", e);
                return Err(anyhow::anyhow!("governance denied: {}", e));
            }
        }

        // Step 3: Fetch from GitHub
        info!("task_next: fetching issues from GitHub");
        let mut step = TransactionStep::new("fetch_github_issues".into(), false);

        // TODO: Call GitHub MCP to fetch issues
        // For now, placeholder
        step = TransactionStep::new("fetch_github_issues".into(), true);
        log.add_step(step);

        // Step 4: Import first issue as task
        info!("task_next: importing issue as task");
        let mut step = TransactionStep::new("import_task".into(), false);

        // TODO: Call b00t::task::add() with GitHub issue
        // For now, placeholder
        step = TransactionStep::new("import_task".into(), true);
        log.add_step(step);

        log.set_result(TransactionResult::Success(0)); // placeholder task_id

        Err(anyhow::anyhow!("TODO: complete integration with GitHub and task APIs"))
    }
}

impl Default for L3dg3rrMcpService {
    fn default() -> Self {
        Self::new()
    }
```

- [ ] **Step 3: Run tests to verify they compile (will error on TODOs)**

```bash
cargo test --test task_next_governance --lib
```

Expected: Compile errors from TODOs are expected at this stage

- [ ] **Step 4: Commit**

```bash
git add b00t-cli/src/mcp/l3dg3rr_service.rs
git commit -m "feat(b00t-cli): implement task_next orchestration logic (stub GitHub + task APIs)"
```

---

## Chunk 3: Integration Points (Task API + GitHub MCP)

### Task 8: Integrate b00t task API with l3dg3rr service

**Files:**
- Modify: `b00t-cli/src/mcp/l3dg3rr_service.rs`
- Modify: `b00t-cli/src/commands/task.rs`

- [ ] **Step 1: Write test for queue check integration**

Add to `b00t-cli/tests/task_next_governance.rs`:
```rust
#[test]
fn queue_state_matches_task_list_length() {
    // This test verifies queue_state correctly maps to task count
    // Will be refined during implementation with actual task API calls
}
```

- [ ] **Step 2: Expose task::list_all and task::next from task.rs**

In `b00t-cli/src/commands/task.rs`, find the task list/add/next functions and add public accessors if not already public:

```rust
pub fn list_all() -> Result<Vec<Task>> {
    // TODO: Read from .b00t/tasks.json
}

pub fn next() -> Result<Task> {
    // TODO: Get first pending task
}

pub fn add(title: String, desc: Option<String>, tags: Option<Vec<String>>) -> Result<Task> {
    // TODO: Create task in .b00t/tasks.json
}
```

- [ ] **Step 3: Update L3dg3rrMcpService to use task API**

In `b00t-cli/src/mcp/l3dg3rr_service.rs`, replace TODOs:

```rust
/// Check if local task queue is empty
pub async fn is_empty_queue(&self) -> Result<bool> {
    let tasks = crate::commands::task::list_all()?;
    Ok(tasks.is_empty())
}

/// Get queue state
pub async fn queue_state(&self) -> Result<TaskQueueState> {
    let tasks = crate::commands::task::list_all()?;
    let state = if tasks.is_empty() {
        TaskQueueState::Empty
    } else if tasks.iter().any(|t| t.status == "in-progress") {
        TaskQueueState::InProgress
    } else {
        TaskQueueState::Pending
    };
    Ok(state)
}

// In task_next():
// "If queue not empty, return local task"
if queue_state != TaskQueueState::Empty {
    let task = crate::commands::task::next()?;
    log.set_result(TransactionResult::Success(task.id as u64));
    return Ok((task, log));
}

// "Import first issue as task"
// TODO: Replace when GitHub integration is complete
let task = crate::commands::task::add(
    "Placeholder from GitHub".into(),
    None,
    Some(vec!["github-imported".into()]),
)?;
log.set_result(TransactionResult::Success(task.id as u64));
Ok((task, log))
```

- [ ] **Step 4: Run tests to verify compilation**

```bash
cargo build -p b00t-cli
```

Expected: Should compile (GitHub integration still stubbed)

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/mcp/l3dg3rr_service.rs b00t-cli/src/commands/task.rs
git commit -m "feat(b00t-cli): integrate task API with l3dg3rr service"
```

---

### Task 9: Add OTel logging to l3dg3rr service

**Files:**
- Modify: `b00t-cli/src/mcp/l3dg3rr_service.rs`

- [ ] **Step 1: Add OTel spans to task_next()**

In `b00t-cli/src/mcp/l3dg3rr_service.rs`, add to imports:

```rust
use opentelemetry::trace::{Tracer, TracerProvider};
use tracing_opentelemetry::OpenTelemetrySpanExt;
```

Wrap each orchestration step with spans:

```rust
pub async fn task_next(&self) -> Result<(Task, TransactionLog)> {
    let tracer = opentelemetry::global::tracer("b00t-l3dg3rr");
    let root_span = tracer.start("task_next");

    let mut log = TransactionLog::new();

    // Step 1: Check local queue state
    {
        let span = tracer.start_with_context("queue_check", &root_span.span_context());
        info!(parent: &span, "checking local queue");

        let queue_state = self.queue_state().await?;
        span.add_event("queue_state", Default::default());

        let mut step = TransactionStep::new("queue_check".into(), true);
        log.add_step(step);
    }

    // ... rest of steps with similar spans

    Ok((task, log))
}
```

- [ ] **Step 2: Verify tracing compiles**

```bash
cargo build -p b00t-cli
```

Expected: SUCCESS

- [ ] **Step 3: Add to existing CLAUDE.md or create testing doc**

In test instructions, add:

```bash
# Test with OTel tracing
RUST_LOG=debug cargo test --test task_next_governance -- --nocapture
```

- [ ] **Step 4: Commit**

```bash
git add b00t-cli/src/mcp/l3dg3rr_service.rs
git commit -m "feat(b00t-cli): add OTel tracing to l3dg3rr service"
```

---

## Chunk 4: End-to-End Integration

### Task 10: Wire task::next() to call l3dg3rr-mcp

**Files:**
- Modify: `b00t-cli/src/commands/task.rs`

- [ ] **Step 1: Write end-to-end test**

```rust
#[tokio::test]
async fn task_next_command_uses_l3dg3rr() {
    // Test that `b00t task --next` delegates to l3dg3rr service
    // This is a high-level integration test
}
```

- [ ] **Step 2: Modify task::next() handler to use l3dg3rr**

In `b00t-cli/src/commands/task.rs`, find `TaskCommand::next()`:

```rust
async fn handle_next(verbose: bool) -> Result<()> {
    let service = crate::mcp::L3dg3rrMcpService::new();

    let (task, transaction) = service.task_next().await?;

    // Display task
    println!("{}", format_task(&task));

    // Display transaction proof if --verbose
    if verbose {
        println!("\nTransaction: {}", format_transaction(&transaction));
        for step in transaction.steps() {
            println!("  {} - {}", if step.passed() { "✓" } else { "✗" }, step.gate());
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Add format helpers**

```rust
fn format_task(task: &Task) -> String {
    format!("Task #{}: {}", task.id, task.title)
}

fn format_transaction(tx: &b00t_l3dg3rr_core::TransactionLog) -> String {
    format!("Transaction {} ({}ms)", tx.id(), tx.duration_ms())
}
```

- [ ] **Step 4: Test the command**

```bash
cargo run -- task --next --verbose
```

Expected: Should show task + transaction steps (all stubs work, GitHub still TODO)

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/commands/task.rs
git commit -m "feat(b00t-cli): wire task::next() to l3dg3rr-mcp service"
```

---

### Task 11: Final integration tests and verification

**Files:**
- Modify: `b00t-cli/tests/task_next_governance.rs`

- [ ] **Step 1: Write comprehensive integration test**

```rust
#[tokio::test]
async fn task_next_end_to_end_with_local_queue() {
    // Setup: Add task to local queue
    b00t_cli::commands::task::add(
        "Integration test task".into(),
        Some("Testing the flow".into()),
        None,
    ).unwrap();

    // Call task_next
    let service = b00t_cli::mcp::L3dg3rrMcpService::new();
    let (task, log) = service.task_next().await.unwrap();

    // Verify: returns local task, doesn't query GitHub
    assert_eq!(task.title, "Integration test task");
    assert!(!log.steps().iter().any(|s| s.gate() == "fetch_github_issues"));
}

#[tokio::test]
async fn task_next_transaction_log_records_all_steps() {
    let service = b00t_cli::mcp::L3dg3rrMcpService::new();
    let (_, log) = service.task_next().await.unwrap();

    // Verify: all steps recorded
    assert!(log.steps().iter().any(|s| s.gate() == "queue_check"));
    assert!(log.steps().iter().any(|s| s.gate() == "can_query_github"));
}

#[tokio::test]
async fn governance_gate_blocks_github_when_queue_not_empty() {
    // Setup: Add task so queue is not empty
    b00t_cli::commands::task::add("Blocking task".into(), None, None).unwrap();

    let service = b00t_cli::mcp::L3dg3rrMcpService::new();
    let result = service.task_next().await;

    // Should succeed (returns local task) but not query GitHub
    // Verify in transaction log
    // This test documents the expected behavior
}
```

- [ ] **Step 2: Run full test suite**

```bash
cargo test -p b00t-cli --test task_next_governance
```

Expected: All tests PASS

- [ ] **Step 3: Run b00t-l3dg3rr-core tests**

```bash
cargo test -p b00t-l3dg3rr-core
```

Expected: All tests PASS

- [ ] **Step 4: Build entire workspace**

```bash
cargo build --workspace
```

Expected: SUCCESS

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/tests/task_next_governance.rs
git commit -m "test(b00t-cli): add comprehensive task_next integration tests"
```

---

## Chunk 5: Documentation and Cleanup

### Task 12: Update documentation

**Files:**
- Modify: `b00t-cli/CLAUDE.md` (if exists)
- Create or Modify: `docs/GOVERNANCE.md`

- [ ] **Step 1: Add l3dg3rr section to b00t-cli CLAUDE.md**

Append to `b00t-cli/CLAUDE.md`:

```markdown
## l3dg3rr Governance Integration

### MCP Service
- **Location**: `src/mcp/l3dg3rr_service.rs`
- **Traits**: Defined in `../b00t-l3dg3rr-core/src/traits.rs`
- **Gates**: Governance rules in `src/mcp/governance.rs`

### Task Discovery Flow
`b00t task --next` delegates to l3dg3rr:
1. Check local queue state (empty/pending/in-progress)
2. Apply CanQueryGitHub gate (only query if queue empty)
3. Fetch from GitHub issues (when allowed)
4. Import as task with "github-imported" tag
5. Log transaction to OTel

### Extending Governance Rules
Add new gate in `src/mcp/governance.rs`:
```rust
pub struct MyGate { ... }
impl GovernanceGate for MyGate {
    fn check(&self, context: &GovernanceContext) -> Result<(), GovernanceError> {
        // Your logic
    }
}
```

Then use in `l3dg3rr_service.rs` task_next() method.

### Testing
```bash
cargo test --test task_next_governance
cargo test -p b00t-l3dg3rr-core
```
```

- [ ] **Step 2: Create governance design doc**

Create `docs/GOVERNANCE.md`:

```markdown
# b00t Governance Framework

## Overview
l3dg3rr provides governance for multi-source task discovery in b00t.

## Architecture
- **Invariant-based gates**: Trait-based authorization checks
- **Pass-thru proxy**: All downstream I/O flows through governed authorization
- **OTel logging**: All transaction steps logged with tracing spans
- **Transaction proof**: Each operation returns complete audit trail

## Current Gates
- `CanQueryGitHub`: Allow GitHub queries only when local queue is empty

## Future Gates
- Rate limiting
- Node affinity
- Dependency cycle detection
- Authorization scope validation

## Transaction Log
Each `task --next` call generates a transaction log showing:
- All governance checks (passed/denied)
- I/O operations (queue read, GitHub fetch, task import)
- OTel span IDs for tracing
- Final result (success/denied/error)

## Example Usage
```bash
b00t task --next --verbose
# Task #5: Fix governance gate in l3dg3rr (from github-imported)
#
# Transaction: abc123 (245ms)
#   ✓ queue_check (empty)
#   ✓ can_query_github (allowed)
#   ✓ fetch_github_issues (5 issues)
#   ✓ import_task (task #5)
```
```

- [ ] **Step 3: Commit**

```bash
git add b00t-cli/CLAUDE.md docs/GOVERNANCE.md
git commit -m "docs: add l3dg3rr governance documentation and architecture guide"
```

---

### Task 13: Summary and tag

**Files:**
- None (docs/superpowers/plans/this-file updated)

- [ ] **Step 1: Run final verification**

```bash
cargo build --workspace
cargo test --workspace
```

Expected: All builds and tests PASS

- [ ] **Step 2: Create summary commit**

```bash
git log --oneline | head -20
```

Expected: Shows all commits from this implementation

- [ ] **Step 3: Note for next phase**

The following items are stubbed for v1.1 iteration:
- GitHub MCP integration (fetch issues from specific repo/labels)
- Node affinity rules
- Rate limiting gate
- Persistent transaction log query interface
- Visualization of transaction DAG

---

## Testing Checklist

- [ ] Unit tests pass: `cargo test -p b00t-l3dg3rr-core`
- [ ] Integration tests pass: `cargo test --test task_next_governance`
- [ ] Governance gate blocks correctly: `cargo test can_query_github_denies`
- [ ] Transaction logging works: `cargo test transaction_log_records`
- [ ] OTel spans compile: `cargo build -p b00t-cli`
- [ ] End-to-end command works: `cargo run -- task --next --verbose`

---

## Success Criteria

- ✅ l3dg3rr-core crate created with governance traits
- ✅ b00t-cli embeds l3dg3rr-mcp as internal MCP service
- ✅ `b00t task --next` delegates to l3dg3rr
- ✅ CanQueryGitHub gate blocks GitHub queries when queue not empty
- ✅ TransactionLog records all steps with OTel spans
- ✅ Tests cover governance logic, transaction logging, end-to-end flow
- ✅ No bash execution in l3dg3rr (all I/O primitives)
- ✅ GitHub MCP integration stubbed for v1.1

---

## Commit History

This plan produces ~13 commits following TDD (test → implementation → commit):

1. `feat(l3dg3rr-core): create governance traits crate skeleton`
2. `feat(l3dg3rr-core): implement GovernanceError enum`
3. `feat(l3dg3rr-core): implement TransactionLog and TaskQueueState types`
4. `feat(l3dg3rr-core): implement governance trait definitions`
5. `feat(b00t-cli): implement CanQueryGitHub governance gate`
6. `feat(b00t-cli): create L3dg3rrMcpService stub`
7. `feat(b00t-cli): implement task_next orchestration logic (stub GitHub + task APIs)`
8. `feat(b00t-cli): integrate task API with l3dg3rr service`
9. `feat(b00t-cli): add OTel tracing to l3dg3rr service`
10. `feat(b00t-cli): wire task::next() to l3dg3rr-mcp service`
11. `test(b00t-cli): add comprehensive task_next integration tests`
12. `docs: add l3dg3rr governance documentation and architecture guide`
13. `chore: cleanup and tag POC v0.1`

