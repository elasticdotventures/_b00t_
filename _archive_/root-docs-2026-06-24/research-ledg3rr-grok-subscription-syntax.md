# Research Summary: ledg3rr Logic Syntax & Grok Chained Provider Pattern

**Date**: 2026-05-03
**Researcher**: Hermes Agent (subagent)
**Scope**: ledg3rr implementation, grok handlers, consumer subscription to data shapes

---

## 1. Current State

### 1.1 ledg3rr (l3dg3rr) Implementation

**Location**: `b00t-l3dg3rr-viz/` crate + governance design docs

**Core Pattern**: Invariant-based governance using Rust traits

```rust
// From: docs/superpowers/specs/2026-05-02-b00t-task-next-l3dg3rr-governance-design.md

pub trait TaskQueueInvariant {
    fn state(&self) -> TaskQueueState;
    fn is_valid_transition(&self, from: TaskQueueState, to: TaskQueueState) -> bool;
}

pub trait GovernanceGate {
    fn check(&self, context: &GovernanceContext) -> Result<(), GovernanceError>;
    fn reason(&self) -> String;
}
```

**Key Components**:
- `InvariantGraph` - Typed invariant visualization with Mermaid/SVG rendering
- `InvariantNode` / `InvariantEdge` - Graph primitives with validation
- `TransactionLog` - Audit trail with OTel span integration
- `VisualizationRole` enum: Ingest, Validate, Classify, Review, Reconcile, Commit, Decision, Step

**Formal Verification**: Designed for integration with kani, z3, kausari (mentioned in context but not yet implemented)

### 1.2 Grok Chained Provider Pattern

**Location**: `b00t-c0re-lib/src/dual_grok.rs`, `b00t-c0re-lib/src/grok.rs`

**DualGrokClient Pattern**:
```rust
pub struct DualGrokClient {
    iron: Option<IrontologyBridgeClient>,  // Lazy init, non-fatal on failure
}

pub enum GrokBackend {
    Raglite,     // Python subprocess (Qdrant + Ollama)
    Irontology,  // Rust native (NeumannStore + 4-way fusion)
    Both,        // Fan-out with dedup (default)
}
```

**Provider Chain in ralph.sh**:
```bash
PROVIDER_CHAIN="llama-cpp,openai-compatible"

provider_chain_contains() {
    local needle
    needle="$(canonical_provider_name "${1:-}")"
    # Iterates through comma-separated provider chain
    for provider in "${_providers[@]}"; do
        [[ "$(canonical_provider_name "${provider}")" == "${needle}" ]] && return 0
    done
    return 1
}
```

**IrontologyBridgeClient** - Semantic mapping:
```rust
// Maps b00t datum schema to irontology semantic layer
pub struct DatumNode {
    pub topic: String,      // b00t datum name
    pub class: String,      // OWL class label
    pub content: String,    // Primary content
    pub tags: Vec<String>,  // Searchable tags
    pub predicates: Vec<(String, String)>,  // RDF-style triples
}

// Converts to FactRecord triples:
// subject = b00t:datum/<topic>/<uuid>
// predicate = b00t:hasContent | b00t:hasTag | b00t:hasClass
```

### 1.3 RHAI Engine (ledg3rr Harmony)

**Location**: `b00t-c0re-lib/src/rhai_engine.rs`

**Capabilities**:
- Dynamic script execution with b00t context
- Registered functions: `run_cmd`, `run_cmd_if`, `install_package`, `file_exists`, `create_dir`, `write_file`, `read_file`
- Context variables: `PID`, `TIMESTAMP`, `USER`, `BRANCH`, `AGENT`, `MODEL_SIZE`, `PRIVACY`, `WORKSPACE_ROOT`

**Integration with irontology-mcp**:
```toml
# vendor/irontology-mcp/examples/acme-corp/phase2d.toml
rhai_modules = ["latent_dependencies"]
[[registries.rhai_modules]]
name = "latent_dependencies"
path = "rhai/latent_dependencies.rhai"
```

---

## 2. What's Missing

### 2.1 Consumer Subscription Pattern

**Gap**: No mechanism for consumers to subscribe to data shape changes.

**Current State**:
- Grok supports `ask/digest/learn/status` - all pull-based
- No push notifications when data shapes change
- No streaming/chained consumer pattern

**Needed**:
1. Subscription registry for consumers
2. Shape change detection and notification
3. Consumer callback/streaming interface

### 2.2 Data Shape Definition

**Gap**: SHACL shapes defined in irontology-mcp but not exposed to b00t consumers.

**Current State** (irontology-mcp PRD):
```turtle
# SHACL Shape for Receipt (in irontology-mcp)
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix receipt: <http://example.org/receipt#> .

receipt:ReceiptShape a sh:NodeShape ;
    sh:targetClass receipt:Receipt ;
    sh:property [
        sh:path receipt:vendor ;
        sh:minCount 1 ;
        sh:datatype xsd:string ;
    ] .
```

**Needed**:
1. Shape subscription API
2. Shape validation at consumer boundary
3. Shape evolution/compatibility checking

### 2.3 Chained Provider Stream

**Gap**: Provider chain exists in ralph.sh for inference, but not for data streaming.

**Current**: `PROVIDER_CHAIN="llama-cpp,openai-compatible"` - model selection only

**Needed**:
1. Data provider chain pattern
2. Stream transformation/throughput
3. Consumer attachment to chain

---

## 3. Proposed Syntax for Consumer Subscription to Data Shapes

### 3.1 Declarative Subscription Syntax (TOML)

```toml
# ~/.b00t/subscriptions.d/inventory-updates.subscription.toml

[subscription]
id = "sub-inventory-001"
consumer = "inventory-sync-agent"
enabled = true

[shape]
# Subscribe to shape changes
shape_uri = "shape:InventoryItemShape"
# Or subscribe to topic changes
topic = "inventory"
# Validation level: strict | lenient | none
validation = "strict"

[trigger]
# Trigger on: create | update | delete | all
on = ["create", "update"]
# Debounce window (ms)
debounce_ms = 500

[delivery]
# Delivery method: webhook | stream | poll | mcp
method = "stream"
# For stream: buffer size
buffer_size = 100
# For webhook: callback URL
# webhook_url = "http://localhost:8080/inventory/webhook"

[governance]
# Optional: governance gate to check before delivery
gate = "CanReceiveInventoryUpdates"
# Require transaction proof
require_proof = true
```

### 3.2 RHAI Script Syntax for Dynamic Subscriptions

```rhai
// scripts/inventory_subscription.rhai

// Define subscription with invariant check
let sub = subscription(
    topic: "inventory",
    shape: "InventoryItemShape",
    on: ["create", "update"]
);

// Attach handler with governance
sub.on_data(|datum, tx_proof| {
    // Invariant: only process if quantity changed
    if datum.has_predicate("quantity_changed") {
        // Validate shape before processing
        if validate_shape(datum, "InventoryItemShape") {
            log_info(`Processing inventory update: ${datum.topic}`);
            
            // Emit to downstream consumer
            emit("inventory-sync", datum);
            
            // Record in transaction log
            tx_proof.add_step("processed", true);
        }
    }
});

// Governance gate: only subscribe if local queue empty
if queue_empty() {
    sub.activate();
} else {
    log_warn("Subscription deferred: queue not empty");
}
```

### 3.3 Rust Trait Definition for Consumer

```rust
// b00t-c0re-lib/src/subscription.rs

/// A consumer that subscribes to data shape changes
pub trait DataShapeConsumer: Send + Sync {
    /// Unique consumer identifier
    fn id(&self) -> &str;
    
    /// Shapes this consumer is interested in
    fn subscribed_shapes(&self) -> Vec<ShapeUri>;
    
    /// Topics this consumer is interested in
    fn subscribed_topics(&self) -> Vec<String>;
    
    /// Called when a datum matching subscription is created/updated
    fn on_datum(&self, datum: &DatumNode, proof: &TransactionProof) -> Result<(), ConsumerError>;
    
    /// Called when a shape definition changes
    fn on_shape_evolution(&self, old_shape: &Shape, new_shape: &Shape) -> Result<(), ShapeError>;
    
    /// Validation level for incoming data
    fn validation_level(&self) -> ValidationLevel;
}

/// Subscription configuration
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

/// Subscription manager
pub struct SubscriptionManager {
    subscriptions: HashMap<String, SubscriptionConfig>,
    consumers: HashMap<String, Arc<dyn DataShapeConsumer>>,
    store: Arc<NeumannStore>,
}

impl SubscriptionManager {
    /// Register a consumer with subscriptions
    pub fn register_consumer(&mut self, consumer: Arc<dyn DataShapeConsumer>) -> Result<()>;
    
    /// Notify consumers of datum change
    pub async fn notify_datum(&self, datum: &DatumNode, event: DatumEvent) -> Result<()>;
    
    /// Stream subscription for a consumer
    pub fn stream(&self, consumer_id: &str) -> Receiver<DatumEvent>;
}
```

### 3.4 CLI Syntax for Subscription Management

```bash
# Create subscription
b00t subscription create \
    --topic inventory \
    --shape InventoryItemShape \
    --consumer inventory-sync-agent \
    --on create,update \
    --delivery stream \
    --debounce 500

# List subscriptions
b00t subscription list --consumer inventory-sync-agent

# Pause/resume subscription
b00t subscription pause sub-inventory-001
b00t subscription resume sub-inventory-001

# Stream events (blocking)
b00t subscription stream sub-inventory-001

# Validate datum against subscribed shapes
b00t subscription validate sub-inventory-001 --datum '{"topic":"inventory",...}'
```

### 3.5 MCP Tool Interface

```rust
// b00t-mcp/src/subscription_mcp_tools.rs

/// MCP tool: Create subscription
#[derive(Parser, Clone)]
pub struct SubscriptionCreateCommand {
    #[arg(long)]
    pub topic: Option<String>,
    
    #[arg(long)]
    pub shape: Option<String>,
    
    #[arg(long)]
    pub consumer: String,
    
    #[arg(long, value_delimiter = ',')]
    pub on: Vec<String>,
    
    #[arg(long, default_value = "stream")]
    pub delivery: String,
}

impl_mcp_tool!(SubscriptionCreateCommand, "b00t_subscription_create", ["subscription", "create"]);

/// MCP tool: Stream subscription events
#[derive(Parser, Clone)]
pub struct SubscriptionStreamCommand {
    #[arg(help = "Subscription ID")]
    pub subscription_id: String,
    
    #[arg(long, default_value = "100")]
    pub buffer: usize,
}

impl_mcp_tool!(SubscriptionStreamCommand, "b00t_subscription_stream", ["subscription", "stream"]);
```

---

## 4. Integration Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│ b00t-cli (surface)                                              │
│ ├─ subscription create/list/pause/resume/stream                 │
│ └─ calls b00t-mcp subscription tools                            │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│ b00t-mcp (MCP server)                                           │
│ ├─ subscription_mcp_tools.rs                                    │
│ │  └─ create, list, pause, resume, stream, validate            │
│ └─ calls SubscriptionManager                                    │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│ b00t-c0re-lib (core)                                            │
│ ├─ subscription.rs                                              │
│ │  ├─ DataShapeConsumer trait                                   │
│ │  ├─ SubscriptionManager                                       │
│ │  └─ SubscriptionConfig                                        │
│ ├─ irontology_bridge.rs (existing)                              │
│ │  └─ DatumNode, IrontologyBridgeClient                        │
│ └─ l3dg3rr integration                                          │
│    ├─ TransactionProof for each delivery                        │
│    └─ GovernanceGate checks before notification                 │
└─────────────────────────────┬───────────────────────────────────┘
                              │
┌─────────────────────────────▼───────────────────────────────────┐
│ Storage Layer                                                   │
│ ├─ NeumannStore (sled-backed)                                   │
│ │  ├─ Facts: b00t:datum/<topic>/<uuid>                         │
│ │  └─ Subscriptions: b00t:subscription/<id>                    │
│ └─ Shape Registry (SHACL shapes)                                │
│    └─ shape:InventoryItemShape, shape:ReceiptShape, ...        │
└─────────────────────────────────────────────────────────────────┘
```

---

## 5. Proposed Implementation Phases

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

### Phase 4: MCP Tools (sm0l, ~1hr)
- Add `subscription_mcp_tools.rs`
- Implement CLI commands

### Phase 5: RHAI Integration (sm0l, ~1hr)
- Add subscription functions to RhaiEngine
- Enable dynamic subscription scripts

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
| `b00t-mcp/src/mcp_tools.rs` | MCP tool definitions |
| `b00t-mcp/src/rag_mcp_tools.rs` | RAGLight MCP tools |
| `vendor/irontology-mcp/PRD-1.md` | SHACL shapes design |
| `docs/superpowers/specs/...governance-design.md` | l3dg3rr governance design |
| `ralphs/ralph-plus-_b00t_/ralph.sh` | Provider chain pattern |

---

## 7. Summary

The b00t ecosystem has a solid foundation for:
- **Dual-backend grok** with fan-out and graceful degradation
- **l3dg3rr governance** with trait-based invariants
- **RHAI scripting** for dynamic logic
- **Provider chains** in ralph.sh for model selection

**Missing pieces for consumer subscription**:
1. Subscription registry and lifecycle management
2. Shape change detection and notification
3. Consumer callback/streaming interface
4. Governance integration at subscription boundary

The proposed syntax combines:
- TOML for declarative configuration
- RHAI for dynamic subscription scripts
- Rust traits for type-safe consumers
- CLI/MCP tools for management

This aligns with the existing l3dg3rr harmony pattern: idiomatic Rust generic/trait invariant patterns with formal verification hooks.
