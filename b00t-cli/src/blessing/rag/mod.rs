// b00t-cli/src/blessing/rag/mod.rs
// Knowledge Base: Semantic discovery and layer caching for blessings
// Phase 7: KnowledgeBase struct + metadata + Phase 8 integration hooks
// Phase 8: Semantic discovery pipeline (assimilate integration)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use chrono::{DateTime, Utc};

/// Metadata about a discovered blessing capability
/// Stores discovery context and quality metrics from prayer workflow
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BlessingMetadata {
    /// Unique blessing identifier (e.g., "blessing:terraform-apply")
    pub blessing_id: String,

    /// Which datum this blessing was discovered from (e.g., "terraform", "aws")
    pub source_datum: String,

    /// When this blessing was discovered
    pub discovered_at: DateTime<Utc>,

    /// Quality score reflecting denial rates from prayer workflow (0.0 to 1.0)
    /// 1.0 = always approved, 0.0 = always denied
    /// Updated by prayer feedback loop
    pub quality_score: f32,

    /// Blessing IDs this capability depends on
    pub depends_on: Vec<String>,

    /// Path to the generated layer artifact
    pub layer_path: PathBuf,
}

impl BlessingMetadata {
    /// Validate quality score is in valid range [0.0, 1.0]
    pub fn is_valid(&self) -> bool {
        self.quality_score >= 0.0 && self.quality_score <= 1.0
    }
}

/// Metadata for a generated layer (GGUF adapter)
/// Task 9 integration: tracks embedding dimension, adapter rank, generation timestamp
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LayerMetadata {
    /// Which blessing this layer implements
    pub blessing_id: String,

    /// Path to GGUF artifact (e.g., /cache/blessing_terraform_apply.gguf)
    pub artifact_path: PathBuf,

    /// Embedding dimension (typically 768, 1024, 4096)
    pub embedding_dim: u32,

    /// LoRA adapter rank (typically 8, 16, 32)
    pub adapter_rank: u32,

    /// When layer was generated
    pub generated_at: DateTime<Utc>,

    /// Quality score for this layer (0.0 to 1.0)
    pub quality_score: f32,
}

/// 🦨 Phase 8: Semantic discovery callback trait
/// Called when capability is discovered or quality changes
/// Implemented by assimilate module to integrate discovery feedback
pub trait SemanticDiscoveryCallback: Send + Sync {
    /// Called when capability is discovered
    fn on_capability_discovered(&self, blessing_id: &str, metadata: &BlessingMetadata);

    /// Called when quality score changes (denial feedback from prayer workflow)
    /// old_score: previous quality score
    /// new_score: updated quality score after feedback
    fn on_quality_score_updated(&self, blessing_id: &str, old_score: f32, new_score: f32);
}

/// Knowledge Base: Semantic discovery and layer caching
/// Stores discovered blessing capabilities and their generated layers
pub struct KnowledgeBase {
    /// All discovered blessings and their metadata
    /// Key: blessing_id, Value: BlessingMetadata
    pub metadata: HashMap<String, BlessingMetadata>,

    /// Cache of generated layers (GGUF adapters)
    /// Key: blessing_id, Value: LayerMetadata
    pub layer_cache: HashMap<String, LayerMetadata>,

    /// Root directory for knowledge index files
    pub index_dir: PathBuf,

    /// 🦨 Phase 8: Callback for semantic discovery events
    /// Registered by assimilate module to integrate discovery feedback
    pub discovery_callback: Option<Arc<dyn SemanticDiscoveryCallback>>,
}

impl KnowledgeBase {
    /// Create or open a knowledge base at the given index directory
    ///
    /// # Arguments
    /// * `index_dir` - Root directory for knowledge index (typically ~/.b00t/knowledge_index)
    ///
    /// # Returns
    /// New KnowledgeBase with empty metadata and layer cache
    pub async fn new(index_dir: &str) -> Self {
        let index_path = PathBuf::from(index_dir);

        // 🤓 Ensure directory exists for Phase 9 persistence
        let _ = tokio::fs::create_dir_all(&index_path).await;

        KnowledgeBase {
            metadata: HashMap::new(),
            layer_cache: HashMap::new(),
            index_dir: index_path,
            discovery_callback: None,
        }
    }

    /// Discover a new capability and store its metadata
    ///
    /// # Arguments
    /// * `blessing` - Metadata about the discovered blessing
    /// * `context` - Context of discovery (agent role, timestamp, etc.) as JSON
    ///
    /// # Behavior
    /// 1. Store blessing metadata in HashMap
    /// 2. Save to index_dir (Phase 9 persistence)
    /// 3. Invoke discovery_callback if registered (Phase 8)
    pub async fn discover_capability(&mut self, blessing: BlessingMetadata, context: &serde_json::Value) {
        let blessing_id = blessing.blessing_id.clone();

        // Store in metadata HashMap
        self.metadata.insert(blessing_id.clone(), blessing.clone());

        // 🦨 Phase 8: Invoke callback if registered
        if let Some(callback) = &self.discovery_callback {
            callback.on_capability_discovered(&blessing_id, &blessing);
        }

        // 🤓 Phase 9: Save to index_dir (stub for future persistence)
        // Index will contain TOML/JSON manifests of discovered capabilities
        // Path: {index_dir}/blessings/{blessing_id}.toml
        let _context = context; // Future: use context for audit trail
    }

    /// Generate a layer (GGUF adapter) for a blessing
    ///
    /// # Arguments
    /// * `blessing_id` - Which blessing to generate layer for
    ///
    /// # Returns
    /// LayerMetadata with generated artifact path, embedding dim, adapter rank
    ///
    /// # Behavior
    /// 1. Create LayerMetadata with standard defaults
    /// 2. Compute artifact path using adapter_path_for()
    /// 3. Cache in layer_cache HashMap
    /// 4. Return metadata (actual artifact generation in Task 9)
    pub async fn generate_layer(&mut self, blessing_id: &str) -> LayerMetadata {
        let artifact_path = self.adapter_path_for(blessing_id);

        let layer = LayerMetadata {
            blessing_id: blessing_id.to_string(),
            artifact_path,
            embedding_dim: 768,  // 🤓 Standard embedding dimension (LLAMA 3.1)
            adapter_rank: 8,      // 🤓 Standard LoRA rank
            generated_at: Utc::now(),
            quality_score: 0.85,  // 🤓 Default quality score
        };

        // Cache the layer
        self.layer_cache.insert(blessing_id.to_string(), layer.clone());

        layer
    }

    /// Compute adapter path for a blessing's GGUF layer
    ///
    /// # Format
    /// {index_dir}/adapters/blessing_{blessing_id_normalized}.adapter
    ///
    /// # Example
    /// Input: "blessing:terraform-apply"
    /// Output: "/home/user/.b00t/knowledge_index/adapters/blessing_terraform_apply.adapter"
    pub fn adapter_path_for(&self, blessing_id: &str) -> PathBuf {
        // Normalize blessing_id: replace ':' and '-' with '_'
        let normalized = blessing_id
            .replace(":", "_")
            .replace("-", "_")
            .to_lowercase();

        self.index_dir.join("adapters").join(format!("{}.adapter", normalized))
    }

    /// Register a semantic discovery callback (Phase 8)
    ///
    /// # Arguments
    /// * `callback` - Arc'd callback implementing SemanticDiscoveryCallback
    ///
    /// # Usage (Phase 8)
    /// ```ignore
    /// let kb = KnowledgeBase::new("/path/to/index").await;
    /// let callback = Arc::new(AssimilateSemanticDiscovery::new());
    /// kb.register_discovery_callback(callback);
    /// ```
    pub fn register_discovery_callback(&mut self, callback: Arc<dyn SemanticDiscoveryCallback>) {
        self.discovery_callback = Some(callback);
    }

    /// Update quality score and notify callback (Phase 8)
    ///
    /// # Arguments
    /// * `blessing_id` - Which blessing's quality to update
    /// * `new_score` - New quality score (0.0 to 1.0)
    ///
    /// # Behavior
    /// 1. Update metadata quality_score
    /// 2. Invoke callback with old/new scores if registered
    /// 3. Used by prayer workflow feedback loop
    pub async fn update_quality_score(&mut self, blessing_id: &str, new_score: f32) {
        if let Some(metadata) = self.metadata.get_mut(blessing_id) {
            let old_score = metadata.quality_score;
            metadata.quality_score = new_score.clamp(0.0, 1.0);

            // 🦨 Phase 8: Notify callback of quality change
            if let Some(callback) = &self.discovery_callback {
                callback.on_quality_score_updated(blessing_id, old_score, metadata.quality_score);
            }
        }
    }
}

pub mod graph;

#[cfg(test)]
mod tests;
