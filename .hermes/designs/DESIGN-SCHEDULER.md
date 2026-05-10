# b00t Scheduler — Multi-Agent Scheduling Subsystem

## Design Philosophy

A distributed scheduling kernel for the b00t hive that agents
register into, not one they depend on. Hermes CLI becomes the
cockpit — a live dashboard showing what every agent is doing.

### Principles

1. **Store is the single source of truth.** A SQLite DB at
   `~/.b00t/scheduler/scheduler.db` is the authoritative record.
   Any agent reads and writes to this store.

2. **Agents come and go; the schedule persists.** The store survives
   agent crashes, machine reboots, and agent technology changes.

3. **Capabilities, not agent types, define routing.**
   Jobs declare what capabilities they need (`grok`, `ontology`,
   `hive`, `terminal`, `bouncer`). The scheduler finds any agent
   that offers the required set — the agent's brand doesn't matter.

4. **Hermes CLI is the cockpit, not the pilot.** Hermes reads the
   shared store and renders it as a live TUI dashboard.

5. **No single-process bottleneck.** There is no central scheduler.
   The "tick" is distributed: any agent claims a due job via
   `BEGIN IMMEDIATE` + `FOR UPDATE SKIP LOCKED`. Winner executes,
   losers move on.

6. **The b00t ecosystem is a federation of capabilities, not one
   adapter.** Every `_b00t_/*.agent.toml` datum and every
   `b00t` CLI subsystem auto-registers as a schedulable capability.
   `b00t grok learn` is not "run via b00t-cli" — it's a capability
   named `grok-learn` that any agent with grok support can claim.

---

## Data Model (SQLite)

```sql
-- ~/.b00t/scheduler/scheduler.db  (WAL mode, immutable foreign keys)

-- A declarative job definition.
CREATE TABLE schedules (
    id           TEXT PRIMARY KEY,          -- "sched_<uuid4_hex>"
    name         TEXT NOT NULL,
    description  TEXT DEFAULT '',

    -- Schedule expression
    schedule_kind     TEXT NOT NULL CHECK (schedule_kind IN ('interval','cron','oneshot')),
    interval_mins     INTEGER,
    cron_expr         TEXT,
    oneshot_at        TEXT,                -- ISO8601 timestamp

    -- Repeat control
    max_runs     INTEGER,                  -- NULL = unlimited
    run_count    INTEGER NOT NULL DEFAULT 0,

    -- ═══════════════════════════════════════════════════════════
    -- Routing: capability-based (preferred) or agent-pin
    -- ═══════════════════════════════════════════════════════════
    required_capabilities TEXT,  -- JSON array: ["grok","ontology","hive"]
                                 -- NULL = no capability filter (any agent qualifies)
    required_agent TEXT,         -- Pin to specific agent_id (overrides capabilities)
                                 -- NULL when capability routing is used
    agent_selector TEXT,         -- Rhai expression, e.g. `pick_least_loaded(agents)`
                                 -- Evaluated after capability filter narrows the pool

    -- Payload
    agent_type   TEXT NOT NULL DEFAULT 'llm',
    -- 'llm'        → runs via LLM agent (Hermes, Claude Code, etc.)
    -- 'b00t'       → runs via b00t subsystem capability (grok-learn, hive-status, etc.)
    -- 'shell'      → runs arbitrary command
    -- 'mcp'        → runs via MCP tool call
    -- 'webhook'    → POST to URL
    agent_config TEXT,  -- JSON: model, skills, toolsets, workdir, env overrides
    prompt       TEXT,  -- Instruction for LLM agents
    command      TEXT,  -- Shell command for 'shell' type agents
    mcp_server   TEXT,  -- MCP server name for 'mcp' type agents
    mcp_tool     TEXT,  -- MCP tool name
    mcp_args     TEXT,  -- JSON args for MCP tool
    webhook_url  TEXT,  -- URL for webhook type
    webhook_body TEXT,  -- JSON body template for webhook
    workdir      TEXT,  -- Absolute path to project directory

    -- Lifecycle
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Immutable run log. Appended by whichever agent executed the job.
CREATE TABLE runs (
    id            TEXT PRIMARY KEY,  -- "run_<uuid4_hex>"
    schedule_id   TEXT NOT NULL REFERENCES schedules(id),
    claimed_by    TEXT NOT NULL,     -- agent_id that claimed this run
    claimed_capability TEXT,         -- which capability executed it (for b00t-type jobs)
    status        TEXT NOT NULL CHECK (status IN ('claimed','running','success','failed','timed_out','cancelled')),
    started_at    TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at   TEXT,
    exit_code     INTEGER,
    output_path   TEXT,  -- path to full output file
    summary       TEXT,  -- truncated output for dashboard
    error         TEXT
);

-- Agent heartbeat / capability registry.
CREATE TABLE agents (
    id              TEXT PRIMARY KEY,  -- "hermes-lappy-01", "b00t-grok-pool"
    agent_type      TEXT NOT NULL,     -- 'hermes', 'claude-code', 'b00t-cli', 'b00t', 'generic'
    status          TEXT NOT NULL DEFAULT 'offline'
                    CHECK (status IN ('online','busy','offline','error')),
    -- ═══════════════════════════════════════════════════════════
    -- Capability declaration: this is the key routing axis
    -- ═══════════════════════════════════════════════════════════
    capabilities    TEXT NOT NULL DEFAULT '[]',  -- JSON array of offered capabilities
    label              TEXT,              -- Human-readable name
    last_heartbeat     TEXT,
    current_job_id     TEXT,               -- NULL if idle
    current_capability TEXT,               -- which capability is executing
    metadata           TEXT                -- JSON: hostname, pid, version, load, etc.
);
CREATE INDEX idx_agents_cap ON agents(capabilities);

-- Notifications: append-only event bus for the dashboard.
CREATE TABLE events (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id  TEXT NOT NULL,
    event     TEXT NOT NULL CHECK (event IN (
                'job_claimed','job_running','job_success','job_failed',
                'agent_online','agent_offline','agent_busy','agent_idle',
                'schedule_created','schedule_deleted','schedule_paused',
                'system_warn','system_error')),
    payload   TEXT,  -- JSON with event-specific data
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_runs_schedule ON runs(schedule_id, started_at DESC);
CREATE INDEX idx_events_time ON events(created_at DESC);
CREATE INDEX idx_events_agent ON events(agent_id, created_at DESC);
```

---

## b00t Ecosystem as a Capability Federation

The key insight: the b00t ecosystem is NOT a single agent adapter.
Each b00t subsystem is an independently schedulable capability.

### Capability Catalog

Derived automatically from the b00t CLI help tree and `_b00t_/*.agent.toml`
datum files. Here's the canonical mapping:

| Capability | Source | Schedules as | Example job |
|---|---|---|---|
| `cli-detect` | `b00t-cli detect` | `b00t` type | `every 1d: b00t-cli detect node` |
| `cli-install` | `b00t-cli install` | `b00t` type | `every 1w: b00t-cli up` |
| `grok-ask` | `b00t grok ask` | `b00t` type | `every 1h: grok ask --rag=irontology` |
| `grok-learn` | `b00t grok learn` | `b00t` type | `every 1d: grok learn --topic 'rust nightly'` |
| `grok-digest` | `b00t grok digest` | `b00t` type | `oneshot: grok digest --topic X` |
| `ontology-query` | `b00t ontology query` | `b00t` type | `every 1h: ontology query --role dev` |
| `hive-status` | `b00t hive status` | `b00t` type | `every 5m: hive status → events table` |
| `hive-activate` | `b00t hive activate` | `b00t` type | `at boot: hive activate=dev` |
| `bouncer-verify` | `b00t bouncer` | `b00t` / `llm` | `post-commit: verify gate` |
| `ledgrrr-audit` | `b00t ledger` / `ledgrrr` | `b00t` type | `every 1d: check invariants` |
| `task-process` | `b00t task` | `b00t` type | `every 30m: process backlog` |
| `checkpoint` | `b00t checkpoint` | `b00t` type | `post-merge: checkpoint` |
| `idiomap-scan` | `b00t idiomap` | `b00t` type | `every 1w: scan for drift` |
| `mcp-install` | `b00t mcp install` | `b00t` type | `at boot: install MCP servers` |
| `agent-discover` | `b00t agent discover` | `b00t` type | `every 1d: discover peers` |

### Auto-Registration from Datum Files

Capabilities are NOT hardcoded. They're discovered at runtime by
scanning `_b00t_/*.agent.toml` and `_b00t_/datums/*.tomllmd`:

```rust
// At scheduler startup or datum index update
fn auto_register_capabilities(db: &SchedulerDb) -> Result<()> {
    // Scan agent datums for [[b00t.agent.skills]]
    for agent_toml in glob("_b00t_/*.agent.toml")? {
        let datum = parse_toml(agent_toml);
        if let Some(skills) = datum.get("b00t.agent.skills") {
            for skill in skills.as_array() {
                db.register_capability(CapabilityRegistration {
                    name: skill.as_str(),
                    source: agent_toml,
                    executor: B00tCapabilityExecutor {
                        cmd: format!("b00t {}", skill),  // inferred
                    },
                });
            }
        }
    }

    // Scan CLI datums for [[b00t.usage]] command hints
    for cli_toml in glob("_b00t_/*.cli.toml")? {
        let datum = parse_toml(cli_toml);
        if let Some(usage) = datum.get("b00t.usage") {
            for entry in usage.as_array() {
                if let Some(cmd) = entry.get("command") {
                    let name = infer_capability_name(cmd.as_str());
                    db.register_capability(CapabilityRegistration {
                        name,
                        source: cli_toml,
                        executor: B00tCapabilityExecutor {
                            cmd: cmd.as_str().to_string(),
                        },
                    });
                }
            }
        }
    }
}
```

This means: add a new `.agent.toml` or `.cli.toml` datum file →
its capabilities are automatically available for scheduling.
No config changes, no restarts.

### b00t Ecosystem Adapter

A single lightweight adapter that can execute any registered capability:

```rust
/// Adapter that runs any b00t ecosystem capability.
/// One binary, many capabilities.
struct B00tEcosystemAdapter {
    agent_id: String,            // "b00t-cli-pool-01"
    offered_capabilities: Vec<String>,  // ["grok-ask", "grok-learn", "hive-status", ...]
}

#[async_trait]
impl AgentAdapter for B00tEcosystemAdapter {
    fn capabilities(&self) -> &[String] {
        &self.offered_capabilities
    }

    async fn execute(&self, schedule: &Schedule, run_id: &str, db: &SchedulerDb) -> Result<RunResult> {
        match schedule.agent_type {
            "b00t" => {
                // capability = schedule's required_capabilities[0]
                let capability = schedule.required_capabilities[0];
                let cmd = resolve_capability_command(&capability, &schedule.agent_config);

                let output = tokio::process::Command::new("b00t")
                    .args(&cmd)
                    .output()
                    .await?;

                RunResult {
                    status: if output.status.success() { "success" } else { "failed" },
                    exit_code: output.status.code(),
                    output: String::from_utf8_lossy(&output.stdout).to_string(),
                }
            }
            _ => Err("unsupported agent_type for b00t adapter"),
        }
    }
}
```

### Capability-Based Claim Protocol

The SQLite claim query becomes capability-aware:

```sql
BEGIN IMMEDIATE;

SELECT s.* FROM schedules s
WHERE s.enabled = 1
  AND (s.max_runs IS NULL OR s.run_count < s.max_runs)
  AND s.id NOT IN (
    SELECT schedule_id FROM runs
    WHERE status IN ('claimed','running')
  )
  AND is_due(s)

  -- Capability routing (preferred path)
  AND (
    s.required_capabilities IS NULL
    OR (
      -- All required capabilities must be offered by this agent
      (SELECT count(*)
       FROM json_each(s.required_capabilities) AS rc
       WHERE rc.value IN (
         SELECT value FROM json_each(?)
       )
      ) = (SELECT count(*) FROM json_each(s.required_capabilities))
    )
  )

  -- Agent pin (fallback path, overrides capabilities)
  AND (
    s.required_agent IS NULL
    OR s.required_agent = ?
  )

ORDER BY s.created_at ASC
LIMIT 1;
```

The `?` bindings: `[agent_capabilities_json, agent_id]`.

---

## Agent Heartbeat + Capability Registration

On startup, every agent (Hermes, b00t-cli daemon, etc.) writes
its identity + offered capabilities to the shared DB:

```python
# On agent start
db.execute("""
    INSERT OR REPLACE INTO agents
        (id, agent_type, status, capabilities, label, last_heartbeat, metadata)
    VALUES (?, ?, 'online', ?, ?, datetime('now'), ?)
""", [
    agent_id,
    agent_type,
    json.dumps(offered_capabilities),
    label,
    json.dumps({hostname, pid, version}),
])
```

### b00t-cli daemon capabilities (auto-detected)

```python
# b00t scheduler daemon auto-discovers its capabilities
capabilities = []

# From CLI subcommands
for subcommand in ["grok ask", "grok learn", "ontology query",
                   "hive status", "hive activate",
                   "bouncer verify", "ledger audit",
                   "task list", "task add",
                   "checkpoint", "idiomap scan",
                   "mcp install", "agent discover"]:
    if shutil.which(f"b00t {subcommand}"):
        capabilities.append(subcommand.replace(" ", "-"))

# From datum files
for datum in glob("_b00t_/*.agent.toml"):
    capabilities.extend(parse_skills_from_datum(datum))

db.register_agent(agent_id, "b00t", capabilities)
```

---

## Hermes CLI Dashboard with Capability View

```
┌──────────────────────────────────────────────────────────────────┐
│  b00t Scheduler Dashboard — 5 agents, 12 schedules, 23 caps     │
├──────────────────────────────────────────────────────────────────┤
│  AGENTS (capabilities)                                           │
│  hermes-lappy     ● online  idle  [llm terminal file web mcp]    │
│  b00t-cli-pool    ● online  idle  [grok-ask grok-learn hive...]  │
│  claude-code-wsl  ○ offline       [llm terminal file]            │
│  bouncer-dedicated ● online  busy  job: sched_bnc                │
│  grok-ingest      ● online  idle  [grok-learn grok-digest]       │
├──────────────────────────────────────────────────────────────────┤
│  SCHEDULES                                                       │
│  grok-ask  every 1h  [grok-ask]        b00t-cli-pool ████░░░░   │
│  hive-status every 5m [hive-status]     any-online   ████████    │
│  bouncer-vfy post-cmt [bouncer-verify]  bouncer-ded  ██░░░░░░   │
│  weekly-scan every 1w [grok-learn ontology-query] b00t-cli ░░░  │
│  ideate    2026-05-10 [llm]              hermes       ░░░░░░░░  │
├──────────────────────────────────────────────────────────────────┤
│  LIVE EVENTS                                                     │
│  04:01:23  b00t-cli-pool   job_claimed    sched_grok_01          │
│  04:01:24  b00t-cli-pool   job_running    cap=grok-ask           │
│  04:01:25  bouncer-ded     job_success    sched_bnc exit=0       │
│  04:01:30  b00t-cli-pool   job_success    sched_grok_01          │
└──────────────────────────────────────────────────────────────────┘
```

---

## Why Capability Routing Over Agent-Type Routing

| Aspect | Agent-type routing | Capability routing |
|--------|-------------------|-------------------|
| Granularity | Whole-agent (Hermes, Claude, etc.) | Per-subsystem (grok-ask, hive-status) |
| New subsystems | Add agent type, write adapter | Add datum file, auto-registered |
| Load distribution | All-or-nothing per agent | Fine-grained: grok jobs to grok pool |
| b00t ecosystem | One big "b00t-cli" adapter | N individual capabilities |
| Scheduling | "Run this via Hermes" | "Anyone with grok-ask claim this" |
| Discoverability | Agent registry only | Datum → capability → scheduler |
| Failure isolation | Whole agent goes down | Individual capability can restart |

---

## Migration Path

1. **Phase 0** — Create `scheduler.db` schema in `b00t-cli`
2. **Phase 1** — `b00t scheduler create/list/claim` subcommands +
   capability auto-registration from datum files
3. **Phase 2** — `b00t scheduler daemon` (auto-claim loop) +
   b00t ecosystem adapter with all sub-capabilities
4. **Phase 3** — Hermes `hermes run --report-to` headless mode +
   `hermes dashboard` capability-aware TUI
5. **Phase 4** — Agent heartbeat daemon + `FOR UPDATE SKIP LOCKED`
   claim protocol
6. **Phase 5** — Agent adapters for Claude Code (ACP), MCP,
   webhook delivery

# b00t:map v1
# summary: b00t Scheduler — multi-agent scheduling with SQLite SST, capability-based routing (not agent-type), b00t ecosystem as capability federation auto-registered from datum files
# tags: scheduler, cron, multi-agent, sqlite, capabilities, b00t-ecosystem, hermes-dashboard, grok, ontology, hive, bouncer
# tier: frontier
# cmds: b00t scheduler create, b00t scheduler list, b00t scheduler daemon, b00t scheduler dashboard
# complexity: 9
