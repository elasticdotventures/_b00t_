# b00t task --next with l3dg3rr Governance POC

**Date**: 2026-05-02
**Status**: Design (POC)
**Author**: @elasticdotventures (Operator)
**Scope**: Proof of concept for governance-driven multi-source task discovery

---

## Executive Summary

Enable `b00t task --next` to search multiple task sources (local queue → GitHub issues) with **governance enforcement at the boundary**. b00t embeds a governance proxy layer (l3dg3rr-mcp, an internal MCP service) that validates state transitions, checks authorization, logs all I/O to OpenTelemetry, and returns transaction proof.

**Key design principle**: Invariant-based governance using Rust traits. Rules are codified as trait implementations, making them testable, composable, and extensible. **b00t's l3dg3rr-mcp acts as a pass-thru/proxy governance layer**: all downstream service calls (GitHub, task APIs, OTel) flow through governed authorization gates before execution.

---

## Problem Statement

Currently, `b00t task --next` only searches the local `.b00t/tasks.json` queue. When the queue is empty, Operator must manually check GitHub issues and import them.

**Desired state**: Automate this:
1. Check local queue
2. If empty, query GitHub (with authorization check)
3. Import first issue as task
4. Return result with transaction proof (for auditing/visualization)

**Governance constraint**: All I/O primitives and decisions must flow through l3dg3rr, logged to OTel, with state invariants enforced at each step.

---

## Architecture

### Three-Layer Design

```
┌─────────────────────────────────────────────────────┐
│ b00t-cli (surface)                                  │
│ ├─ task --next                                      │
│ └─ calls b00t's l3dg3rr-mcp (internal)              │
└─────────────────────┬───────────────────────────────┘
                      │
┌─────────────────────▼───────────────────────────────────────────┐
│ b00t (integrated system)                                        │
│ ├─ b00t-cli (frontend)                                          │
│ ├─ task subsystem (CRUD)                                        │
│ ├─ l3dg3rr-mcp (internal MCP service)                          │
│ │  └─ task/next RPC endpoint                                   │
│ │  ├─ orchestrates: queue check → gate check → proxy to services
│ │  ├─ logs all decisions to OTel                               │
│ │  └─ returns (Task, TransactionProof)                         │
│ └─ passes requests to downstream services via l3dg3rr governance
└─────────────────────┬───────────────────────────────────────────┘
                      │
        ┌─────────────┼─────────────┐
        │             │             │
    ┌───▼──┐   ┌─────▼─────┐   ┌──▼────┐
    │Local │   │ GitHub    │   │OTel   │
    │Queue │   │ MCP       │   │Logging│
    │APIs  │   │(proxied)  │   │(built-in)
    └──────┘   └───────────┘   └───────┘
```

**Architecture**:

1. **b00t-cli** — CLI surface, thin wrapper
   - Calls `b00t's internal l3dg3rr-mcp.task_next()` via local MCP
   - Displays result + transaction proof
   - No business logic

2. **l3dg3rr-core** — Governance primitives (shared library)
   - Traits for invariants and gates
   - Transaction logging
   - Solver (future: constraint satisfaction)
   - No I/O, no service coupling
   - Used by both b00t and l3dg3rr

3. **b00t's l3dg3rr-mcp** — Internal MCP service (defined by b00t)
   - Implements `task/next` RPC endpoint
   - **Pass-thru/proxy governance layer**: all downstream calls (GitHub, task APIs, OTel) flow through governed authorization
   - Calls b00t's task APIs (queue read, task import)
   - Proxies GitHub MCP calls through governance gates
   - Orchestrates state transitions
   - Logs to OTel
   - Returns task + transaction proof

---

## Core Components

### 1. l3dg3rr-core Traits

#### `TaskQueueInvariant`

```rust
pub trait TaskQueueInvariant {
    fn state(&self) -> TaskQueueState;
    fn is_valid_transition(&self, from: TaskQueueState, to: TaskQueueState) -> bool;
}

pub enum TaskQueueState {
    Empty,
    Pending,
    InProgress,
}
```

Invariant: `can_query_github() → state == Empty`

#### `GovernanceGate`

```rust
pub trait GovernanceGate {
    fn check(&self, context: &GovernanceContext) -> Result<(), GovernanceError>;
    fn reason(&self) -> String; // why was this gate added?
}

pub struct GovernanceContext {
    queue_state: TaskQueueState,
    source: TaskSource, // Local, GitHub, etc.
    timestamp: Instant,
}

pub enum GovernanceError {
    QueueNotEmpty,
    RateLimited,
    Unauthorized,
}
```

#### `TransactionLog`

```rust
pub struct TransactionLog {
    id: Uuid,                        // transaction ID
    steps: Vec<TransactionStep>,
    root_span: OTelSpanId,
    result: TransactionResult,
}

pub struct TransactionStep {
    gate: String,              // "queue_check", "can_query_github", etc.
    passed: bool,
    reason: Option<String>,
    otel_span_id: OTelSpanId,
}

pub enum TransactionResult {
    Success(TaskId),
    Denied(GovernanceError),
    Error(String),
}
```

### 2. b00t-cli Integration

Modify `b00t-cli/src/commands/task.rs`:

```rust
// In TaskCommand::next()
async fn handle_next() -> Result<Task> {
    // Call l3dg3rr MCP
    let (task, transaction) = l3dg3rr_client
        .task_next()
        .await?;

    // Display task
    println!("{}", format_task(&task));

    // Display transaction proof (opt-in: --verbose)
    if verbose {
        println!("\nTransaction: {}", format_transaction(&transaction));
    }

    Ok(task)
}
```

### 3. l3dg3rr MCP Endpoint: `task/next`

```rust
// l3dg3rr-mcp/src/endpoints/task.rs

pub async fn task_next(
    ctx: &ServerContext,
) -> Result<(Task, TransactionLog)> {
    let mut log = TransactionLog::new();
    let root_span = ctx.otel.start_span("task_next");

    // Step 1: Check local queue
    log.add_step(Step {
        gate: "queue_check".into(),
        span: ctx.otel.start_span("queue_check"),
    });
    let queue_state = b00t::task::list_all()?.len() == 0;
    log.steps.last_mut().passed = queue_state;

    if !queue_state {
        // Queue not empty, return local task
        let task = b00t::task::next()?;
        log.result = TransactionResult::Success(task.id());
        return Ok((task, log));
    }

    // Step 2: Check governance gate
    log.add_step(Step {
        gate: "can_query_github".into(),
        span: ctx.otel.start_span("can_query_github"),
    });
    let gate = CanQueryGitHub::new(queue_state);
    match gate.check(&GovernanceContext::default()) {
        Ok(_) => log.steps.last_mut().passed = true,
        Err(e) => {
            log.result = TransactionResult::Denied(e.clone());
            log.end_span(root_span);
            return Err(e.into());
        }
    }

    // Step 3: Fetch from GitHub
    log.add_step(Step {
        gate: "fetch_github_issues".into(),
        span: ctx.otel.start_span("fetch_github_issues"),
    });
    let issues = github_mcp::fetch_issues(
        &ctx.github_client,
        "elasticdotventures/_b00t_",
        Default::default(),
    ).await?;
    log.steps.last_mut().passed = !issues.is_empty();

    if issues.is_empty() {
        log.result = TransactionResult::Error("no issues found".into());
        log.end_span(root_span);
        return Err("no github issues found".into());
    }

    // Step 4: Import first issue as task
    log.add_step(Step {
        gate: "import_task".into(),
        span: ctx.otel.start_span("import_task"),
    });
    let issue = &issues[0];
    let task = b00t::task::add(
        issue.title.clone(),
        Some(issue.body.clone()),
        Some("github-imported".into()),
    )?;
    log.steps.last_mut().passed = true;

    log.result = TransactionResult::Success(task.id());
    log.end_span(root_span);

    Ok((task, log))
}
```

---

## Data Flow: `b00t task --next`

```
User: b00t task --next --verbose
  ↓
b00t-cli calls l3dg3rr MCP: task_next()
  ↓
l3dg3rr orchestrates:
  1. OTel span: "task_next" (root)
     ├─ OTel span: "queue_check"
     │  ├─ Call b00t::task::list_all()
     │  ├─ Log: queue_state = "empty"
     │  └─ Passed: true
     │
     ├─ OTel span: "can_query_github"
     │  ├─ Check CanQueryGitHub gate
     │  ├─ Invariant: queue_state == Empty
     │  └─ Passed: true
     │
     ├─ OTel span: "fetch_github_issues"
     │  ├─ Call GitHub MCP: fetch_issues(...)
     │  ├─ Log: fetched 5 issues
     │  └─ Passed: true
     │
     └─ OTel span: "import_task"
        ├─ Call b00t::task::add(issue #123, tag: "github-imported")
        ├─ Log: imported task #9 from github issue #123
        └─ Passed: true
  ↓
Return: (Task #9, TransactionLog { 4 steps, all passed })
  ↓
b00t-cli displays:
  Task #9: "Fix: authorize l3dg3rr in task governance" (from github-imported)

  [--verbose shows:]
  Transaction: xxxxxxxx-xxxx-xxxx (took 245ms)
    ✓ queue_check → empty
    ✓ can_query_github → allowed (queue_state == empty)
    ✓ fetch_github_issues → 5 issues
    ✓ import_task → task #9 from github #123
```

---

## Governance Rules (POC v1)

### Rule: `CanQueryGitHub`

```rust
pub struct CanQueryGitHub {
    queue_empty: bool,
}

impl GovernanceGate for CanQueryGitHub {
    fn check(&self, _ctx: &GovernanceContext) -> Result<()> {
        if !self.queue_empty {
            return Err(GovernanceError::QueueNotEmpty);
        }
        Ok(())
    }

    fn reason(&self) -> String {
        "GitHub queries only allowed when local queue is empty".into()
    }
}
```

**Future rules** (iteration):
- Rate limiting: max 10 GitHub queries per hour
- Authorization: require `GITHUB_TOKEN` with `repo:read` scope
- Node affinity: only query if assigned to node with `github-connector` capability
- Cycle detection: reject issues that would create task dependency cycles

---

## Error Handling

**Governance errors** → deny, log reason, surface to user:
```
$ b00t task --next
Error: governance denied: queue not empty
  Reason: GitHub queries only allowed when local queue is empty
  Steps:
    ✓ queue_check → 3 pending tasks
    ✗ can_query_github → DENIED
```

**I/O errors** (GitHub API, b00t task read/write) → log step as failed, return error:
```
Error: github fetch failed (otel span: abc123)
  Step: fetch_github_issues failed after 2.3s
  Reason: GitHub API rate limit exceeded
  (Check OTel dashboard for full trace)
```

---

## Testing Strategy

### Unit Tests (l3dg3rr-core)

```rust
#[test]
fn can_query_github_allows_when_queue_empty() {
    let gate = CanQueryGitHub { queue_empty: true };
    assert!(gate.check(&GovernanceContext::default()).is_ok());
}

#[test]
fn can_query_github_denies_when_queue_not_empty() {
    let gate = CanQueryGitHub { queue_empty: false };
    assert_eq!(
        gate.check(&GovernanceContext::default()),
        Err(GovernanceError::QueueNotEmpty)
    );
}

#[test]
fn transaction_log_records_all_steps() {
    let mut log = TransactionLog::new();
    log.add_step(Step { gate: "check1".into(), passed: true, ... });
    log.add_step(Step { gate: "check2".into(), passed: false, ... });
    assert_eq!(log.steps.len(), 2);
    assert_eq!(log.result, TransactionResult::Denied(...));
}
```

### Integration Tests (l3dg3rr MCP)

```rust
#[tokio::test]
async fn task_next_returns_local_task_when_queue_not_empty() {
    // Setup: local queue has tasks
    b00t::task::add("task 1", None, None).unwrap();

    let (task, log) = task_next(&ctx).await.unwrap();

    assert_eq!(task.title(), "task 1");
    assert_eq!(log.steps[0].gate, "queue_check");
    assert!(log.steps[0].passed);
}

#[tokio::test]
async fn task_next_queries_github_when_queue_empty() {
    // Setup: local queue empty, GitHub has issues (mocked)
    let github_mock = MockGitHubClient::with_issues(vec![
        Issue { id: 123, title: "Fix bug".into(), ... },
    ]);

    let (task, log) = task_next_with_github(&ctx, github_mock).await.unwrap();

    assert_eq!(task.title(), "Fix bug");
    assert_eq!(task.tag(), Some("github-imported"));
    assert_eq!(log.steps[2].gate, "fetch_github_issues");
    assert!(log.steps[2].passed);
}

#[tokio::test]
async fn task_next_denies_github_query_when_queue_not_empty() {
    // Setup: local queue has tasks, GitHub available
    b00t::task::add("local task", None, None).unwrap();

    let (task, log) = task_next(&ctx).await.unwrap();

    // Should return local task, never query GitHub
    assert_eq!(task.title(), "local task");
    assert!(!log.steps.iter().any(|s| s.gate == "fetch_github_issues"));
}
```

### End-to-End Tests (b00t-cli)

```bash
# Test 1: queue has tasks, returns local task
$ b00t task add "local task"
$ b00t task --next
# Expected: returns "local task"

# Test 2: queue empty, GitHub has issues, imports and returns
$ rm .b00t/tasks.json
$ b00t task --next --verbose
# Expected: imports from GitHub, shows transaction proof

# Test 3: OTel logging works
$ OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4318 \
  b00t task --next --verbose
# Expected: OTel spans appear in observability backend
```

---

## Dependencies

### New Crates

- **l3dg3rr-core** (new, standalone) — Governance traits, no I/O, minimal deps
  - `serde` (task serialization)
  - `uuid` (transaction IDs)
  - `thiserror` (error types)

### Modified Crates

- **b00t-cli** — Add l3dg3rr-mcp internal MCP service
  - Adds `modelcontextprotocol` dep (MCP service definition)
  - Adds `opentelemetry` + `tracing` (OTel logging)
  - Already has: `tokio`, `serde`, task CRUD
  - Extends `src/` with:
    - `mcp/l3dg3rr_service.rs` — task/next endpoint + governance orchestration
    - `mcp/governance.rs` — gate implementations
  - CLI calls internal l3dg3rr-mcp service instead of external endpoint

### Existing Dependencies

- **GitHub MCP** — assumed available, called via l3dg3rr governance proxy
- **OTel stack** — assumed available (collector, exporter)
- **b00t task APIs** — already exist, used by l3dg3rr-mcp for queue state

---

## Integration Points

### 1. b00t-cli → l3dg3rr MCP

```
b00t task --next
  └─ Call l3dg3rr MCP: POST /task/next
     └─ Response: { task, transaction_log }
```

**Interface**:
```json
{
  "method": "task_next",
  "params": {
    "verbose": false
  }
}
```

**Response**:
```json
{
  "task": {
    "id": 9,
    "title": "Fix: authorize l3dg3rr",
    "source": "github-imported",
    "gh_issue_id": 123
  },
  "transaction": {
    "id": "xxxxxxxx-xxxx",
    "steps": [
      { "gate": "queue_check", "passed": true },
      { "gate": "can_query_github", "passed": true },
      { "gate": "fetch_github_issues", "passed": true },
      { "gate": "import_task", "passed": true }
    ],
    "duration_ms": 245
  }
}
```

### 2. l3dg3rr MCP → b00t (library calls)

```rust
// Read queue state
b00t::task::list_all() -> Vec<Task>

// Import new task
b00t::task::add(title, description, tags) -> Task
```

### 3. l3dg3rr MCP → GitHub MCP

```
github_mcp::fetch_issues(repo, query) -> Vec<Issue>
```

### 4. l3dg3rr → OTel

```rust
otel.start_span("queue_check");
otel.log_event("queue_check", {"queue_size": 0, "state": "empty"});
otel.end_span("queue_check");
```

---

## Iteration Roadmap (Future)

### POC (v1, this spec)
- ✓ Local queue check
- ✓ GitHub query gate
- ✓ Import with tags
- ✓ OTel logging
- ✓ Transaction proof

### v1.1 (soon)
- [ ] Node affinity rules
- [ ] Rate limiting gate
- [ ] GitHub token scope validation
- [ ] Reject cycles in task dependencies

### v2 (next generation)
- [ ] Solver: constraint satisfaction for rule conflicts
- [ ] Visualization: transaction DAG in b00t UI
- [ ] Multi-source search (GitLab, Linear, Jira)
- [ ] Persistent transaction log with query interface

---

## Success Criteria

- [ ] `b00t task --next` queries GitHub when local queue is empty
- [ ] Governance gate blocks GitHub query if queue not empty
- [ ] All I/O (queue read, GH fetch, task import) logged to OTel
- [ ] Transaction proof shows all steps + outcome
- [ ] Can deny GitHub query and explain why (governance error)
- [ ] Tests cover: gate logic, transaction logging, end-to-end flow
- [ ] No bash execution in l3dg3rr (pure I/O primitives)

---

## Appendix: Example OTel Trace

```
trace_id: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
  root_span: task_next (245ms)
    ├─ span: queue_check (1ms)
    │  ├─ event: "started"
    │  ├─ event: "b00t::task::list_all() → 0 tasks"
    │  ├─ attribute: queue_state = "empty"
    │  └─ event: "passed"
    │
    ├─ span: can_query_github (0.5ms)
    │  ├─ event: "started"
    │  ├─ event: "check(queue_state == empty) → true"
    │  ├─ attribute: gate = "CanQueryGitHub"
    │  └─ event: "passed"
    │
    ├─ span: fetch_github_issues (180ms)
    │  ├─ event: "started"
    │  ├─ event: "github_mcp::fetch_issues(elasticdotventures/_b00t_) called"
    │  ├─ event: "fetched 5 issues"
    │  ├─ attribute: issue_count = 5
    │  └─ event: "passed"
    │
    └─ span: import_task (60ms)
       ├─ event: "started"
       ├─ event: "b00t::task::add('Fix: authorize', tag: 'github-imported')"
       ├─ event: "task_id = 9"
       ├─ attribute: gh_issue_id = 123
       └─ event: "passed"
```

