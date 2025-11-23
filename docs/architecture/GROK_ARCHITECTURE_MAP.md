# b00t Grok Architecture Map & Refactoring Plan

**Date:** 2025-11-10
**Status:** 🔨 Architectural Analysis & Design Phase

---

## Current Architecture Analysis

### Datum Types (from `b00t-cli/src/lib.rs:115-127`)

```rust
pub enum DatumType {
    Unknown,
    Mcp,      // Model Context Protocol servers
    Bash,     // Shell scripts
    Vscode,   // VSCode extensions
    Docker,   // Container images
    K8s,      // Kubernetes/Helm charts
    Apt,      // APT packages
    Nix,      // Nix packages
    Ai,       // AI providers (OpenAI, Anthropic, Ollama, etc.)
    Cli,      // CLI tools
    Stack,    // Composed stacks of other datums
}
```

### Current Grok Dependency Chain

```
grok-guru.mcp.toml
├─ depends_on: ["qdrant.docker", "ollama.docker"]
├─ Environment Expectations:
│  ├─ QDRANT_URL (from qdrant.docker)
│  ├─ OLLAMA_URL (from ollama.docker)
│  ├─ EMBEDDING_PROVIDER ("openai" | "ollama")
│  ├─ OPENAI_API_KEY (if provider=openai)
│  └─ OLLAMA_API_URL (if provider=ollama)
└─ Python Config (b00t_grok_guru/config.py)
   ├─ get_embedding_func() → Callable
   ├─ get_llm_func() → Callable
   └─ get_vision_func() → Callable
```

### Existing AI Datums

```toml
# ollama.ai.toml
[b00t]
name = "ollama"
type = "ai"

[models.llama2]
capabilities = "text,chat"
context_length = 4096
cost_per_token = 0.0

[env]
OPENAI_API_BASE = "http://localhost:11434"
OPENAI_API_KEY = ""
OLLAMA_HOST = "localhost:11434"

# openai.ai.toml
[b00t]
name = "openai"
type = "ai"

[models."gpt-4-turbo"]
capabilities = "text,chat,vision,json"
context_length = 128000
cost_per_1k_tokens = 0.01

[env]
OPENAI_API_BASE = "https://api.openai.com/v1"
OPENAI_API_KEY = "${OPENAI_API_KEY}"

# anthropic.ai.toml
[b00t]
name = "anthropic"
type = "ai"

[models."claude-3-5-sonnet-20241022"]
capabilities = "text,vision,artifacts,computer_use"
context_length = 200000
cost_per_1k_input_tokens = 0.003
cost_per_1k_output_tokens = 0.015

[env]
ANTHROPIC_API_BASE = "https://api.anthropic.com"
ANTHROPIC_API_KEY = "${ANTHROPIC_API_KEY}"
```

---

## The Problem: Missing Indirection Layer

### Current Issues

1. **❌ Direct Provider Coupling**: Grok directly depends on `ollama.docker` but Ollama is just **one way** to provide an OpenAI-compatible API
2. **❌ No API Abstraction**: No datum represents "an OpenAI-compatible embedding API endpoint" abstractly
3. **❌ Missing Model Validation**: No check if the provider can actually serve the required embedding model
4. **❌ Local vs Remote Conflation**: ollama.docker provides *infrastructure*, but ollama.ai provides *API access* - these are different concerns
5. **❌ No Fallback Chain**: Cannot express "try Ollama local, fallback to OpenAI remote"

### What's Missing

**API Protocol Datum Type**: Need a way to describe:
- "This is an OpenAI-compatible API endpoint"
- "It supports embedding model X"
- "It's provided by service Y (which may be local or remote)"
- "It requires API key Z (or not, if local)"

---

## Proposed Refactoring: Three-Layer Architecture

### Layer 1: Infrastructure (Docker/K8s/Process)

**Purpose:** Provides the *runtime* for AI services

```toml
# ollama.docker.toml (EXISTING - NO CHANGE)
[b00t]
name = "ollama"
type = "docker"
image = "docker.io/ollama/ollama:latest"

[b00t.env]
OLLAMA_HOST = "0.0.0.0:11434"
OLLAMA_API_URL = "http://localhost:11434"
OLLAMA_URL = "http://localhost:11434"
```

```toml
# vllm.docker.toml (NEW - EXAMPLE)
[b00t]
name = "vllm"
type = "docker"
hint = "vLLM OpenAI-compatible inference server"
image = "vllm/vllm-openai:latest"
docker_args = ["-p", "8000:8000", "--gpus", "all"]

[b00t.env]
VLLM_API_URL = "http://localhost:8000"
```

### Layer 2: API Protocol (NEW DatumType Needed)

**Purpose:** Describes the *API capabilities* independent of infrastructure

**Option A: New `Api` DatumType**

```rust
// b00t-cli/src/lib.rs
pub enum DatumType {
    // ... existing types
    Api,  // NEW: API protocol endpoints
}
```

```toml
# ollama-embeddings.api.toml (NEW)
[b00t]
name = "ollama-embeddings"
type = "api"
hint = "Ollama OpenAI-compatible embeddings API"
protocol = "openai-embeddings-v1"
depends_on = ["ollama.docker"]  # Infrastructure dependency

[b00t.capabilities]
# Embedding models that MUST be loaded/available
required_models = ["nomic-embed-text"]
embedding_dimensions = 768
max_batch_size = 32

[b00t.env]
# Maps to standard OpenAI env vars
OPENAI_API_BASE = "${OLLAMA_API_URL}"  # Inherited from ollama.docker
OPENAI_API_KEY = ""  # Local = no key needed
EMBEDDING_PROVIDER = "ollama"  # For Python config
EMBEDDING_MODEL = "nomic-embed-text"
```

```toml
# openai-embeddings.api.toml (NEW)
[b00t]
name = "openai-embeddings"
type = "api"
hint = "OpenAI official embeddings API (remote)"
protocol = "openai-embeddings-v1"
depends_on = []  # No infrastructure - remote service

[b00t.capabilities]
available_models = ["text-embedding-3-small", "text-embedding-3-large", "text-embedding-ada-002"]
embedding_dimensions = [1536, 3072, 1536]

[b00t.env]
OPENAI_API_BASE = "https://api.openai.com/v1"
OPENAI_API_KEY = "${OPENAI_API_KEY}"  # REQUIRED from environment
EMBEDDING_PROVIDER = "openai"
EMBEDDING_MODEL = "text-embedding-3-small"
```

**Option B: Extend `Ai` Datum with Protocol Field**

```toml
# ollama-embeddings.ai.toml (EXTENDED)
[b00t]
name = "ollama-embeddings"
type = "ai"
protocol = "openai-embeddings-v1"  # NEW FIELD
depends_on = ["ollama.docker"]

[b00t.provides]  # NEW SECTION
services = ["embeddings"]
models = ["nomic-embed-text"]

[env]
OPENAI_API_BASE = "${OLLAMA_API_URL}"
EMBEDDING_PROVIDER = "ollama"
```

### Layer 3: Application/MCP (Consumer)

**Purpose:** Declares *what it needs*, not *how to provide it*

```toml
# grok-guru.mcp.toml (REFACTORED)
[b00t]
name = "grok-guru"
type = "mcp"
hint = "b00t grok RAG knowledgebase with vector search"

# CHANGED: Depend on API abstractions, not infrastructure
depends_on = [
    "qdrant.docker",           # Vector DB infrastructure
    "ollama-embeddings.api",   # Embedding API (prefers local)
    # OR: "openai-embeddings.api" as fallback
]

# Alternative: Dependency with preferences
[b00t.dependencies]
vector_db = { required = "qdrant.docker" }
embeddings = {
    prefer = ["ollama-embeddings.api", "openai-embeddings.api"],
    fallback = "openai-embeddings.api"
}

[b00t.env]
# Inherited from dependencies automatically
# - QDRANT_URL from qdrant.docker
# - OPENAI_API_BASE from chosen embeddings API
# - EMBEDDING_PROVIDER from chosen embeddings API
```

---

## Orchestrator Enhancements Needed

### Current Orchestrator (`b00t-cli/src/orchestrator.rs`)

```rust
// Current: Only starts infrastructure
pub async fn ensure_dependencies(&self, datum_key: &str) -> Result<Vec<String>> {
    for dep_key in dep_keys {
        if self.needs_start(dep_datum).await? {
            self.start_service(dep_datum).await?;  // Docker/K8s only
        }
    }
}
```

### Enhanced Orchestrator (Proposed)

```rust
pub async fn ensure_dependencies(&self, datum_key: &str) -> Result<DependencyReport> {
    let mut report = DependencyReport::default();

    for dep_key in dep_keys {
        match dep_datum.datum_type {
            DatumType::Docker | DatumType::K8s => {
                // Start infrastructure
                if self.needs_start(dep_datum).await? {
                    self.start_service(dep_datum).await?;
                    report.services_started.push(dep_key);
                }
            }
            DatumType::Api => {
                // Validate API availability
                if !self.validate_api(dep_datum).await? {
                    // Try fallback or fail
                    report.warnings.push(format!("{} unavailable", dep_key));
                }
            }
            _ => {}
        }
    }

    Ok(report)
}

async fn validate_api(&self, datum: &BootDatum) -> Result<bool> {
    // Check if API is reachable
    // Check if required models are loaded
    // Verify API key if needed
}
```

---

## Migration Path

### Phase 1: Add API Datum Type ✅

1. Add `Api` to `DatumType` enum
2. Create `datum_api.rs` module
3. Implement API validation logic

### Phase 2: Create API Datums ✅

1. `ollama-embeddings.api.toml`
2. `openai-embeddings.api.toml`
3. `vllm-embeddings.api.toml` (if needed)
4. `litellm-embeddings.api.toml` (if needed)

### Phase 3: Refactor Grok Dependencies ✅

1. Update `grok-guru.mcp.toml` to depend on `ollama-embeddings.api`
2. Update `grok.stack.toml` to include API datums
3. Remove direct `ollama.docker` dependency from grok-guru

### Phase 4: Enhance Orchestrator ✅

1. Add API validation to orchestrator
2. Implement model availability checks
3. Add fallback logic

### Phase 5: Test End-to-End ✅

1. Cold start with no services
2. Warm start with services running
3. API key validation
4. Model availability checks

---

## Benefits of This Architecture

### ✅ Separation of Concerns
- **Infrastructure** (Docker): "How do I run this?"
- **API Protocol** (Api): "What can I do?"
- **Application** (MCP): "What do I need?"

### ✅ Flexibility
```toml
# Can swap infrastructure without changing consumers
ollama-embeddings.api → depends_on: ["ollama.docker"]
# OR
ollama-embeddings.api → depends_on: ["vllm.docker"]
# Consumer (grok-guru) doesn't care!
```

### ✅ DRY Principle Maintained
- Infrastructure datum defines its env vars (SOURCE OF TRUTH)
- API datum passes through with `${VAR}` syntax
- Consumer inherits automatically

### ✅ Fallback Chains
```toml
depends_on = [
    "ollama-embeddings.api",   # Try local first
    "openai-embeddings.api"    # Fallback to cloud
]
```

### ✅ Model Validation
```rust
// Orchestrator can verify:
// - Is nomic-embed-text actually loaded in Ollama?
// - Does the API key work for OpenAI?
// - Is the model available on this plan?
```

---

## Example: Complete Grok Stack (After Refactoring)

```
grok.stack
├─ qdrant.docker (vector DB infrastructure)
├─ ollama.docker (LLM inference infrastructure)
├─ ollama-embeddings.api (embedding API protocol)
│  └─ depends_on: ollama.docker
│  └─ provides: OpenAI-compatible embeddings
│  └─ models: ["nomic-embed-text"]
└─ grok-guru.mcp (application)
   └─ depends_on: ["qdrant.docker", "ollama-embeddings.api"]
   └─ inherits: QDRANT_URL, OPENAI_API_BASE, EMBEDDING_PROVIDER
```

**Orchestration Flow:**
1. User: `b00t grok learn "content"`
2. Orchestrator: Check `grok-guru.mcp` dependencies
3. Orchestrator: Start `qdrant.docker` if needed ✅
4. Orchestrator: Start `ollama.docker` if needed (via `ollama-embeddings.api` dependency) ✅
5. Orchestrator: Validate `ollama-embeddings.api` is accessible ✅
6. Orchestrator: Check `nomic-embed-text` model is loaded ✅
7. Orchestrator: Pass through environment variables ✅
8. Execute: `grok-guru` MCP server starts with correct config ✅

---

## Advanced: Multi-Level API Composition

### Concept: APIs Can Depend on Other APIs

**Key Insight:** Just as MCPs depend on Docker services, API datums can depend on OTHER API datums, creating **protocol layering** and **capability composition**.

#### Example 1: Protocol Implementation Hierarchy

```toml
# openai-compat-base.api.toml (BASE PROTOCOL)
[b00t]
name = "openai-compat-base"
type = "api"
hint = "Base OpenAI-compatible protocol specification"
protocol_version = "v1"

[b00t.provides]
endpoints = ["/v1/embeddings", "/v1/chat/completions", "/v1/completions"]
authentication = "bearer-token"

# This is an ABSTRACT protocol - no actual implementation

---

# ollama-api.api.toml (CONCRETE IMPLEMENTATION)
[b00t]
name = "ollama-api"
type = "api"
hint = "Ollama implements OpenAI-compatible protocol"
depends_on = ["ollama.docker"]  # Infrastructure
implements = "openai-compat-base.api"  # Protocol compliance

[b00t.provides]
protocol = "openai-compat-base"
capabilities = ["embeddings", "chat", "completions"]
models = {
    embeddings = ["nomic-embed-text", "all-minilm"],
    chat = ["llama3.2", "mistral"],
}

[b00t.env]
OPENAI_API_BASE = "${OLLAMA_API_URL}"
OPENAI_API_KEY = ""  # Local, no key needed
```

#### Example 2: Service Composition (APIs requiring APIs)

```toml
# embedding-api.api.toml (CAPABILITY INTERFACE)
[b00t]
name = "embedding-api"
type = "api"
hint = "Generic embedding capability interface"
protocol = "embeddings-v1"

[b00t.provides]
capability = "embeddings"
required_methods = ["embed_text", "embed_batch"]

[b00t.requires]
# This API requires NOTHING - it's a pure interface

---

# vector-db-api.api.toml (CAPABILITY INTERFACE)
[b00t]
name = "vector-db-api"
type = "api"
hint = "Vector database capability interface"
protocol = "vector-db-v1"

[b00t.provides]
capability = "vector-search"
required_methods = ["upsert", "search", "delete"]

---

# rag-api.api.toml (COMPOSITE API)
[b00t]
name = "rag-api"
type = "api"
hint = "Complete RAG system API composition"
protocol = "rag-v1"

[b00t.requires]
# This API REQUIRES other APIs!
embedding_api = { capability = "embeddings" }
vector_api = { capability = "vector-search" }

[b00t.provides]
capability = "rag"
operations = ["ingest", "query", "update"]

# Satisfied by any embedding provider
[b00t.dependencies]
embedding_api = {
    prefer = ["ollama-embeddings.api", "openai-embeddings.api"],
    requires_capability = "embeddings"
}
vector_api = {
    required = "qdrant-api.api"
}
```

#### Example 3: Concrete Implementation with API Dependencies

```toml
# grok-guru.mcp.toml (REFACTORED WITH API COMPOSITION)
[b00t]
name = "grok-guru"
type = "mcp"
hint = "b00t grok RAG knowledgebase"

[b00t.requires]
# Express requirements as CAPABILITIES, not implementations
rag_system = { capability = "rag" }
# OR more granular:
# embeddings = { capability = "embeddings", protocol = "openai-compat-base" }
# vector_db = { capability = "vector-search" }

[b00t.dependencies]
# Resolved to concrete implementations
rag_system = "rag-api.api"  # Which itself requires embeddings + vector-db
# Orchestrator expands this recursively:
#   rag-api → embedding-api → ollama-api → ollama.docker
#   rag-api → vector-db-api → qdrant.docker
```

### Dependency Resolution Examples

#### Simple Chain: MCP → API → Docker
```
grok-guru.mcp
└─ requires: embedding-api (capability)
   └─ resolved_to: ollama-embeddings.api
      └─ depends_on: ollama.docker
         └─ type: docker (infrastructure)
```

#### Complex Graph: Composite API Dependencies
```
grok-guru.mcp
└─ requires: rag-api (capability)
   ├─ rag-api.api (composite)
   │  ├─ requires: embedding-api
   │  │  └─ resolved_to: ollama-embeddings.api
   │  │     └─ depends_on: ollama.docker
   │  └─ requires: vector-db-api
   │     └─ resolved_to: qdrant-api.api
   │        └─ depends_on: qdrant.docker
   └─ All infrastructure started: [ollama.docker, qdrant.docker]
```

#### Fallback Chain with API Alternatives
```
grok-guru.mcp
└─ requires: embedding-api (capability)
   ├─ try: ollama-embeddings.api
   │  └─ depends_on: ollama.docker
   │     └─ status: NOT RUNNING → try fallback
   └─ fallback: openai-embeddings.api
      └─ depends_on: [] (remote service)
      └─ env_check: OPENAI_API_KEY → FOUND ✓
      └─ status: AVAILABLE ✓
```

### Orchestrator Enhancement: Recursive Resolution

```rust
// Enhanced orchestrator with recursive API resolution
pub struct DependencyGraph {
    nodes: HashMap<String, DatumNode>,
    edges: Vec<(String, String, DependencyType)>,
}

enum DependencyType {
    Infrastructure,  // Must start this service
    Protocol,        // Must implement this protocol
    Capability,      // Must provide this capability
}

impl Orchestrator {
    pub async fn resolve_dependencies(
        &self,
        datum_key: &str
    ) -> Result<DependencyGraph> {
        let mut graph = DependencyGraph::new();
        self.resolve_recursive(datum_key, &mut graph).await?;
        Ok(graph)
    }

    async fn resolve_recursive(
        &self,
        datum_key: &str,
        graph: &mut DependencyGraph
    ) -> Result<()> {
        let datum = self.datums.get(datum_key)?;

        match datum.datum_type {
            DatumType::Mcp => {
                // MCP requires APIs by capability
                for capability in &datum.requires_capabilities {
                    let api_datum = self.find_provider(capability)?;
                    graph.add_edge(datum_key, api_datum, DependencyType::Capability);
                    self.resolve_recursive(api_datum, graph).await?;
                }
            }
            DatumType::Api => {
                // API may require other APIs
                for required_api in &datum.requires_apis {
                    graph.add_edge(datum_key, required_api, DependencyType::Protocol);
                    self.resolve_recursive(required_api, graph).await?;
                }
                // API may depend on infrastructure
                for infra in &datum.depends_on {
                    graph.add_edge(datum_key, infra, DependencyType::Infrastructure);
                    self.resolve_recursive(infra, graph).await?;
                }
            }
            DatumType::Docker | DatumType::K8s => {
                // Infrastructure is a leaf node
            }
            _ => {}
        }

        Ok(())
    }
}
```

### Benefits of Multi-Level API Composition

#### 1. Protocol Abstraction
```toml
# Consumer doesn't care if it's Ollama, vLLM, or OpenAI
[b00t.requires]
embeddings = { protocol = "openai-compat-base" }
# Any implementation of that protocol works!
```

#### 2. Capability-Based Selection
```toml
# Consumer specifies WHAT it needs, not HOW
[b00t.requires]
rag = { capabilities = ["embeddings", "vector-search"] }
# Orchestrator finds ANY datum providing those capabilities
```

#### 3. Layered Services
```toml
# High-level API built from lower-level APIs
rag-api.api → embedding-api.api + vector-db-api.api
# Consumers get complex functionality from simple interface
```

#### 4. Mix-and-Match Infrastructure
```toml
# Same API, different backends
ollama-embeddings.api → ollama.docker  # Local
openai-embeddings.api → remote         # Cloud
vllm-embeddings.api → vllm.docker      # Alternative local
# All satisfy "embedding-api" capability!
```

#### 5. Graceful Degradation
```toml
[b00t.dependencies]
embeddings = {
    prefer = ["local-gpu.api", "local-cpu.api"],
    fallback = "cloud-paid.api",
    ultimate_fallback = "mock-embeddings.api"  # For testing
}
```

### Example: Complete Stack with API Composition

```
ai-dev-stack.stack
├─ base-protocols/
│  ├─ openai-compat.api (abstract protocol)
│  └─ embeddings-interface.api (abstract capability)
├─ infrastructure/
│  ├─ ollama.docker → provides: ollama-api.api
│  ├─ vllm.docker → provides: vllm-api.api
│  └─ qdrant.docker → provides: qdrant-api.api
├─ api-implementations/
│  ├─ ollama-embeddings.api
│  │  ├─ implements: embeddings-interface.api
│  │  ├─ implements: openai-compat.api
│  │  └─ depends_on: ollama.docker
│  ├─ qdrant-api.api
│  │  ├─ implements: vector-db-interface.api
│  │  └─ depends_on: qdrant.docker
│  └─ rag-api.api (composite)
│     ├─ requires: embeddings-interface.api
│     ├─ requires: vector-db-interface.api
│     └─ provides: rag-capability
└─ applications/
   ├─ grok-guru.mcp
   │  └─ requires: rag-capability
   └─ code-search.mcp
      └─ requires: embeddings-interface.api
```

**Resolution Example:**
```
User: b00t grok learn "content"
├─ grok-guru.mcp requires { rag-capability }
├─ Resolved to: rag-api.api
│  ├─ rag-api requires { embeddings-interface }
│  │  ├─ Resolved to: ollama-embeddings.api
│  │  │  └─ depends_on: ollama.docker → START
│  ├─ rag-api requires { vector-db-interface }
│  │  └─ Resolved to: qdrant-api.api
│  │     └─ depends_on: qdrant.docker → START
└─ Services started: [ollama.docker ✓, qdrant.docker ✓]
└─ APIs validated: [ollama-embeddings.api ✓, qdrant-api.api ✓, rag-api.api ✓]
└─ Execute: grok-guru.mcp
```

---

## Open Questions for User

1. **New `Api` DatumType vs Extended `Ai`?**
   - Option A: New `Api` type for protocol endpoints
   - Option B: Extend `Ai` type with `protocol` and `provides` fields

2. **Fallback Syntax?**
   ```toml
   # Simple list (try in order)
   depends_on = ["ollama-embeddings.api", "openai-embeddings.api"]

   # OR explicit preferences
   [b00t.dependencies]
   embeddings = { prefer = "ollama-embeddings.api", fallback = "openai-embeddings.api" }
   ```

3. **Model Validation Depth?**
   - Just check API reachable?
   - Verify specific models loaded?
   - Test actual embedding call?

4. **Error Handling?**
   - Fail fast if primary unavailable?
   - Auto-fallback silently?
   - Warn and prompt user?

---

**Next Step:** Get user feedback on architectural direction before implementation.
