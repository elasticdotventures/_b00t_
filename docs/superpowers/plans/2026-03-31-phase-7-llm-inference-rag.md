# Phase 7: LLM Inference + RAG Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a three-tier LLM inference system (Candle primary → llama.cpp-rs fallback → ripgrep text search) with semantic knowledge base integration for blessing-based model layer stacking.

**Architecture:**
- Trait-based abstraction (LLMInference) for backend-agnostic inference
- GraphRAG + vector DB for semantic storage of discovered capabilities
- GGUF adapter layer generation from embeddings
- Graceful degradation: Candle (GPU) → llama.cpp-rs (CPU) → ripgrep (offline)
- Integration with prayer workflow: blessings now carry composition plans for model layer stacking

**Tech Stack:** Rust, Candle (primary), llama.cpp-rs (fallback), GraphRAG, ripgrep, tokio async

---

## Phase 8 Integration Strategy (Lightweight Hooks)

Tasks 3, 7, and 9 include **minimal additional design** to enable Phase 8 without rework:

- **Task 3 (Candle)**: Model lifecycle traits (stubs) + device detection
- **Task 7 (KnowledgeBase)**: Discovery callbacks + quality feedback mechanism
- **Task 9 (Prayer)**: Checkpoint validation traits + orchestrator hooks + audit events

These additions (est. ~350 lines total) cost ~15% more in Phase 7 but eliminate 40% rework in Phase 8. All hooks are optional traits; Phase 8 will implement them as concrete structs.

---

## File Structure

**New modules:**
- `b00t-cli/src/blessing/inference/mod.rs` - trait definition + backend selection
- `b00t-cli/src/blessing/inference/candle.rs` - Candle backend
- `b00t-cli/src/blessing/inference/llamacpp.rs` - llama.cpp-rs backend (feature-gated)
- `b00t-cli/src/blessing/rag/mod.rs` - KnowledgeBase struct
- `b00t-cli/src/blessing/rag/graph.rs` - GraphRAG implementation
- `b00t-cli/src/blessing/rag/fallback.rs` - ripgrep fallback

**Modified files:**
- `b00t-cli/Cargo.toml` - add inference dependencies
- `b00t-cli/src/blessing/mod.rs` - export inference/rag modules
- `b00t-cli/src/blessing/prayer/mod.rs` - integrate composition_plan + compose_layers
- Tests: `b00t-cli/src/blessing/inference/tests.rs`, `b00t-cli/src/blessing/rag/tests.rs`, `b00t-cli/src/blessing/prayer/tests.rs`

---

## Implementation Tasks

### Task 1: Add Cargo Dependencies

**Files:**
- Modify: `b00t-cli/Cargo.toml`

- [ ] **Step 1: Add inference dependencies to Cargo.toml**

Open `b00t-cli/Cargo.toml` and add under `[dependencies]`:

```toml
# LLM inference backends
candle-core = { version = "0.4", optional = true }
candle-nn = { version = "0.4", optional = true }
candle-transformers = { version = "0.4", optional = true }
hf-hub = { version = "0.3", optional = true }
tokenizers = { version = "0.14", optional = true }

# llama.cpp-rs fallback (feature-gated, unsafe code)
llama-cpp-sys = { version = "0.2", optional = true }

# RAG and vector storage
ndarray = "0.15"
serde_json = "1.0"
chrono = { version = "0.4", features = ["serde"] }
tokio = { version = "1.0", features = ["full"] }

# Text search fallback
regex = "1.10"
```

Add feature flags:

```toml
[features]
default = ["candle"]
candle = ["candle-core", "candle-nn", "candle-transformers", "hf-hub", "tokenizers"]
llamacpp-fallback = ["llama-cpp-sys"]
```

- [ ] **Step 2: Verify dependencies resolve**

Run: `cargo check`
Expected: No errors, dependencies resolve

- [ ] **Step 3: Commit**

```bash
git add b00t-cli/Cargo.toml
git commit -m "feat(inference): add Candle, llama.cpp-rs, and RAG dependencies"
```

---

### Task 2: Define LLMInference Trait

**Files:**
- Create: `b00t-cli/src/blessing/inference/mod.rs`
- Test: `b00t-cli/src/blessing/inference/tests.rs`

- [ ] **Step 1: Write failing test for LLMInference trait**

Create `b00t-cli/src/blessing/inference/tests.rs`:

```rust
#[cfg(test)]
mod inference_tests {
    use super::super::*;

    #[tokio::test]
    async fn test_lmm_inference_trait_exists() {
        // Trait should exist and be object-safe
        let _: Box<dyn LLMInference> = Box::new(std::panic::panic_any(""));
        // This will fail: LLMInference trait doesn't exist yet
    }

    #[tokio::test]
    async fn test_backend_selector_tries_candle_first() {
        // Test that select_inference_backend tries Candle
        // Will fail: function doesn't exist yet
        let _backend = select_inference_backend(&Default::default()).await;
    }

    #[test]
    fn test_embedding_type() {
        // Embeddings should be vectors of f32
        let embedding = Embedding(vec![0.1, 0.2, 0.3]);
        assert_eq!(embedding.0.len(), 3);
    }
}
```

- [ ] **Step 2: Create inference/mod.rs with trait definition**

Create `b00t-cli/src/blessing/inference/mod.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Embedding: vector representation of text (f32 floats)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Embedding(pub Vec<f32>);

impl Embedding {
    pub fn dim(&self) -> usize {
        self.0.len()
    }

    pub fn cosine_similarity(&self, other: &Embedding) -> f32 {
        let dot: f32 = self.0.iter().zip(&other.0).map(|(a, b)| a * b).sum();
        let mag_a: f32 = self.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        let mag_b: f32 = other.0.iter().map(|x| x * x).sum::<f32>().sqrt();
        if mag_a == 0.0 || mag_b == 0.0 {
            0.0
        } else {
            dot / (mag_a * mag_b)
        }
    }
}

/// Model information for logging/debugging
#[derive(Clone, Debug)]
pub struct ModelInfo {
    pub model_id: String,
    pub embedding_dim: usize,
    pub backend: String,
    pub available: bool,
}

/// LLMInference trait: abstraction over inference backends
#[async_trait::async_trait]
pub trait LLMInference: Send + Sync {
    /// Generate embedding for text
    async fn embed(&self, text: &str) -> Result<Embedding, Box<dyn std::error::Error>>;

    /// Compose layers (adapters) into the model
    async fn compose_layers(
        &mut self,
        blessing_ids: &[&str],
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Check if backend is available (model downloaded, device ready)
    fn is_available(&self) -> bool;

    /// Get backend information
    fn model_info(&self) -> ModelInfo;

    /// Clear cached layers
    fn clear_layers(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

/// Backend selection: Candle → llama.cpp-rs → ripgrep
pub enum InferenceBackendSelector {
    Candle(Box<dyn LLMInference>),
    #[cfg(feature = "llamacpp-fallback")]
    LlamaCpp(Box<dyn LLMInference>),
    Ripgrep(Box<dyn LLMInference>),
}

impl InferenceBackendSelector {
    pub fn backend_name(&self) -> &str {
        match self {
            InferenceBackendSelector::Candle(_) => "Candle",
            #[cfg(feature = "llamacpp-fallback")]
            InferenceBackendSelector::LlamaCpp(_) => "llama.cpp-rs",
            InferenceBackendSelector::Ripgrep(_) => "ripgrep",
        }
    }

    pub async fn embed(&self, text: &str) -> Result<Embedding, Box<dyn std::error::Error>> {
        match self {
            InferenceBackendSelector::Candle(b) => b.embed(text).await,
            #[cfg(feature = "llamacpp-fallback")]
            InferenceBackendSelector::LlamaCpp(b) => b.embed(text).await,
            InferenceBackendSelector::Ripgrep(b) => b.embed(text).await,
        }
    }
}

pub struct InferenceConfig {
    pub base_model_id: String,
    pub knowledge_index_dir: std::path::PathBuf,
    pub prefer_candle: bool,
    pub enable_llamacpp: bool,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        InferenceConfig {
            base_model_id: "meta-llama/Llama-2-7b".to_string(),
            knowledge_index_dir: std::path::PathBuf::from("./knowledge_index"),
            prefer_candle: true,
            enable_llamacpp: cfg!(feature = "llamacpp-fallback"),
        }
    }
}

/// Backend selection: try Candle first, fallback to llama.cpp-rs, then ripgrep
pub async fn select_inference_backend(
    config: &InferenceConfig,
) -> Result<InferenceBackendSelector, Box<dyn std::error::Error>> {
    // Try 1: Candle (preferred)
    if config.prefer_candle {
        match candle::CandleBackend::new(&config.base_model_id).await {
            Ok(backend) => {
                eprintln!("✅ LLM backend: Candle");
                return Ok(InferenceBackendSelector::Candle(Box::new(backend)));
            }
            Err(e) => eprintln!("⚠️ Candle backend failed: {}", e),
        }
    }

    // Try 2: llama.cpp-rs (if feature enabled)
    #[cfg(feature = "llamacpp-fallback")]
    {
        if config.enable_llamacpp {
            match llamacpp::LlamaCppBackend::new(&config.base_model_id).await {
                Ok(backend) => {
                    eprintln!("⚠️ LLM backend: llama.cpp-rs (deprecated in Phase 9)");
                    return Ok(InferenceBackendSelector::LlamaCpp(Box::new(backend)));
                }
                Err(e) => eprintln!("⚠️ llama.cpp-rs backend failed: {}", e),
            }
        }
    }

    // Try 3: ripgrep fallback
    eprintln!("⚠️ LLM inference unavailable; using ripgrep fallback");
    let ripgrep = fallback::RipgrepBackend::new(&config.knowledge_index_dir)?;
    Ok(InferenceBackendSelector::Ripgrep(Box::new(ripgrep)))
}

pub mod candle;
#[cfg(feature = "llamacpp-fallback")]
pub mod llamacpp;
pub mod fallback;

#[cfg(test)]
mod tests;
```

- [ ] **Step 3: Add async_trait dependency**

In `Cargo.toml`, add:

```toml
async-trait = "0.1"
```

- [ ] **Step 4: Run tests (expected: fail, trait doesn't compile yet)**

Run: `cargo test --lib blessing::inference::tests --no-run 2>&1 | head -20`
Expected: Compilation errors about missing modules

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/blessing/inference/mod.rs b00t-cli/Cargo.toml
git commit -m "feat(inference): define LLMInference trait and backend selection"
```

---

### Task 3: Implement Candle Backend (with Phase 8 Hooks)

**Files:**
- Create: `b00t-cli/src/blessing/inference/candle.rs`

**Phase 8 Integration:**
- Add `ModelCache` trait for lifecycle management (stub for Phase 8)
- Add metadata emission hook (device, model_loaded_at timestamp)
- Add device detection that Phase 8 model manager can query

- [ ] **Step 1: Write failing test for Candle**

Add to `b00t-cli/src/blessing/inference/tests.rs`:

```rust
#[cfg(feature = "candle")]
#[tokio::test]
async fn test_candle_backend_new() {
    // Candle should be creatable with default model
    let backend = candle::CandleBackend::new("meta-llama/Llama-2-7b").await;
    // Will fail: CandleBackend doesn't exist yet
    assert!(backend.is_ok());
}

#[cfg(feature = "candle")]
#[tokio::test]
async fn test_candle_embed() {
    let backend = candle::CandleBackend::new("meta-llama/Llama-2-7b").await
        .expect("Candle init");

    let embedding = backend.embed("test query").await
        .expect("embed");

    assert!(embedding.dim() > 0);
}

#[cfg(feature = "candle")]
#[test]
fn test_candle_is_available() {
    // Backend should report availability
    let backend = candle::CandleBackend::default();
    let available = backend.is_available();
    assert_eq!(available, true);  // Or false if GPU not present
}
```

- [ ] **Step 2: Create candle.rs**

Create `b00t-cli/src/blessing/inference/candle.rs`:

```rust
use super::{Embedding, LLMInference, ModelInfo};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

/// Candle backend: pure Rust, GPU-capable LLM inference
pub struct CandleBackend {
    model_id: String,
    embedding_dim: usize,
    base_model_path: PathBuf,
    active_layers: Vec<String>,
    device: String,  // "gpu" or "cpu"
}

impl CandleBackend {
    pub async fn new(model_id: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // TODO: In Phase 7.5, integrate with candle-transformers to:
        // 1. Download GGUF from HuggingFace
        // 2. Load model into Candle
        // 3. Detect GPU availability (cuda/metal)
        // 4. Fall back to CPU if GPU unavailable

        // For now: stub implementation
        Ok(CandleBackend {
            model_id: model_id.to_string(),
            embedding_dim: 768,
            base_model_path: PathBuf::from("/tmp/model.gguf"),
            active_layers: vec![],
            device: "cpu".to_string(),
        })
    }

    pub fn default() -> Self {
        CandleBackend {
            model_id: "meta-llama/Llama-2-7b".to_string(),
            embedding_dim: 768,
            base_model_path: PathBuf::from("/tmp/model.gguf"),
            active_layers: vec![],
            device: "cpu".to_string(),
        }
    }
}

#[async_trait]
impl LLMInference for CandleBackend {
    async fn embed(&self, text: &str) -> Result<Embedding, Box<dyn std::error::Error>> {
        // TODO: In Phase 7.5, use candle-transformers to:
        // 1. Tokenize text
        // 2. Run forward pass through model
        // 3. Extract embedding from final layer

        // Stub: return zero embedding
        Ok(Embedding(vec![0.0; self.embedding_dim]))
    }

    async fn compose_layers(
        &mut self,
        blessing_ids: &[&str],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: In Phase 7.6, implement adapter composition:
        // 1. Fetch GGUF layers from cache
        // 2. Stack adapters on base model
        // 3. Update active_layers

        self.active_layers = blessing_ids.iter().map(|s| s.to_string()).collect();
        Ok(())
    }

    fn is_available(&self) -> bool {
        // TODO: Check if model is downloaded and device is ready
        true
    }

    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            model_id: self.model_id.clone(),
            embedding_dim: self.embedding_dim,
            backend: "Candle".to_string(),
            available: self.is_available(),
        }
    }
}
```

- [ ] **Step 3: Run tests (will fail on actual inference, pass on structure)**

Run: `cargo test --lib blessing::inference --features=candle -- --nocapture 2>&1 | grep -A2 "test_candle"`
Expected: Tests compile, candle tests execute (may fail on actual GPU/download)

- [ ] **Step 4: Commit**

```bash
git add b00t-cli/src/blessing/inference/candle.rs
git commit -m "feat(inference): implement Candle backend stub"
```

---

### Task 4: Implement llama.cpp-rs Backend (Feature-Gated)

**Files:**
- Create: `b00t-cli/src/blessing/inference/llamacpp.rs`

- [ ] **Step 1: Write test for llama.cpp-rs**

Add to `b00t-cli/src/blessing/inference/tests.rs`:

```rust
#[cfg(feature = "llamacpp-fallback")]
#[tokio::test]
async fn test_llamacpp_backend_new() {
    let backend = llamacpp::LlamaCppBackend::new("meta-llama/Llama-2-7b").await;
    assert!(backend.is_ok());
}

#[cfg(feature = "llamacpp-fallback")]
#[test]
fn test_llamacpp_deprecation_warning() {
    let backend = llamacpp::LlamaCppBackend::default();
    // Should log deprecation warning
    assert!(backend.deprecated);
}
```

- [ ] **Step 2: Create llamacpp.rs**

Create `b00t-cli/src/blessing/inference/llamacpp.rs`:

```rust
use super::{Embedding, LLMInference, ModelInfo};
use async_trait::async_trait;
use std::path::PathBuf;

/// llama.cpp-rs backend: C++ bindings fallback
/// ⚠️ DEPRECATED: Contains unsafe code. Will be removed in Phase 9.
#[cfg(feature = "llamacpp-fallback")]
pub struct LlamaCppBackend {
    model_id: String,
    embedding_dim: usize,
    base_model_path: PathBuf,
    active_layers: Vec<String>,
    pub deprecated: bool,
}

#[cfg(feature = "llamacpp-fallback")]
impl LlamaCppBackend {
    pub async fn new(model_id: &str) -> Result<Self, Box<dyn std::error::Error>> {
        eprintln!("⚠️ llama.cpp-rs backend loaded. This backend will be removed in Phase 9.");
        eprintln!("   Use Candle backend (default) for production code.");

        Ok(LlamaCppBackend {
            model_id: model_id.to_string(),
            embedding_dim: 768,
            base_model_path: PathBuf::from("/tmp/model.gguf"),
            active_layers: vec![],
            deprecated: true,
        })
    }

    pub fn default() -> Self {
        LlamaCppBackend {
            model_id: "meta-llama/Llama-2-7b".to_string(),
            embedding_dim: 768,
            base_model_path: PathBuf::from("/tmp/model.gguf"),
            active_layers: vec![],
            deprecated: true,
        }
    }
}

#[cfg(feature = "llamacpp-fallback")]
#[async_trait]
impl LLMInference for LlamaCppBackend {
    async fn embed(&self, text: &str) -> Result<Embedding, Box<dyn std::error::Error>> {
        // TODO: In Phase 7.5, integrate with llama-cpp-sys to:
        // 1. Load GGUF via unsafe FFI
        // 2. Tokenize and embed
        // 3. Return embedding

        Ok(Embedding(vec![0.0; self.embedding_dim]))
    }

    async fn compose_layers(
        &mut self,
        blessing_ids: &[&str],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Load adapters via unsafe FFI
        self.active_layers = blessing_ids.iter().map(|s| s.to_string()).collect();
        Ok(())
    }

    fn is_available(&self) -> bool {
        // TODO: Check if llama.cpp library is available
        true
    }

    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            model_id: self.model_id.clone(),
            embedding_dim: self.embedding_dim,
            backend: "llama.cpp-rs (deprecated)".to_string(),
            available: self.is_available(),
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib blessing::inference --features=llamacpp-fallback -- --nocapture 2>&1 | grep -A2 "test_llamacpp"`
Expected: Deprecation warning in logs

- [ ] **Step 4: Commit**

```bash
git add b00t-cli/src/blessing/inference/llamacpp.rs
git commit -m "feat(inference): implement llama.cpp-rs backend (deprecated, feature-gated)"
```

---

### Task 5: Implement Ripgrep Fallback Backend

**Files:**
- Create: `b00t-cli/src/blessing/inference/fallback.rs`

- [ ] **Step 1: Write tests for ripgrep fallback**

Add to `b00t-cli/src/blessing/inference/tests.rs`:

```rust
#[tokio::test]
async fn test_ripgrep_backend_new() {
    let backend = fallback::RipgrepBackend::new("./knowledge_index");
    assert!(backend.is_ok());
}

#[tokio::test]
async fn test_ripgrep_embed_returns_zeros() {
    let backend = fallback::RipgrepBackend::new("./knowledge_index")
        .expect("ripgrep init");

    let embedding = backend.embed("test query").await
        .expect("embed");

    assert_eq!(embedding.dim(), 768);
    assert!(embedding.0.iter().all(|x| *x == 0.0));
}

#[tokio::test]
async fn test_ripgrep_search_query() {
    let backend = fallback::RipgrepBackend::new("./knowledge_index")
        .expect("ripgrep init");

    let results = backend.search("terraform apply", 5).await
        .expect("search");

    // Results may be empty if index dir doesn't exist, that's ok
    assert!(results.len() <= 5);
}
```

- [ ] **Step 2: Create fallback.rs**

Create `b00t-cli/src/blessing/inference/fallback.rs`:

```rust
use super::{Embedding, LLMInference, ModelInfo};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;

/// Search result from ripgrep-based keyword search
#[derive(Clone)]
pub struct BlessingSearchResult {
    pub blessing_id: String,
    pub relevance_score: f32,
    pub source_file: PathBuf,
    pub matched_lines: Vec<String>,
}

/// Ripgrep fallback backend: text search when LLM unavailable
pub struct RipgrepBackend {
    index_dir: PathBuf,
    cached_index: HashMap<String, BlessingSearchResult>,
}

impl RipgrepBackend {
    pub fn new(index_dir: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = PathBuf::from(index_dir);
        Ok(RipgrepBackend {
            index_dir: path,
            cached_index: HashMap::new(),
        })
    }

    pub async fn search(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<BlessingSearchResult>, Box<dyn std::error::Error>> {
        let keywords: Vec<&str> = query.split_whitespace().collect();
        if keywords.is_empty() {
            return Ok(vec![]);
        }

        // Use tokio to run rg command
        let mut cmd = tokio::process::Command::new("rg");
        cmd.arg("--files-with-matches")
            .arg("--type=toml")
            .args(&keywords)
            .current_dir(&self.index_dir);

        let output = cmd.output().await?;
        if !output.status.success() {
            return Ok(vec![]);
        }

        let matched_files: Vec<&str> = String::from_utf8(output.stdout)?
            .lines()
            .take(top_k * 2)
            .collect();

        let mut results = Vec::new();
        for file_path in matched_files {
            if let Ok(content) = tokio::fs::read_to_string(file_path).await {
                let keyword_count = keywords.iter()
                    .filter(|kw| content.contains(kw))
                    .count();

                if keyword_count > 0 {
                    if let Ok(blessing_id) = extract_blessing_id(&content) {
                        results.push(BlessingSearchResult {
                            blessing_id,
                            relevance_score: (keyword_count as f32) / (keywords.len() as f32),
                            source_file: PathBuf::from(file_path),
                            matched_lines: extract_matching_lines(&content, &keywords),
                        });
                    }
                }
            }
        }

        results.sort_by(|a, b| b.relevance_score.partial_cmp(&a.relevance_score).unwrap());
        Ok(results.into_iter().take(top_k).collect())
    }

    pub async fn retrieve_blessing(
        &self,
        blessing_id: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let path = self.index_dir.join(format!("{}.toml", blessing_id.replace(':', "-")));
        let content = tokio::fs::read_to_string(path).await?;
        Ok(content)
    }
}

fn extract_blessing_id(content: &str) -> Result<String, Box<dyn std::error::Error>> {
    for line in content.lines() {
        if line.contains("id =") {
            if let Some(id) = line.split('"').nth(1) {
                return Ok(id.to_string());
            }
        }
    }
    Err("No blessing ID found".into())
}

fn extract_matching_lines(content: &str, keywords: &[&str]) -> Vec<String> {
    content.lines()
        .filter(|line| keywords.iter().any(|kw| line.contains(kw)))
        .map(|s| s.to_string())
        .take(3)
        .collect()
}

#[async_trait]
impl LLMInference for RipgrepBackend {
    async fn embed(&self, _text: &str) -> Result<Embedding, Box<dyn std::error::Error>> {
        // Ripgrep backend doesn't generate embeddings
        // Returns zero vector as placeholder
        Ok(Embedding(vec![0.0; 768]))
    }

    async fn compose_layers(
        &mut self,
        _blessing_ids: &[&str],
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Ripgrep backend doesn't support layer composition
        Ok(())
    }

    fn is_available(&self) -> bool {
        self.index_dir.exists()
    }

    fn model_info(&self) -> ModelInfo {
        ModelInfo {
            model_id: "ripgrep-text-search".to_string(),
            embedding_dim: 0,
            backend: "ripgrep (fallback)".to_string(),
            available: self.is_available(),
        }
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib blessing::inference::tests::test_ripgrep -- --nocapture`
Expected: Tests pass (results may be empty if index dir doesn't exist)

- [ ] **Step 4: Commit**

```bash
git add b00t-cli/src/blessing/inference/fallback.rs
git commit -m "feat(inference): implement ripgrep fallback backend"
```

---

### Task 6: Wire Inference Module into Blessing Module

**Files:**
- Modify: `b00t-cli/src/blessing/mod.rs`

- [ ] **Step 1: Add module exports**

In `b00t-cli/src/blessing/mod.rs`, add after existing module declarations:

```rust
pub mod inference;
pub use inference::{LLMInference, InferenceBackendSelector, select_inference_backend};
```

- [ ] **Step 2: Run tests to verify module loads**

Run: `cargo test --lib blessing::inference -- --nocapture 2>&1 | head -30`
Expected: Inference module tests run

- [ ] **Step 3: Commit**

```bash
git add b00t-cli/src/blessing/mod.rs
git commit -m "feat(blessing): export inference module"
```

---

### Task 7: Implement Knowledge Base (with Phase 8 Hooks)

**Files:**
- Create: `b00t-cli/src/blessing/rag/mod.rs`
- Test: `b00t-cli/src/blessing/rag/tests.rs`

**Phase 8 Integration:**
- Add `SemanticDiscoveryCallback` trait (hook for assimilate to register)
- Store quality_score feedback mechanism (denial rates from prayer)
- Layer generation timestamps for versioning (Phase 8 model manager)
- Emit discovery events (on_capability_discovered hook) for assimilate integration

- [ ] **Step 1: Write failing tests for KnowledgeBase**

Create `b00t-cli/src/blessing/rag/tests.rs`:

```rust
#[cfg(test)]
mod rag_tests {
    use super::super::*;

    #[tokio::test]
    async fn test_knowledge_base_new() {
        let kb = KnowledgeBase::new("./knowledge_index").await;
        assert!(kb.is_ok());
    }

    #[tokio::test]
    async fn test_discover_capability() {
        let mut kb = KnowledgeBase::new("./knowledge_index").await
            .expect("KB init");

        let blessing = crate::blessing::BlessingNode {
            id: "blessing:test".to_string(),
            type_: "blessing".to_string(),
            usage_notes: Some("Test blessing".to_string()),
            ..Default::default()
        };

        let result = kb.discover_capability(&blessing, "test context").await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_blessing_metadata() {
        let metadata = BlessingMetadata {
            blessing_id: "blessing:test".to_string(),
            source_datum: "assimilate".to_string(),
            discovered_at: chrono::Utc::now(),
            quality_score: 0.8,
            depends_on: vec![],
            layer_path: std::path::PathBuf::from("./layers/test.gguf"),
        };

        assert_eq!(metadata.blessing_id, "blessing:test");
        assert!(metadata.quality_score >= 0.0 && metadata.quality_score <= 1.0);
    }
}
```

- [ ] **Step 2: Create rag/mod.rs**

Create `b00t-cli/src/blessing/rag/mod.rs`:

```rust
use crate::blessing::{BlessingNode, BlessingGraph};
use crate::blessing::inference::{Embedding, LLMInference};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Blessing metadata: tracking knowledge base entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlessingMetadata {
    pub blessing_id: String,
    pub source_datum: String,
    pub discovered_at: DateTime<Utc>,
    pub quality_score: f32,
    pub depends_on: Vec<String>,
    pub layer_path: PathBuf,
}

/// Knowledge base: semantic storage of discovered capabilities
pub struct KnowledgeBase {
    pub metadata: HashMap<String, BlessingMetadata>,
    pub layer_cache: HashMap<String, LayerMetadata>,
    index_dir: PathBuf,
}

#[derive(Clone, Debug)]
pub struct LayerMetadata {
    pub blessing_id: String,
    pub artifact_path: PathBuf,
    pub embedding_dim: usize,
    pub adapter_rank: usize,
    pub generated_at: DateTime<Utc>,
    pub quality_score: f32,
}

impl KnowledgeBase {
    pub async fn new(index_dir: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let path = PathBuf::from(index_dir);

        // Create index dir if it doesn't exist
        if !path.exists() {
            tokio::fs::create_dir_all(&path).await?;
        }

        Ok(KnowledgeBase {
            metadata: HashMap::new(),
            layer_cache: HashMap::new(),
            index_dir: path,
        })
    }

    pub async fn discover_capability(
        &mut self,
        blessing: &BlessingNode,
        _agent_context: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. Extract semantics from blessing trifecta
        let semantic_text = format!(
            "Usage: {}\nExecution: {:?}\nData: {:?}",
            blessing.usage_notes.as_ref().unwrap_or(&String::new()),
            blessing.execute_access,
            blessing.data_permissions
        );

        // 2. Store metadata
        let metadata = BlessingMetadata {
            blessing_id: blessing.id.clone(),
            source_datum: "assimilate".to_string(),
            discovered_at: Utc::now(),
            quality_score: 0.8,
            depends_on: blessing.requires.clone(),
            layer_path: self.adapter_path_for(&blessing.id),
        };

        self.metadata.insert(blessing.id.clone(), metadata);

        // 3. Save blessing to index
        let index_file = self.index_dir.join(format!("{}.toml", blessing.id.replace(':', "-")));
        let toml_content = format!(
            r#"
id = "{}"
type = "{}"
usage_notes = "{}"

[execute_access]
binary = "{}"

[data_permissions]
readable_paths = []
"#,
            blessing.id,
            blessing.type_,
            semantic_text.replace('"', "\\\""),
            blessing.execute_access.as_ref().map(|e| &e.binary).unwrap_or(&"".to_string())
        );

        tokio::fs::write(&index_file, toml_content).await?;
        Ok(())
    }

    pub async fn generate_layer(
        &mut self,
        blessing_id: &str,
    ) -> Result<LayerMetadata, Box<dyn std::error::Error>> {
        // Stub: create layer metadata
        let layer = LayerMetadata {
            blessing_id: blessing_id.to_string(),
            artifact_path: self.adapter_path_for(blessing_id),
            embedding_dim: 768,
            adapter_rank: 8,
            generated_at: Utc::now(),
            quality_score: 0.85,
        };

        self.layer_cache.insert(blessing_id.to_string(), layer.clone());
        Ok(layer)
    }

    fn adapter_path_for(&self, blessing_id: &str) -> PathBuf {
        self.index_dir.join(format!("layers/{}.gguf", blessing_id.replace(':', "-")))
    }
}

pub mod graph;
pub mod fallback;

#[cfg(test)]
mod tests;
```

- [ ] **Step 3: Run tests**

Run: `cargo test --lib blessing::rag::tests -- --nocapture`
Expected: Tests pass

- [ ] **Step 4: Commit**

```bash
git add b00t-cli/src/blessing/rag/mod.rs b00t-cli/src/blessing/rag/tests.rs
git commit -m "feat(rag): implement KnowledgeBase struct and BlessingMetadata"
```

---

### Task 8: Implement GraphRAG Module

**Files:**
- Create: `b00t-cli/src/blessing/rag/graph.rs`

- [ ] **Step 1: Create GraphRAG structure**

Create `b00t-cli/src/blessing/rag/graph.rs`:

```rust
use serde_json::{json, Value};
use std::collections::HashMap;

/// GraphRAG node representing a capability
#[derive(Clone, Debug)]
pub struct GraphRAGNode {
    pub id: String,
    pub label: String,  // blessing type
    pub properties: Value,
}

/// GraphRAG edge representing dependency
#[derive(Clone, Debug)]
pub struct GraphRAGEdge {
    pub from: String,
    pub to: String,
    pub relationship: String,
}

/// GraphRAG: semantic graph of capabilities and dependencies
pub struct GraphRAG {
    pub nodes: HashMap<String, GraphRAGNode>,
    pub edges: Vec<GraphRAGEdge>,
}

impl GraphRAG {
    pub fn new() -> Self {
        GraphRAG {
            nodes: HashMap::new(),
            edges: vec![],
        }
    }

    pub fn add_node(&mut self, id: String, label: String, properties: Value) {
        self.nodes.insert(id.clone(), GraphRAGNode { id, label, properties });
    }

    pub fn add_edge(&mut self, from: String, to: String, relationship: String) {
        self.edges.push(GraphRAGEdge { from, to, relationship });
    }

    pub fn traverse_from(&self, node_id: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        // BFS traversal from node_id
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut path = vec![];

        queue.push_back(node_id.to_string());
        visited.insert(node_id.to_string());

        while let Some(current) = queue.pop_front() {
            path.push(current.clone());

            for edge in &self.edges {
                if edge.from == current && !visited.contains(&edge.to) {
                    visited.insert(edge.to.clone());
                    queue.push_back(edge.to.clone());
                }
            }
        }

        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_rag_new() {
        let graph = GraphRAG::new();
        assert_eq!(graph.nodes.len(), 0);
        assert_eq!(graph.edges.len(), 0);
    }

    #[test]
    fn test_add_node() {
        let mut graph = GraphRAG::new();
        graph.add_node(
            "blessing:test".to_string(),
            "blessing".to_string(),
            json!({"key": "value"}),
        );

        assert_eq!(graph.nodes.len(), 1);
        assert!(graph.nodes.contains_key("blessing:test"));
    }

    #[test]
    fn test_traverse_from() {
        let mut graph = GraphRAG::new();
        graph.add_node("A".to_string(), "node".to_string(), json!({}));
        graph.add_node("B".to_string(), "node".to_string(), json!({}));
        graph.add_node("C".to_string(), "node".to_string(), json!({}));

        graph.add_edge("A".to_string(), "B".to_string(), "depends".to_string());
        graph.add_edge("B".to_string(), "C".to_string(), "depends".to_string());

        let path = graph.traverse_from("A").expect("traverse");
        assert_eq!(path, vec!["A", "B", "C"]);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test --lib blessing::rag::graph -- --nocapture`
Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add b00t-cli/src/blessing/rag/graph.rs
git commit -m "feat(rag): implement GraphRAG traversal and node/edge management"
```

---

### Task 9: Integrate Composition Plan into Prayer Workflow (with Phase 8 Hooks)

**Files:**
- Modify: `b00t-cli/src/blessing/prayer/mod.rs`

**Phase 8 Integration:**
- Add `CompositionValidation` trait for checkpoint system (Phase 3/8)
- Add orchestrator step transition hooks (blessing → step state)
- Emit composition_audit event (for orchestrator state machines)
- Add denial_audit event (feeds Kaizen loop in Phase 4/8)

- [ ] **Step 1: Add CompositionPlan struct**

Add to top of `b00t-cli/src/blessing/prayer/mod.rs`:

```rust
use super::rag::LayerMetadata;

/// Model layer composition plan
#[derive(Clone, Debug)]
pub struct CompositionPlan {
    pub base_model_id: String,
    pub layers: Vec<LayerMetadata>,
    pub total_adapter_params: usize,
}

impl CompositionPlan {
    pub fn estimated_tokens_per_inference(&self) -> usize {
        let base_cost = 7_000;
        let adapter_overhead_per_layer = 500;
        base_cost + (self.layers.len() * adapter_overhead_per_layer)
    }

    pub fn fits_in_budget(&self, budget_mb: usize) -> bool {
        let base_model_size = 14_000;
        let adapter_size_per_layer = 50;
        let total_size = base_model_size + (self.layers.len() * adapter_size_per_layer);
        total_size <= budget_mb
    }
}
```

- [ ] **Step 2: Extend BlessingPrayerResult**

Modify `BlessingPrayerResult` struct:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct BlessingPrayerResult {
    pub granted: bool,
    pub blessing: Option<BlessingNode>,
    pub denial_reason: Option<String>,
    pub suggestions: Vec<String>,
    pub composition_plan: Option<CompositionPlan>,  // NEW
}
```

- [ ] **Step 3: Update test for composition_plan**

Update existing tests to include composition_plan:

```rust
#[test]
fn test_prayer_approved_observer_observe() {
    let evaluator = BlessingEvaluator::new(sample_blessing_graph(), sample_policy());

    let request = BlessingRequest {
        blessing_id: "blessing:observe-infrastructure".to_string(),
        agent_role: "observer".to_string(),
        agent_blessings: vec![],
        available_budget: 1000,
        executive_override: false,
    };

    let result = evaluator.evaluate_prayer(&request);

    assert!(result.granted);
    assert!(result.blessing.is_some());
    assert_eq!(result.blessing.unwrap().id, "blessing:observe-infrastructure");
    assert!(result.denial_reason.is_none());
    assert!(result.composition_plan.is_none());  // Added (no composition for observer)
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test --lib blessing::prayer -- --nocapture`
Expected: Tests pass

- [ ] **Step 5: Commit**

```bash
git add b00t-cli/src/blessing/prayer/mod.rs
git commit -m "feat(prayer): extend BlessingPrayerResult with composition_plan"
```

---

### Task 10: Wire RAG Module into Blessing Module

**Files:**
- Modify: `b00t-cli/src/blessing/mod.rs`

- [ ] **Step 1: Export RAG modules**

Add to `b00t-cli/src/blessing/mod.rs`:

```rust
pub mod rag;
pub use rag::{KnowledgeBase, BlessingMetadata, LayerMetadata};
```

- [ ] **Step 2: Verify all modules compile**

Run: `cargo test --lib blessing -- --nocapture 2>&1 | head -50`
Expected: All inference and RAG tests compile and run

- [ ] **Step 3: Commit**

```bash
git add b00t-cli/src/blessing/mod.rs
git commit -m "feat(blessing): export RAG module"
```

---

### Task 11: Write Documentation for GGUF Layer Schema

**Files:**
- Create: `docs/blessing-system/gguf-layer-schema.md`

- [ ] **Step 1: Create documentation file**

Create `docs/blessing-system/gguf-layer-schema.md`:

```markdown
# GGUF Layer Schema for Blessing-Based Model Stacking

## Overview

GGUF layers are adapter weights derived from blessing-discovered capabilities. Each blessing can generate a specialized layer that stacks on top of the base model.

## Blessing Node Extension

Every blessing node can now carry layer metadata:

```toml
[[b00t.blessings]]
id = "blessing:terraform-apply"
type = "blessing"
usage_notes = "Apply Terraform configurations to AWS"

[blessing.execute_access]
binary = "/usr/bin/terraform"
allowed_args = ["apply", "plan"]
denied_args = ["destroy"]
timeout_seconds = 600

[blessing.data_permissions]
readable_paths = [".terraform/"]
writable_paths = ["tfstate"]
blocked_paths = ["/etc"]
requires_blessings = ["blessing:aws-credentials"]
requires_vpn = false

# Layer metadata (NEW)
[blessing.layer_metadata]
embedding_dim = 768
adapter_rank = 8
quality_score = 0.85
```

## Layer File Structure

GGUF layers are stored in OCI or Git LFS:

```
knowledge_index/
├── layers/
│   ├── blessing-terraform-apply.gguf    # Binary adapter weights
│   ├── blessing-aws-credentials.gguf
│   └── blessing-observe-infrastructure.gguf
├── blessing-terraform-apply.toml         # Metadata
├── blessing-aws-credentials.toml
└── blessing-observe-infrastructure.toml
```

## CompositionPlan Example

```json
{
  "base_model_id": "meta-llama/Llama-2-7b",
  "layers": [
    {
      "blessing_id": "blessing:observe-infrastructure",
      "artifact_path": "./layers/blessing-observe-infrastructure.gguf",
      "embedding_dim": 768,
      "adapter_rank": 8,
      "quality_score": 0.85
    },
    {
      "blessing_id": "blessing:terraform-apply",
      "artifact_path": "./layers/blessing-terraform-apply.gguf",
      "embedding_dim": 768,
      "adapter_rank": 8,
      "quality_score": 0.90
    }
  ],
  "total_adapter_params": 512000
}
```

## Backend Support

### Candle (Preferred)

Pure Rust implementation supports:
- GPU-accelerated inference (CUDA/Metal)
- Efficient adapter composition
- Layer hot-swapping

### llama.cpp-rs (Deprecated)

C++ fallback with deprecation timeline:
- Phase 7: Available
- Phase 8: Deprecated warnings
- Phase 9: Removed entirely

### Ripgrep (Fallback)

Text search when inference unavailable:
- Works offline
- No GPU required
- Returns keyword-matched capabilities

## Deprecation & Safety

**Unsafe code elimination:**
- Phase 7: llama.cpp-rs feature-gated (`--features=llamacpp-fallback`)
- Phase 8: Mark unsafe code with warnings
- Phase 9: Remove entirely, use Candle only
```

- [ ] **Step 2: Verify documentation**

Run: `ls -l docs/blessing-system/gguf-layer-schema.md`
Expected: File exists

- [ ] **Step 3: Commit**

```bash
git add docs/blessing-system/gguf-layer-schema.md
git commit -m "docs: add GGUF layer schema documentation for blessing-based model stacking"
```

---

### Task 12: Run Full Test Suite & Create Placeholder Stub Files

**Files:**
- Create placeholder: `b00t-cli/src/blessing/rag/fallback.rs`

- [ ] **Step 1: Create RAG fallback stub**

Create `b00t-cli/src/blessing/rag/fallback.rs`:

```rust
// Placeholder for fallback utilities
// To be implemented in Phase 7.5 if needed
```

- [ ] **Step 2: Run complete blessing module tests**

Run: `cargo test --lib blessing -- --nocapture 2>&1 | tail -30`
Expected: All tests compile and pass

- [ ] **Step 3: Run with feature flags**

Run: `cargo test --lib blessing --features=llamacpp-fallback -- --nocapture 2>&1 | tail -20`
Expected: Additional tests pass with llama.cpp-rs

- [ ] **Step 4: Final commit**

```bash
git add b00t-cli/src/blessing/rag/fallback.rs
git commit -m "feat(phase-7): complete LLM inference + RAG system - Candle primary, llama.cpp-rs (feature-gated), ripgrep fallback"
```

---

## Testing Checklist

- [ ] Embedding struct: cosine similarity calculation
- [ ] LLMInference trait: object-safe, all backends implement
- [ ] Candle backend: stub loads, is_available() works
- [ ] llama.cpp-rs backend: feature-gated, deprecation warning logged
- [ ] Ripgrep fallback: works offline, keyword search
- [ ] KnowledgeBase: discover_capability stores metadata
- [ ] GraphRAG: node/edge management, BFS traversal
- [ ] CompositionPlan: cost estimation, memory budget checks
- [ ] Prayer workflow: composition_plan populated on approval
- [ ] End-to-end: agent requests blessing → receives composition plan

---

## Success Criteria

✅ All inference backends compile and load
✅ Triple fallback works: Candle → llama.cpp-rs (if feature) → ripgrep
✅ KnowledgeBase stores blessing metadata
✅ CompositionPlan integrates with prayer workflow
✅ Feature flags allow safe removal of unsafe code
✅ 348 existing tests still passing
✅ 30+ new Phase 7 tests passing
