# Governance Epoch 2 — Implementation Plan

## Architecture

```
b00t-c0re-gov/              # New workspace crate (~/.b00t/b00t-c0re-gov/)
├── Cargo.toml
├── src/
│   ├── lib.rs              # Re-exports
│   ├── types.rs            # HookToken, GateResult, AgentContext
│   ├── traits.rs           # GovernanceGate trait
│   ├── registry.rs         # HookRegistry — register/fire/list/cancel
│   ├── scheduler.rs        # EventScheduler — tokio select! loop
│   ├── store.rs            # ContextStore — file-backed atomic writes
│   ├── ring.rs             # SPSC lock-free ring buffer
│   ├── continuation.rs     # AgentContinuation snapshot/restore
│   └── gates/
│       ├── mod.rs
│       ├── timer.rs        # TimerGate — fire after duration/at instant
│       ├── event.rs        # EventGate — fire when event_id emitted
│       └── pipeline.rs     # CompositeGate — AnyOf/AllOf
└── tests/
    ├── registry_test.rs
    ├── scheduler_test.rs
    └── store_test.rs
```

## Gate Return Type

```rust
pub enum GateResult {
    Allow,
    Deny { reason: String, escalation_path: Option<EscalationPath> },
    Hook(HookToken),
}

pub struct HookToken {
    pub id: Uuid,
    pub hook_type: HookType,
    pub created_at: Instant,
    pub ttl: Option<Duration>,  // None = 2 years (max)
}

pub enum HookType {
    Timer(Duration),
    AtInstant(Instant),
    Event(String),
    AnyOf(Vec<HookToken>),
    AllOf(Vec<HookToken>),
    Cron(String),  // cron expression
    Condition(Box<dyn Fn() -> bool + Send>),  // predicate
}
```

## Context Store

```rust
// File-backed, atomic via tmp+rename
// Path: ~/.local/share/b00t/hooks/<hook_id>.json
pub struct ContextStore { path: PathBuf }

impl ContextStore {
    fn save(&self, token: &HookToken, context: AgentContext) -> Result<()>
    fn load(&self, token: &HookToken) -> Result<Option<AgentContext>>
    fn delete(&self, token: &HookToken) -> Result<()>
    fn list(&self) -> Result<Vec<HookToken>>
}

pub struct AgentContext {
    pub agent_id: String,
    pub task: String,
    pub gate: String,
    pub result_so_far: serde_json::Value,
    pub reasoning: String,
    pub created_at: Instant,
    pub hook_token: HookToken,
    pub continuation: String,  // "what to do next" — agent picks up here
}
```

## SPSC Ring Buffer

```rust
// Lock-free single-producer, single-consumer ring for hook notifications
// Producer: EventScheduler thread
// Consumer: Agent runtime thread
pub struct HookRing<const N: usize> {
    ring: [UnsafeCell<MaybeUninit<HookNotification>>; N],
    head: AtomicU64,
    tail: AtomicU64,
}

pub struct HookNotification {
    pub hook_id: Uuid,
    pub event: HookEvent,
}

pub enum HookEvent {
    Fired,
    Cancelled,
    Expired,
    Error(String),
}
```

## Scheduler Loop

```rust
// Single tokio task that:
// 1. Checks for expired timers
// 2. Listens for event emissions
// 3. Pushes notifications to ring buffer
// 4. Sleeps until next timer expiry (or 1s, whichever is sooner)
Loop {
    let next_timer = registry.next_expiry();
    let sleep_dur = min(next_timer, Duration::from_secs(1));
    tokio::select! {
        _ = tokio::time::sleep(sleep_dur) => {
            let fired = registry.check_timers();
            ring.push_all(fired);
        }
        event = event_rx.recv() => {
            let matched = registry.match_event(event);
            ring.push_all(matched);
        }
    }
}
```

## Implementation Order

1. **types.rs + ring.rs** — Core types and lock-free ring buffer
2. **store.rs** — File-backed context store with tmp+rename
3. **traits.rs + gates/** — GovernanceGate trait + built-in gates
4. **registry.rs + scheduler.rs** — Hook registry and event scheduler
5. **continuation.rs** — AgentContinuation snapshot/restore
6. **Wire into b00t-cli** — Replace blocking HiveGuard with async hook-based gates
7. **Self-host meta-hook** — Governance system monitors its own health

## Design Principles

1. **No agent blocks.** Ever. Gate returns Hook → agent snapshots → goes productive → hook fires → agent restores.
2. **No polling.** Agent registers hook, scheduler fires it. Agent doesn't check back.
3. **Context is cheap.** Filesystem-backed, page-cached. 0.1ms saves, no io_uring needed.
4. **Ring buffer is fast.** Lock-free SPSC for hook notifications. No mutex, no channel overhead.
5. **Atomic writes.** tmp+rename for context store. Crash-safe, always consistent.
