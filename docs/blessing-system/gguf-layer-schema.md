# GGUF Layer Schema for Blessing Composition

## Overview

GGUF (GPT-Generated Unified Format) layers are modular model artifacts that encapsulate blessing-specific knowledge and capabilities. Each layer represents a specialized adapter or fine-tuned configuration stacked onto a base model. Blessing layers enable the composition of multi-capability models where each blessing (authorization to execute, access data, or perform actions) has a corresponding layer that constrains and optimizes model behavior.

A blessing layer is a GGUF artifact paired with metadata (blessing ID, embedding dimension, adapter rank, quality score). When an agent requests a blessing, the system composes one or more layers with the base model to produce a constrained inference runtime. This design follows the principle of "principle of least privilege" applied to model capabilities: the model only has access to the knowledge and decision-making context required for that specific blessing.

Model layer composition is the process of stacking adapter layers (LoRA, prefix-tuning, or adapter modules) onto a base model. Each layer adds specialized knowledge for a specific domain or capability (e.g., Terraform apply operations, AWS credential handling). The composition plan defines which layers to activate, in what order, and with what resource constraints, ensuring the final model respects the blessing's execution scope, data permissions, and resource budgets.

---

## Blessing Node Extension: Layer Schema

Each blessing node in the blessing graph extends the standard structure with layer metadata and composition hints:

```toml
# blessing-terraform-apply.toml
[[b00t.blessings]]
id = "blessing-terraform-apply"
type = "infrastructure"
datum = "terraform"
cost_tokens = 512
role_access = ["orchestrator", "executive"]
requires = []  # No prerequisites

# Trifecta Component 1: Usage Notes
usage_notes = """
Blessing for 'terraform apply' operations on AWS infrastructure.
Requires prior approval from infrastructure team.
Only applies to blessed Terraform configurations.
"""

# Trifecta Component 2: Execute Access
[b00t.blessings.execute_access]
binary = "/usr/local/bin/terraform"
timeout_seconds = 600
max_cpu_percent = 50
max_memory_mb = 512

[[b00t.blessings.execute_access.bash_filters]]
role = "orchestrator"
allowed_commands = ["terraform", "terraform-docs"]
deny_by_default = true

# Trifecta Component 3: Data Permissions
[b00t.blessings.data_permissions]
readable_paths = [
  "~/.terraform/",
  "/etc/terraform/",
  "./infrastructure/*.tf"
]
writable_paths = [
  "~/.terraform/state/",
  "./infrastructure/.terraform/"
]
blocked_paths = [
  "/etc/shadow",
  "~/.ssh/",
  "/root/.aws/"
]
network_allowed_hosts = ["api.terraform.io", "registry.terraform.io"]

# Layer Metadata: GGUF artifact specification
[b00t.blessings.layer_metadata]
blessing_id = "blessing-terraform-apply"
artifact_path = "/cache/blessings/layers/blessing-terraform-apply.gguf"
embedding_dim = 768
adapter_rank = 16
quality_score = 0.92
generated_at = "2026-04-01T10:00:00Z"
```

---

## Layer File Structure

GGUF layers are organized in a knowledge index directory with parallel TOML configurations:

```
knowledge_index/
├── layers/
│   ├── blessing-terraform-apply.gguf          # LoRA adapter (TensorFlow)
│   ├── blessing-aws-credentials.gguf          # Access control layer
│   ├── blessing-kubectl-apply.gguf            # Kubernetes operations layer
│   └── base-model-llama2-7b.gguf              # (optional) base model cached
│
├── blessing-terraform-apply.toml              # Metadata + Trifecta
├── blessing-aws-credentials.toml
├── blessing-kubectl-apply.toml
│
├── knowledge_base.json                        # Semantic index for layer discovery
└── composition-plans/                         # Cached composition histories
    ├── exec_2026_04_01_102345.json
    └── exec_2026_04_01_102600.json
```

**Artifact Path Conventions:**
- Layer artifacts stored in `{knowledge_index_dir}/layers/blessing-{blessing_id}.gguf`
- TOML metadata mirrors the layer ID: `blessing-{blessing_id}.toml`
- No path fragments: always use absolute or `~`-expanded paths
- Directory must be writable for layer caching and composition checkpoints

---

## CompositionPlan Structure (JSON)

When an agent requests a blessing, the system generates a composition plan defining the model configuration:

```json
{
  "base_model_id": "meta-llama/Llama-2-7b",
  "layers": [
    {
      "blessing_id": "blessing-terraform-apply",
      "artifact_path": "/home/user/.cache/blessings/layers/blessing-terraform-apply.gguf",
      "embedding_dim": 768,
      "adapter_rank": 16,
      "generated_at": "2026-04-01T10:00:00Z",
      "quality_score": 0.92
    },
    {
      "blessing_id": "blessing-aws-credentials",
      "artifact_path": "/home/user/.cache/blessings/layers/blessing-aws-credentials.gguf",
      "embedding_dim": 768,
      "adapter_rank": 8,
      "generated_at": "2026-04-01T09:15:00Z",
      "quality_score": 0.88
    }
  ],
  "total_adapter_params": 114688,
  "estimated_tokens_per_inference": 2456
}
```

**Composition Plan Fields:**
- `base_model_id`: Hugging Face model identifier (e.g., "meta-llama/Llama-2-7b", "mistral-community/Mistral-7B-v0.1")
- `layers`: Array of `LayerMetadata` objects (ordered; first layer is primary blessing)
- `total_adapter_params`: Sum of all layer parameters (`embedding_dim × adapter_rank`)
- `estimated_tokens_per_inference`: Conservative estimate for cost tracking (base 2048 + per-layer overhead)

**Example Calculation:**
- Base model overhead: 2048 tokens
- Layer 1 (terraform): `768 × 16 / 1000 = 12` tokens
- Layer 2 (aws): `768 × 8 / 1000 = 6` tokens
- Total: `2048 + 12 + 6 = 2066` tokens

**Memory Budget Estimate:**
- 7B base model: ~14 GB
- Adapter memory per layer: `embedding_dim × adapter_rank × 4 bytes / (1024×1024)` MB
- Example: `768 × 16 × 4 / 1048576 ≈ 0.047 MB` per layer (negligible)

---

## Backend Support & Inference Pipeline

### Phase 7→9 Timeline: Backend Deprecation

| Backend | Status | Phase | Use Case | GPU Support |
|---------|--------|-------|----------|-------------|
| **Candle** | Primary | 7-9 | GPU-accelerated inference (CUDA/Metal) | Yes (CUDA 12.x) |
| **llama.cpp-rs** | Deprecated | 8→9 | CPU fallback (legacy wrapper) | No (CPU only) |
| **Ripgrep** | Fallback | 7-9 | Keyword-based BM25 (guaranteed) | No (search-based) |

**Candle Backend (Primary):**
- Native Rust ML framework by Meta
- Feature flag: `candle` in Cargo.toml
- GPU acceleration via CUDA (requires CUDA 12.x Toolkit)
- Loads GGUF artifacts directly using `candle_core::Tensor`
- Optimal for inference batching and long-running services

**llama.cpp-rs Backend (Deprecated Phase 8→9):**
- FFI wrapper around llama.cpp C library
- Feature flag: `llamacpp-fallback` (disabled by Phase 9)
- Pure CPU inference via LLaMA quantization
- Slower than Candle but more memory-efficient for embedded scenarios
- Phase 8: Warning on llama.cpp selection
- Phase 9: Removed; migrate to Ripgrep fallback or upgrade to GPU

**Ripgrep Fallback (Always Available):**
- Offline keyword-based retrieval using BM25 scoring
- No neural network inference (stateless)
- Guaranteed to work on any system (no dependencies)
- Returns keyword-ranked results from knowledge base
- Quality: 0.5-0.7 (acceptable for blocking operations)
- Used when Candle GPU unavailable and llama.cpp disabled

**Selection Algorithm:**

```
if config.prefer_candle:
    return Candle        # Try GPU first
elif config.enable_llamacpp:
    return LlamaCpp      # Fallback to CPU
else:
    return Ripgrep       # Final fallback (always works)
```

---

## Deprecation & Migration Guide

### Phase 8 (Current): llama.cpp-rs Deprecation Notice

When an agent selects llama.cpp backend:

```
⚠️  llama.cpp-rs backend will be removed in Phase 9 (Q3 2026).
    Migrate to Candle (GPU) or Ripgrep (keyword fallback).
    See: docs/blessing-system/migration-llama-to-candle.md
```

**Migration Checklist:**
1. If you have GPU: Enable Candle feature, install CUDA 12.x Toolkit
2. If CPU-only: Switch to Ripgrep fallback (no neural inference)
3. Update feature flags in `Cargo.toml`
4. Recompile and re-run test suite

### Phase 9 (Q3 2026): llama.cpp-rs Removal

- Feature flag `llamacpp-fallback` removed from Cargo.toml
- `InferenceBackendSelector::LlamaCpp` variant deleted
- Import statements referencing `blessing::inference::llamacpp` will fail
- All migrations must complete before Phase 9 release

---

## Rust Usage Example: Agent Requesting Blessing with Layers

```rust
use b00t_cli::blessing::{
    BlessingRequest, BlessingEvaluator, BlessingGraph,
    CompositionPlan, InferenceConfig, select_inference_backend
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load blessing graph from TOML
    let graph_toml = std::fs::read_to_string("~/.dotfiles/_b00t_/blessings.toml")?;
    let graph = BlessingGraph::from_toml(&graph_toml)?;

    // Create evaluator with policy rules
    let evaluator = BlessingEvaluator::new(graph, policy_rules);

    // Agent requests blessing
    let request = BlessingRequest {
        blessing_id: "blessing-terraform-apply".to_string(),
        agent_role: "orchestrator".to_string(),
        agent_blessings: vec!["blessing-aws-credentials".to_string()],
        available_budget: 5000,  // tokens
        executive_override: false,
    };

    // Prayer: evaluate and compose layers
    let prayer_result = evaluator.pray(&request).await?;

    if prayer_result.granted {
        // Blessing approved: composition plan available
        let composition = prayer_result.composition_plan.unwrap();
        println!("Base model: {}", composition.base_model_id);
        println!("Layers: {:?}", composition.layers.len());
        println!("Estimated tokens: {}", composition.estimated_tokens_per_inference());

        // Select inference backend
        let config = InferenceConfig {
            base_model_id: composition.base_model_id.clone(),
            knowledge_index_dir: "~/.cache/blessings".to_string(),
            prefer_candle: true,  // Try GPU first
            enable_llamacpp: false,  // Phase 8: discourage llama.cpp
        };

        let backend = select_inference_backend(&config);
        println!("Selected backend: {:?}", backend);

        // Load and compose layers
        for layer in &composition.layers {
            println!("Loading layer: {} (rank={})", layer.blessing_id, layer.adapter_rank);
            // Backend-specific layer loading here
        }
    } else {
        // Blessing denied
        eprintln!("Denied: {}", prayer_result.denial_reason.unwrap());
        eprintln!("Suggestions: {:?}", prayer_result.suggestions);
    }

    Ok(())
}
```

**Key Integration Points:**
1. `BlessingEvaluator::pray()` returns `BlessingPrayerResult` with optional `CompositionPlan`
2. `CompositionPlan` lists all layers to load, in order
3. Backend selector uses `InferenceConfig` to pick Candle/llama.cpp/Ripgrep
4. Layer metadata drives GPU memory allocation and timeout settings
5. Audit events emitted on approval/denial for Kaizen loop

---

## Composition Plan Validation

Before a composition plan is executed, validation gates ensure safety:

```rust
pub trait CompositionValidation: Send + Sync {
    /// Validate layer compatibility and resource constraints
    /// Checks:
    /// - All layers have matching embedding_dim
    /// - Total adapter params fit in memory budget
    /// - No circular layer dependencies
    /// - Quality scores above minimum threshold
    async fn validate(&self, plan: &CompositionPlan) -> Result<(), String>;

    /// Checkpoint plan to disk before orchestrator executes
    /// Persists composition plan to knowledge_index/composition-plans/{timestamp}.json
    /// Used for audit trail and rollback on failure
    async fn checkpoint(&self, blessing_id: &str, plan: &CompositionPlan) -> Result<(), String>;
}
```

**Validation Rules:**
- All layers must have `embedding_dim >= 256` (safety minimum)
- All layers must have matching `embedding_dim` (tensor compatibility)
- Total adapter params must be ≤ 100M (memory ceiling on standard 24GB GPU)
- Quality score must be ≥ 0.7 (minimum quality threshold)
- Layer generation timestamps must be within 30 days (freshness check)

---

## Summary Table: Blessing Layer Properties

| Property | Type | Example | Required |
|----------|------|---------|----------|
| `blessing_id` | String | `blessing-terraform-apply` | Yes |
| `artifact_path` | Path | `/cache/blessings/layers/...gguf` | Yes |
| `embedding_dim` | u32 | 768, 1024, 4096 | Yes |
| `adapter_rank` | u32 | 8, 16, 32 | Yes |
| `generated_at` | DateTime (UTC) | `2026-04-01T10:00:00Z` | Yes |
| `quality_score` | f32 (0.0-1.0) | 0.92 | Yes |

**See Also:**
- [Blessing Graph Architecture](architecture_blessing_orchestration.md)
- [Prayer Workflow: Blessing Requests](../architecture/k0mmand3r_interface.md)
- [Knowledge Base & RAG Integration](./knowledge-base-integration.md)
- [Phase 8→9 Migration: llama.cpp→Candle](./migration-llama-to-candle.md)
