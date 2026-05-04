# SDD-006: ledg3rr Syntax + Grok Chained Provider Architecture

> **Status:** RESEARCH COMPLETE | **Date:** 2026-05-03
> **Components:** DualGrokClient, GrokBackend, IrontologyBridgeClient, DatumNode, InvariantGraph
> **Dependencies:** b00t-c0re-lib, irontology-mcp, ralph.sh provider chain

---

## 1. Problem Statement

Grok on another node experiencing problems. Need chained provider stream architecture to enable:
1. Multi-backend fan-out with graceful degradation
2. Consumer subscription to data shape changes
3. ledg3rr-based governance for data flow validation

---

## 2. Current Grok Provider Architecture

### 2.1 GrokBackend Enum

Two versions exist in the codebase:

**Version 1: dual_grok.rs (Fan-out Selector)**
```rust
// b00t-c0re-lib/src/dual_grok.rs:30
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GrokBackend {
    /// RAGLight Python subprocess only
    Raglite,
    /// Irontology NeumannStore only
    Irontology,
    /// Fan-out to both (default)
    Both,
}
```

**Version 2: grok.rs (MCP Client Selector)**
```rust
// b00t-c0re-lib/src/grok.rs:25
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrokBackend {
    /// b00t-grok-py (Python, requires Qdrant + Ollama)
    Python,
    /// irontology-mcp (Rust, NeumannStore + 4-way fusion)
    Irontology,
}
```

**Backend Selection Logic:**
```rust
// dual_grok.rs:41
pub fn from_flag(raw: Option<&str>) -> Result<Self> {
    match raw {
        None | Some("both") | Some("") => Ok(Self::Both),
        Some("raglite") | Some("raglight") | Some("rag-light") => Ok(Self::Raglite),
        Some("irontology") | Some("iron") => Ok(Self::Irontology),
        // ... error handling
    }
}
```

### 2.2 DualGrokClient Pattern

**Location:** `b00t-c0re-lib/src/dual_grok.rs:102`

```rust
/// Fan-out grok client dispatching to raglite and/or irontology
pub struct DualGrokClient {
    // both backends are lazily initialized; failure on init is non-fatal
    iron: Option<IrontologyBridgeClient>,
}

impl DualGrokClient {
    pub fn new() -> Self {
        let iron = IrontologyBridgeClient::new("b00t-grok")
            .map_err(|e| {
                tracing::warn!("IrontologyBridgeClient init failed (non-fatal): {}", e);
                e
            })
            .ok();
        Self { iron }
    }

    /// Ingest content to the specified backend(s)
    /// Partial failure is surfaced as warnings, not as a hard error
    pub async fn ingest(&mut self, topic: &str, content: &str, backend: GrokBackend) 
        -> Result<DualIngestResult> { ... }

    /// Query across backends, merging and deduplicating results
    pub async fn query(&self, query_str: &str, topic: Option<&str>, limit: Option<usize>, 
        backend: GrokBackend) -> Result<DualQueryResult> { ... }
}
```

**Result Types:**
```rust
pub struct DualIngestResult {
    pub topic: String,
    pub backend: String,
    pub raglite_job_id: Option<String>,
    pub irontology_subject: Option<String>,
    pub raglite_ok: bool,
    pub irontology_ok: bool,
    pub warnings: Vec<String>,
}

pub struct DualQueryItem {
    pub backend: String,
    pub content: String,
    pub topic: String,
    pub tags: Vec<String>,
    pub score: f32,
}
```

### 2.3 IrontologyBridgeClient

**Location:** `b00t-c0re-lib/src/irontology_bridge.rs:224`

```rust
/// Client wrapping a shared `NeumannStore` for b00t grok operations
#[derive(Clone)]
pub struct IrontologyBridgeClient {
    store: std::sync::Arc<NeumannStore>,
    namespace: String,
}

/// Canonical b00t datum — portable across raglite and irontology backends
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatumNode {
    pub topic: String,      // b00t datum name
    pub class: String,      // OWL class label
    pub content: String,    // Primary content
    pub tags: Vec<String>,  // Searchable tags
    pub predicates: Vec<(String, String)>,  // RDF-style triples
}
```

**Semantic Mapping (b00t datum → irontology triples):**
```
subject   = b00t:datum/<topic>/<uuid>
predicate = b00t:hasContent | b00t:hasTag | b00t:hasClass | b00t:<custom>
object    = JSON Value
```

### 2.4 CLI Integration

**Location:** `b00t-cli/src/commands/grok.rs`

```rust
#[derive(Subcommand, Clone)]
pub enum GrokCommands {
    /// Digest content into chunks about a topic
    Digest {
        #[arg(short, long)]
        topic: String,
        content: String,
        #[arg(long = "rag", value_name = "BACKEND", num_args = 0..=1)]
        rag: Option<String>,
    },
    /// Ask questions and search the knowledgebase
    Ask { query: String, topic: Option<String>, limit: Option<usize>, rag: Option<String> },
    /// Learn from URLs or files
    Learn { source: Option<String>, content: Option<String>, topic: Option<String>, rag: Option<String> },
    /// Assimilate content: LLM-distill → store as git blob → write datum TOML
    Assimilate { topic: String, content: Option<String>, file: Option<PathBuf>, class: String, tags: Vec<String>, ingest: bool, source_url: Option<String> },
}
```

---

## 3. ledg3rr Syntax

### 3.1 InvariantGraph Core Types

**Location:** `b00t-l3dg3rr-viz/src/lib.rs`

```rust
/// Stable visual category shared by l3dg3rr docs and b00t capability graphs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VisualizationRole {
    Ingest,
    Validate,
    Classify,
    Review,
    Reconcile,
    Commit,
    Decision,
    Step,
}

/// A typed invariant vertex. IDs are stable machine identifiers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantNode {
    pub id: String,
    pub label: String,
    pub role: VisualizationRole,
    pub invariant: Option<String>,
}

/// A typed invariant edge. Endpoints MUST reference existing node IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantEdge {
    pub from: String,
    pub to: String,
    pub label: Option<String>,
}

/// Host-neutral graph for l3dg3rr-style invariant visualization
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariantGraph {
    pub name: String,
    pub nodes: Vec<InvariantNode>,
    pub edges: Vec<InvariantEdge>,
}
```

### 3.2 Validation Invariants

```rust
/// Verify graph invariants before rendering:
/// - graph name is non-empty
/// - node IDs are non-empty and unique
/// - every edge endpoint references a known node
/// - self-edges are rejected unless labelled
pub fn validate(&self) -> Result<(), GraphValidationError> { ... }
```

### 3.3 Governance Trait Pattern

**From:** `docs/superpowers/specs/2026-05-02-b00t-task-next-l3dg3rr-governance-design.md`

```rust
pub trait TaskQueueInvariant {
    fn state(&self) -> TaskQueueState;
    fn is_valid_transition(&self, from: TaskQueueState, to: TaskQueueState) -> bool;
}

pub trait GovernanceGate {
    fn check(&self, context: &GovernanceContext) -> Result<(), GovernanceError>;
    fn reason(&self) -> String;
}
```

### 3.4 RHAI Engine Integration

**Location:** `b00t-c0re-lib/src/rhai_engine.rs`

```rust
/// RHAI engine wrapper with b00t-specific functionality
pub struct RhaiEngine {
    engine: Engine,
    context: B00tContext,
    scripts_dir: PathBuf,
}

// Registered functions:
// - run_cmd(cmd) -> Result<String>
// - run_cmd_if(cmd, condition) -> Result<String>
// - command_exists(cmd) -> bool
// - install_package(package, is_docker) -> Result<String>
// - file_exists(path) -> bool
// - create_dir(path) -> Result<()>
// - write_file(path, content) -> Result<()>
// - read_file(path) -> Result<String>
```

---

## 4. Provider Chain Pattern (ralph.sh)

### 4.1 Current Implementation

**Location:** `ralphs/ralph-plus-_b00t_/ralph.sh:10`

```bash
PROVIDER_CHAIN="${PROVIDER_CHAIN:-${B00T_UP_PROVIDERS:-}}"

# Default fallback
if [[ -z "${PROVIDER_CHAIN}" && -n "${MODEL_ALIAS}" ]]; then
    PROVIDER_CHAIN="llama-cpp,openai-compatible"
fi

# Chain accumulation
--provider)
    if [[ -n "${PROVIDER_CHAIN}" ]]; then
        PROVIDER_CHAIN="${PROVIDER_CHAIN},$2"
    else
        PROVIDER_CHAIN="$2"
    fi
    ;;
```

### 4.2 Provider Resolution

```bash
canonical_provider_name() {
    local provider="${1:-}"
    case "${provider,,}" in
        llamacpp|llama_cpp|direct) echo "llama-cpp" ;;
        openai_compatible) echo "openai-compatible" ;;
        litellm|gateway) echo "openai" ;;
        *) echo "${provider,,}" ;;
    esac
}

provider_chain_contains() {
    local needle
    needle="$(canonical_provider_name "${1:-}")"
    IFS=',' read -ra _providers <<< "${PROVIDER_CHAIN:-}"
    for provider in "${_providers[@]}"; do
        [[ "$(canonical_provider_name "${provider}")" == "${needle}" ]] && return 0
    done
    return 1
}
```

### 4.3 Transport Resolution

```bash
resolve_pi_transport() {
    IFS=',' read -ra _providers <<< "${PROVIDER_CHAIN:-}"
    for provider in "${_providers[@]}"; do
        case "$(canonical_provider_name "${provider}")" in
            llama-cpp)
                if http_ok "${direct_url}"; then
                    PI_PROVIDER="${PI_DIRECT_PROVIDER}"
                    PI_BASE_URL="${direct_base}"
                    return 0
                fi
                ;;
            openai-compatible|openai)
                if http_ok "${gateway_url}"; then
                    PI_PROVIDER="openai"
                    PI_BASE_URL="${gateway_base}"
                    return 0
                fi
                ;;
        esac
    done
    # Fallback to direct local model
}
```

---

## 5. Proposed Consumer Subscription Pattern

### 5.1 Data Shape Subscription Syntax (TOML)

```toml
# ~/.b00t/subscriptions.d/inventory-updates.subscription.toml

[subscription]
id = "sub-inventory-001"
consumer = "inventory-sync-agent"
enabled = true

[shape]
shape_uri = "shape:InventoryItemShape"
topic = "inventory"
validation = "strict"  # strict | lenient | none

[trigger]
on = ["create", "update"]  # create | update | delete | all
debounce_ms = 500

[delivery]
method = "stream"  # webhook | stream | poll | mcp
buffer_size = 100

[governance]
gate = "CanReceiveInventoryUpdates"
require_proof = true
```

### 5.2 DataShapeConsumer Trait

```rust
// b00t-c0re-lib/src/subscription.rs

/// A consumer that subscribes to data shape changes
pub trait DataShapeConsumer: Send + Sync {
    fn id(&self) -> &str;
    fn subscribed_shapes(&self) -> Vec<ShapeUri>;
    fn subscribed_topics(&self) -> Vec<String>;
    fn on_datum(&self, datum: &DatumNode, proof: &TransactionProof) -> Result<(), ConsumerError>;
    fn on_shape_evolution(&self, old_shape: &Shape, new_shape: &Shape) -> Result<(), ShapeError>;
    fn validation_level(&self) -> ValidationLevel;
}

pub struct SubscriptionConfig {
    pub id: String,
    pub consumer_id: String,
    pub shape_uri: Option<ShapeUri>,
    pub topic: Option<String>,
    pub triggers: Vec<TriggerType>,
    pub delivery: DeliveryMethod,
    pub governance_gate: Option<String>,
    pub debounce_ms: u64,
}

pub struct SubscriptionManager {
    subscriptions: HashMap<String, SubscriptionConfig>,
    consumers: HashMap<String, Arc<dyn DataShapeConsumer>>,
    store: Arc<NeumannStore>,
}
```

### 5.3 Chained Provider Stream Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ Provider Chain (Data)                                           │
│ PROVIDER_DATA_CHAIN="irontology,raglite,remote-grok"           │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│ SubscriptionManager                                             │
│ ├─ notify_datum(datum, event)                                   │
│ │  ├─ debounce (500ms window)                                   │
│ │  ├─ governance_gate.check()                                   │
│ │  └─ fan-out to consumers                                      │
│ └─ stream(consumer_id) -> Receiver<DatumEvent>                  │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│ Consumer Registry                                               │
│ ├─ inventory-sync-agent: DataShapeConsumer                      │
│ │  └─ shapes: [InventoryItemShape]                              │
│ │  └─ topics: [inventory]                                       │
│ └─ ledger-reconciler: DataShapeConsumer                         │
│    └─ shapes: [ReceiptShape, TransactionShape]                  │
│    └─ topics: [transactions, receipts]                          │
└─────────────────────────────────────────────────────────────────┘
```

---

## 6. Key Files Referenced

| File | Purpose |
|------|---------|
| `b00t-c0re-lib/src/dual_grok.rs` | DualGrokClient fan-out pattern |
| `b00t-c0re-lib/src/grok.rs` | GrokClient MCP wrapper |
| `b00t-c0re-lib/src/irontology_bridge.rs` | DatumNode, IrontologyBridgeClient |
| `b00t-c0re-lib/src/rhai_engine.rs` | RHAI scripting engine |
| `b00t-l3dg3rr-viz/src/lib.rs` | InvariantGraph, L3dg3rrVisualizable |
| `b00t-cli/src/commands/grok.rs` | Grok CLI commands |
| `ralphs/ralph-plus-_b00t_/ralph.sh` | Provider chain pattern |

---

## 7. Implementation Gaps

### Gap 1: No Consumer Subscription Mechanism
- Grok supports `ask/digest/learn/status` - all pull-based
- No push notifications when data shapes change
- No streaming/chained consumer pattern

### Gap 2: Data Shapes Not Exposed
- SHACL shapes defined in irontology-mcp but not exposed to b00t consumers
- No shape validation at consumer boundary
- No shape evolution/compatibility checking

### Gap 3: Provider Chain Data-Only
- Current `PROVIDER_CHAIN` in ralph.sh is model selection only
- Need data provider chain pattern for streaming
- Need consumer attachment to chain

---

## 8. Proposed Implementation Phases

### Phase 1: Core Subscription Types (sm0l, ~1hr)
- Add `subscription.rs` to b00t-c0re-lib
- Define `DataShapeConsumer` trait
- Define `SubscriptionConfig`, `SubscriptionManager` structs

### Phase 2: Storage Integration (ch0nky, ~2hr)
- Add subscription storage to NeumannStore
- Implement shape registry with SHACL validation
- Wire to sled persistence

### Phase 3: Notification Pipeline (ch0nky, ~3hr)
- Implement `notify_datum()` with debounce
- Add governance gate checks
- Wire to OTel transaction logging

### Phase 4: Provider Chain Extension (sm0l, ~1hr)
- Extend ralph.sh `PROVIDER_CHAIN` to support data providers
- Add `PROVIDER_DATA_CHAIN` for data streaming
- Implement consumer attachment pattern

<!-- b00t:map v1
summary: ledg3rr syntax + grok chained provider architecture research
tags: grok, ledg3rr, provider-chain, subscription, datum-node, irontology
tier: ch0nky
cmds: b00t grok ask, b00t grok digest, b00t grok status
complexity: 7
-->
