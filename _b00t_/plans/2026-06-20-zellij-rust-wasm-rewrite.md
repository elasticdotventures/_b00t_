# b00t Zellij Interaction — Rust/WASM Rewrite Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.
> **Tier:** frontier (architecture) → ch0nky (implementation) → sm0l (tests)

**Goal:** Replace bash-based Zellij interaction PoC (shell injection, broken exit codes, fragile JSON)
with a Rust binary (`b00t zellij`) that compiles to WASM for browser agents and native for terminal agents.

**Architecture:** Single `b00t-cli` subcommand → `b00t-c0re-lib::KvStore` (persistence) →
`b00t-c0re-gov::Gate` (governance) → `duct`/`std::process` (zellij CLI calls). 
Core types in `b00t-c0re-lib` so WASM target can import them without CLI deps.

**Tech Stack:** Rust 2024, serde, clap, duct, fs2 (file locking), b00t-c0re-lib::KvStore.
WASM target: wasm-bindgen + serde-wasm-bindgen (no duct — API-only).

---

## Phase 0: Gap Analysis (current state → target state)

### What the bash PoC does (and where it fails)

| Bash Script | Function | Critical Flaw |
|-------------|----------|---------------|
| `zellij-kv-cache.sh` | JSON read/write via Python string interp | **Shell injection** — user values interpolated into Python code |
| `zellij-run-interactive.sh` | Launch fzf/confirm in floating pane | **Exit code loss** — `zellij run` returns 0 immediately, inner exit lost |
| `zellij-mandatory-gate.sh` | Eisenhower routing + audit | **Exit code reads always 0** (depends on broken runner) |
| `zellij-user-interaction.sh` | 5-mode dialog engine | **Shell injection** (wizard mode interps `$step_input` in Python) |
| `zellij-gate.just` | Just recipes wrapping gate | **grep-based JSON parsing** (fragile), TOCTOU race |
| `init-zellij-agent.sh` | Detection + KVCache init | `|| true` masks all failures silently |

### What the Rust rewrite fixes

| Flaw | Fix |
|------|-----|
| Shell injection via Python string interp | `serde_json` — no shell, no Python, pure typed JSON |
| `zellij run` exit code loss | Use `zellij pipe` or `zellij action query-tab-panes` for result capture |
| grep-based JSON parsing | `serde_json::from_str` with proper error handling |
| TOCTOU race in KVCache | `fs2::FileExt::lock_exclusive` before read/write |
| `|| true` masking failures | Rust `Result<T, E>` — no silent failures |
| Triplicated menu items | Single source: `zellij-interaction.gate.toml` parsed by Rust |
| Missing tests | `#[cfg(test)]` module per file, property-based with `proptest` |

---

## Phase 1: Core Types in `b00t-c0re-lib` (shared by CLI + WASM)

### Task 1.1: GateResult enum

**Objective:** Replace Allow/Deny/Hook bash exit codes with typed enum

**File:** `b00t-c0re-lib/src/kv_store.rs` (add alongside KvStore)

```rust
/// Governance gate result — maps to bash exit codes 0/1/2
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GateResult {
    Allow,
    Deny,
    Hook,
    Error(String),
}

impl GateResult {
    pub fn exit_code(&self) -> i32 {
        match self {
            GateResult::Allow => 0,
            GateResult::Deny => 1,
            GateResult::Hook => 2,
            GateResult::Error(_) => 3,
        }
    }
}
```

### Task 1.2: InteractionMode enum

**Objective:** Type-safe representation of the 5 interaction modes

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InteractionMode {
    Confirm { title: String, message: String },
    FzfMenu { title: String, items: Vec<String> },
    TextInput { prompt: String, default: String },
    SubagentReport { agent: String, status: String, summary: String },
    Wizard { title: String },
}
```

### Task 1.3: EisenhowerQuadrant enum + routing

**Objective:** Type-safe Eisenhower classification

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum EisenhowerQuadrant {
    DoNow,        // Urgent + Important → Allow via confirm
    Schedule,     // Not-Urgent + Important → Hook via fzf menu
    Delegate,     // Urgent + Not-Important → Hook via subagent report
    Eliminate,    // Not-Urgent + Not-Important → Deny
}
```

### Task 1.4: KvStore enhancement — atomic write + gate keys

**Objective:** Add `lock_exclusive` + gate-specific key helpers to existing KvStore

**File:** `b00t-c0re-lib/src/kv_store.rs`

```rust
impl KvStore {
    /// Atomic write with file locking (prevents TOCTOU race)
    pub fn set_atomic(&self, key: &str, value: &str) -> Result<(), KvStoreError> {
        let path = self.resolve_path()?;
        let file = std::fs::OpenOptions::new()
            .read(true).write(true).create(true)
            .open(&path)?;
        file.lock_exclusive()?;  // fs2 — blocks until lock acquired
        // ... read, modify, write ...
        file.unlock()?;
        Ok(())
    }
    
    /// Batch-write gate state (4 keys in one atomic operation)
    pub fn set_gate_state(&self, result: &GateResult, selection: &str) -> Result<()> {
        self.set_atomic("zellij.gate.last-result", &result.to_string())?;
        self.set_atomic("zellij.gate.last-selection", selection)?;
        self.set_atomic("zellij.gate.last-exit-code", &result.exit_code().to_string())?;
        self.set_atomic("zellij.gate.last-timestamp", &Utc::now().to_rfc3339())?;
        Ok(())
    }
}
```

### Task 1.5: AuditTrail struct

**Objective:** Structured JSONL audit logging (replaces bash `echo "{...}" >> file`)

```rust
#[derive(Debug, Serialize)]
pub struct AuditEntry {
    pub timestamp: DateTime<Utc>,
    pub session: String,
    pub agent: String,
    pub action: String,
    pub result: String,
    pub selection: String,
    pub exit_code: i32,
}

impl AuditEntry {
    pub fn append_to(path: &Path, entry: &AuditEntry) -> Result<()> {
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        writeln!(file, "{}", serde_json::to_string(entry)?)?;
        Ok(())
    }
}
```

---

## Phase 2: `b00t zellij` Subcommand in `b00t-cli`

### Task 2.1: CLI struct + clap definition

**File:** `b00t-cli/src/commands/zellij.rs` (new)

```rust
#[derive(Subcommand)]
pub enum ZellijCommand {
    /// Detect Zellij session and init KVCache
    Init {
        #[arg(long)]
        agent_type: Option<String>,
        #[arg(long)]
        task_context: Option<String>,
    },
    /// Launch interactive fzf menu (proper TTY via zellij run)
    Menu {
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        urgent: bool,
        #[arg(long)]
        important: bool,
    },
    /// Quick Y/N confirm dialog
    Confirm {
        title: String,
        #[arg(long)]
        message: Option<String>,
    },
    /// Text input dialog
    Input {
        prompt: String,
        #[arg(long, default_value = "")]
        default: String,
    },
    /// Sub-agent report modal
    Report {
        agent: String,
        status: String,
        summary: String,
    },
    /// Multi-step wizard
    Wizard {
        title: String,
    },
    /// Check gate state (reads KVCache, returns Allow/Deny/Hook)
    GateCheck {
        #[arg(long)]
        action: Option<String>,
    },
    /// View audit trail
    Audit {
        #[arg(long, default_value = "20")]
        lines: usize,
    },
    /// System status
    Status,
}
```

### Task 2.2: Zellij detection + init

**Objective:** Replace `init-zellij-agent.sh`

```rust
pub fn detect_zellij() -> Option<ZellijSession> {
    let session = std::env::var("ZELLIJ_SESSION_NAME").ok()?;
    let pane_id = std::env::var("ZELLIJ_PANE_ID").unwrap_or_default();
    Some(ZellijSession { session, pane_id })
}

pub fn init_zellij(kv: &KvStore) -> Result<()> {
    let Some(session) = detect_zellij() else {
        kv.set("zellij.active", "false")?;
        return Ok(());
    };
    kv.set("zellij.active", "true")?;
    kv.set("zellij.session", &session.session)?;
    kv.set("zellij.pane", &session.pane_id)?;
    kv.set("zellij.last-seen", &Utc::now().to_rfc3339())?;
    // Detect tools
    kv.set("zellij.fzf", &detect_fzf_version().unwrap_or("none".into()))?;
    kv.set("zellij.whiptail", &detect_whiptail_version().unwrap_or("none".into()))?;
    Ok(())
}
```

### Task 2.3: TTY-safe subprocess execution

**Objective:** Replace `zellij-run-interactive.sh` — solve exit code propagation

```rust
/// Run an interactive program in a Zellij floating pane.
/// Returns the exit code of the INNER program, not zellij run itself.
pub fn run_interactive(mode: &InteractionMode) -> Result<GateResult> {
    let script = render_interaction_script(mode)?;
    let tmp = tempfile::NamedTempFile::new()?;
    std::fs::write(tmp.path(), script)?;
    
    // Use zellij run with --close-on-exit
    // Capture exit code via a wrapper that writes to a temp file
    let exit_file = tempfile::NamedTempFile::new()?;
    let wrapper = format!(
        "bash {}; echo $? > {}",
        tmp.path().display(),
        exit_file.path().display()
    );
    
    duct::cmd!("zellij", "run", "--floating", "--close-on-exit", "--",
               "bash", "-c", &wrapper)
        .stdout_null()
        .run()?;
    
    // Read the INNER exit code (not zellij's exit code)
    let exit_code: i32 = std::fs::read_to_string(exit_file.path())?
        .trim()
        .parse()
        .unwrap_or(1);
    
    Ok(match exit_code {
        0 => GateResult::Allow,
        1 => GateResult::Deny,
        2 => GateResult::Hook,
        _ => GateResult::Error(format!("unexpected exit code: {exit_code}")),
    })
}
```

### Task 2.4: Eisenhower routing logic

**Objective:** Replace `zellij-mandatory-gate.sh` lines 85-200

```rust
pub fn route_eisenhower(
    quadrant: EisenhowerQuadrant,
    action: &str,
    kv: &KvStore,
) -> Result<GateResult> {
    match quadrant {
        EisenhowerQuadrant::DoNow => {
            let result = run_interactive(&InteractionMode::Confirm {
                title: format!("🥾 {action}"),
                message: "URGENT — Press Y to proceed, N to block".into(),
            })?;
            kv.set_gate_state(&result, "confirmed")?;
            Ok(result)
        }
        EisenhowerQuadrant::Schedule => {
            let items = load_menu_items_from_gate_toml()?;
            let result = run_interactive(&InteractionMode::FzfMenu {
                title: format!("🥾 Action Required — {action}"),
                items,
            })?;
            kv.set_gate_state(&result, "menu-selection")?;
            Ok(result)
        }
        EisenhowerQuadrant::Delegate => {
            run_interactive(&InteractionMode::SubagentReport {
                agent: "gate-delegate".into(),
                status: "info".into(),
                summary: format!("Delegated: {action}"),
            })?;
            kv.set_gate_state(&GateResult::Hook, "delegated")?;
            Ok(GateResult::Hook)
        }
        EisenhowerQuadrant::Eliminate => {
            AuditEntry::new("deny", "low-priority", action)
                .append_to(Path::new("~/.b00t/audit/zellij-gate.jsonl"))?;
            kv.set_gate_state(&GateResult::Deny, "low-priority")?;
            Ok(GateResult::Deny)
        }
    }
}
```

### Task 2.5: Wire into `b00t-cli` main

**File:** `b00t-cli/src/main.rs`

Add to clap:
```rust
Zellij {
    #[command(subcommand)]
    command: ZellijCommand,
}
```

Add to command dispatch:
```rust
Commands::Zellij { command } => commands::zellij::handle(command),
```

---

## Phase 3: Gate Integration with `b00t-c0re-gov`

### Task 3.1: Register ZellijGate in governance system

**File:** `b00t-c0re-gov/src/gates/eisenhower.rs`

```rust
pub struct ZellijGate {
    pub kv: Arc<KvStore>,
    pub audit_path: PathBuf,
}

impl Gate for ZellijGate {
    fn check(&self, action: &str, urgency: Urgency, importance: Importance) -> GateResult {
        if !zellij_detected() {
            return GateResult::Allow; // No Zellij — gate bypassed
        }
        let quadrant = EisenhowerQuadrant::classify(urgency, importance);
        route_eisenhower(quadrant, action, &self.kv)
    }
    
    fn name(&self) -> &str { "zellij-interaction" }
    fn mode(&self) -> GateMode { GateMode::Mandatory }
}
```

### Task 3.2: Gate check in justfile recipes

**Objective:** Replace bash `gate-preflight` with `b00t zellij gate-check`

```justfile
gate-preflight action="command":
    #!/bin/bash
    b00t zellij gate-check --action "{{ action }}"
    EXIT_CODE=$?
    case $EXIT_CODE in
        0) echo "✅ Gate: ALLOW" ;;
        1) echo "❌ Gate: DENY" ; exit 1 ;;
        2) echo "🔄 Gate: HOOK" ; exit 2 ;;
    esac
```

---

## Phase 4: WASM Target (browser agents)

### Task 4.1: Feature-gate CLI deps

**File:** `b00t-c0re-lib/Cargo.toml`

```toml
[features]
default = ["cli"]
cli = ["duct", "tempfile"]  # Only native builds
wasm = ["wasm-bindgen", "serde-wasm-bindgen"]  # Browser target
```

### Task 4.2: WASM API surface

**File:** `b00t-c0re-lib/src/wasm.rs` (new, behind `#[cfg(feature = "wasm")]`)

```rust
#[wasm_bindgen]
pub struct WasmKvStore { inner: KvStore }

#[wasm_bindgen]
impl WasmKvStore {
    pub fn new() -> WasmKvStore { ... }
    pub fn get(&self, key: &str) -> Option<String> { ... }
    pub fn set(&mut self, key: &str, value: &str) { ... }
    pub fn gate_check(&self, action: &str, urgent: bool, important: bool) -> String {
        // Pure logic — no TTY, no zellij process
        // Returns JSON: {"result": "allow", "mode": "confirm", ...}
        serde_json::to_string(&classify(action, urgent, important)).unwrap()
    }
    pub fn get_menu_items(&self) -> String {
        serde_json::to_string(&load_menu_items()).unwrap()
    }
}
```

**What WASM CAN do:**
- Read/write KVCache (via localStorage adapter)
- Eisenhower classification
- Menu item parsing from TOML
- Audit trail generation (JSONL)
- Gate check logic (returns result, caller handles TTY)

**What WASM CANNOT do (by design):**
- Spawn zellij processes (no subprocess in browser)
- TTY interaction (browser uses its own UI)

### Task 4.3: Justfile WASM build

```justfile
build-wasm:
    cd b00t-c0re-lib && wasm-pack build --target web --features wasm
```

---

## Phase 5: Replace Bash Scripts (one-for-one)

| Bash Script | Rust Replacement | Notes |
|-------------|-----------------|-------|
| `zellij-kv-cache.sh` | `b00t zellij init` | Uses KvStore directly |
| `zellij-run-interactive.sh` | `b00t zellij menu/confirm/input/report/wizard` | TTY-safe via wrapper |
| `zellij-mandatory-gate.sh` | `b00t zellij gate-check` | Eisenhower routing |
| `zellij-user-interaction.sh` | `b00t zellij menu` (renders inline) | No bash templating |
| `init-zellij-agent.sh` | `b00t zellij init` | Detection + KVCache |
| `gate-init-agent.sh` | `b00t zellij init --gate` | Gate activation |
| `zellij-gate.just` | Just recipes wrapping `b00t zellij` | Thin shim |
| `zellij-fzf-menu.sh` | `b00t zellij menu` | Merged |

Bash scripts become 1-line shims:
```bash
#!/usr/bin/env bash
# Shim — delegates to b00t-cli
exec b00t zellij menu --title "${1:-Action}" "${@:2}"
```

---

## Phase 6: Testing

### Task 6.1: Unit tests for KvStore atomic ops

```rust
#[test]
fn test_atomic_write_prevents_corruption() {
    let kv = KvStore::new_temp()?;
    kv.set_atomic("key1", "value1")?;
    assert_eq!(kv.get("key1").unwrap(), "value1");
}

#[test]
fn test_set_gate_state_batch_writes_all_four_keys() {
    let kv = KvStore::new_temp()?;
    kv.set_gate_state(&GateResult::Allow, "build-test")?;
    assert_eq!(kv.get("zellij.gate.last-result").unwrap(), "allow");
    assert_eq!(kv.get("zellij.gate.last-selection").unwrap(), "build-test");
}
```

### Task 6.2: Integration test for gate routing

```rust
#[test]
fn test_eisenhower_do_now_returns_allow() {
    let kv = KvStore::new_temp()?;
    let result = route_eisenhower(EisenhowerQuadrant::DoNow, "test", &kv)?;
    assert_eq!(result, GateResult::Allow);
}

#[test]
fn test_eliminate_returns_deny_and_logs_audit() {
    let kv = KvStore::new_temp()?;
    let audit = tempfile::NamedTempFile::new()?;
    let result = route_eisenhower(EisenhowerQuadrant::Eliminate, "noise", &kv)?;
    assert_eq!(result, GateResult::Deny);
    let logged = std::fs::read_to_string(audit.path())?;
    assert!(logged.contains("noise"));
}
```

### Task 6.3: Property tests for JSON roundtrip

```rust
#[test]
fn test_gate_result_serde_roundtrip() {
    for result in [GateResult::Allow, GateResult::Deny, GateResult::Hook, GateResult::Error("test".into())] {
        let json = serde_json::to_string(&result)?;
        let back: GateResult = serde_json::from_str(&json)?;
        assert_eq!(result, back);
    }
}
```

---

## Roadmap Summary (7 weeks)

| Week | Phase | Deliverable |
|------|-------|-------------|
| 1 | Phase 1: Core types | GateResult, InteractionMode, EisenhowerQuadrant, atomic KvStore, AuditTrail in `b00t-c0re-lib` |
| 2 | Phase 2.1-2.3: CLI struct + detection + TTY-safe exec | `b00t zellij init/menu/confirm/input/report/wizard` working |
| 3 | Phase 2.4-2.5: Eisenhower routing + main wiring | `b00t zellij gate-check` working, gate routing correct |
| 4 | Phase 3: Governance integration | ZellijGate registered in b00t-c0re-gov, just recipes updated |
| 5 | Phase 4: WASM target | `b00t-c0re-lib` compiles to WASM with `wasm-pack build` |
| 6 | Phase 5: Replace bash scripts | All bash shims point to `b00t zellij`, old scripts archived |
| 7 | Phase 6: Testing | Full test suite, property tests, integration tests pass |

### First PR (Week 1-2): `b00t zellij init` + `b00t zellij menu`

Minimal viable surface — detection + fzf menu working from Rust. Everything else builds on this.

### Anti-Patterns to Avoid

1. **DO NOT** use `|| true` or `.unwrap_or_default()` to mask errors — propagate with `?`
2. **DO NOT** interpolate strings into shell/Python — use `serde_json` for all data
3. **DO NOT** parse JSON with regex/grep — use `serde_json::from_str`
4. **DO NOT** assume `zellij run` exit code = script exit code — wrap with exit-file capture
5. **DO NOT** duplicate menu items in 3 places — parse TOML once, generate from single source

<!-- b00t:map v1
summary: Rust/WASM rewrite plan — replace bash PoC with b00t zellij CLI subcommand, phased over 7 weeks
tags: rust, wasm, zellij, gate, governance, rewrite, plan, roadmap
tier: frontier
cmds: b00t zellij init, b00t zellij gate-check, b00t zellij menu
complexity: 10
-->
