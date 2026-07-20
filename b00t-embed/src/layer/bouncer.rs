use crate::Embedding;
use crate::layer::trait_def::TensorSource;
use crate::layer::{LayerError, LayerId};

use async_trait::async_trait;
use dyn_clone::DynClone;

/// A single gate in the layer lifecycle validation chain.
///
/// OCI analogy: each OCI layer is validated (digest → size → content type)
/// before being mounted into the container rootfs.
#[async_trait]
pub trait LayerGate: DynClone + Send + Sync {
    /// Gate identifier (e.g., "tensor-shape-check", "dim-coherence")
    fn name(&self) -> &'static str;

    /// Validate that a layer source is safe/valid BEFORE loading.
    async fn validate_pre_load(
        &self,
        id: &LayerId,
        source: &dyn TensorSource,
    ) -> Result<(), LayerError>;

    /// Verify that the embedding after a swap is coherent.
    async fn verify_post_swap(
        &self,
        id: &LayerId,
        before: &Embedding,
        after: &Embedding,
    ) -> Result<(), LayerError>;
}

dyn_clone::clone_trait_object!(LayerGate);

// ---------------------------------------------------------------------------
// Concrete gates
// ---------------------------------------------------------------------------

/// Ensures tensor specs match between the source and what the model expects.
#[derive(Clone)]
pub struct TensorShapeGate {
    /// Minimum number of tensors a layer must provide
    pub min_tensors: usize,
    /// Maximum allowed embedding dimension
    pub max_embedding_dim: usize,
}

#[async_trait]
impl LayerGate for TensorShapeGate {
    fn name(&self) -> &'static str {
        "tensor-shape-check"
    }

    async fn validate_pre_load(
        &self,
        id: &LayerId,
        source: &dyn TensorSource,
    ) -> Result<(), LayerError> {
        let specs = source.tensor_specs();
        if specs.len() < self.min_tensors {
            return Err(LayerError::gate_rejected(
                self.name(),
                format!(
                    "layer {}: expected >= {} tensors, got {}",
                    id,
                    self.min_tensors,
                    specs.len()
                ),
            ));
        }
        if source.embedding_dim() == 0 || source.embedding_dim() > self.max_embedding_dim {
            return Err(LayerError::gate_rejected(
                self.name(),
                format!(
                    "layer {}: embedding dim {} out of range [1, {}]",
                    id,
                    source.embedding_dim(),
                    self.max_embedding_dim,
                ),
            ));
        }
        for spec in &specs {
            if spec.name.is_empty() {
                return Err(LayerError::gate_rejected(
                    self.name(),
                    format!("layer {}: empty tensor name", id),
                ));
            }
            if spec.shape.is_empty() || spec.shape.iter().any(|&d| d == 0) {
                return Err(LayerError::gate_rejected(
                    self.name(),
                    format!("layer {}: tensor '{}' has zero-dim shape", id, spec.name),
                ));
            }
        }
        Ok(())
    }

    async fn verify_post_swap(
        &self,
        id: &LayerId,
        before: &Embedding,
        after: &Embedding,
    ) -> Result<(), LayerError> {
        if before.data.is_empty() || after.data.is_empty() {
            return Err(LayerError::gate_rejected(
                self.name(),
                format!("layer {}: empty embedding post-swap", id),
            ));
        }
        if before.data.len() != after.data.len() {
            return Err(LayerError::gate_rejected(
                self.name(),
                format!(
                    "layer {}: embedding dim mismatch before={} after={}",
                    id,
                    before.data.len(),
                    after.data.len()
                ),
            ));
        }
        Ok(())
    }
}

impl Default for TensorShapeGate {
    fn default() -> Self {
        Self {
            min_tensors: 1,
            max_embedding_dim: 8192,
        }
    }
}

/// Checks resource availability before loading (memory, VRAM).
#[derive(Clone)]
pub struct ResourceGate {
    pub max_tensors_per_layer: usize,
}

#[async_trait]
impl LayerGate for ResourceGate {
    fn name(&self) -> &'static str {
        "resource-check"
    }

    async fn validate_pre_load(
        &self,
        id: &LayerId,
        source: &dyn TensorSource,
    ) -> Result<(), LayerError> {
        let specs = source.tensor_specs();
        if specs.len() > self.max_tensors_per_layer {
            return Err(LayerError::gate_rejected(
                self.name(),
                format!(
                    "layer {}: {} tensors exceeds max {}",
                    id,
                    specs.len(),
                    self.max_tensors_per_layer
                ),
            ));
        }
        Ok(())
    }

    async fn verify_post_swap(
        &self,
        _id: &LayerId,
        _before: &Embedding,
        _after: &Embedding,
    ) -> Result<(), LayerError> {
        Ok(())
    }
}

impl Default for ResourceGate {
    fn default() -> Self {
        Self {
            max_tensors_per_layer: 100,
        }
    }
}

/// Architecture compatibility gate.
#[derive(Clone)]
pub struct ArchitectureGate {
    pub allowed_architectures: Vec<&'static str>,
}

#[async_trait]
impl LayerGate for ArchitectureGate {
    fn name(&self) -> &'static str {
        "architecture-check"
    }

    async fn validate_pre_load(
        &self,
        id: &LayerId,
        source: &dyn TensorSource,
    ) -> Result<(), LayerError> {
        let arch = source.model_architecture();
        if !self.allowed_architectures.is_empty() && !self.allowed_architectures.contains(&arch) {
            return Err(LayerError::gate_rejected(
                self.name(),
                format!(
                    "layer {}: architecture '{}' not in allowed set {:?}",
                    id, arch, self.allowed_architectures
                ),
            ));
        }
        Ok(())
    }

    async fn verify_post_swap(
        &self,
        _id: &LayerId,
        _before: &Embedding,
        _after: &Embedding,
    ) -> Result<(), LayerError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Gate keeper — runs all registered gates in sequence
// ---------------------------------------------------------------------------

/// A single audit entry for bouncer gate transitions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GateAuditEntry {
    pub timestamp: String,
    pub layer_id: String,
    pub gate: String,
    pub phase: String,
    pub decision: String,
    pub reason: String,
}

/// Runs all registered LayerGates in order for a given lifecycle event.
#[derive(Clone)]
pub struct LayerGateKeeper {
    pub gates: Vec<Box<dyn LayerGate>>,
    pub enabled: bool,
    /// Audit trail of all gate decisions
    pub audit_log: std::sync::Arc<std::sync::Mutex<Vec<GateAuditEntry>>>,
    /// Optional file path for persistent audit JSONL output
    pub audit_path: Option<std::path::PathBuf>,
}

impl LayerGateKeeper {
    pub fn new(enabled: bool) -> Self {
        Self {
            gates: Vec::new(),
            enabled,
            audit_log: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            audit_path: None,
        }
    }

    /// Create with default gate set.
    pub fn with_defaults() -> Self {
        let mut k = Self::new(true);
        k.register(Box::new(TensorShapeGate::default()));
        k.register(Box::new(ResourceGate::default()));
        k
    }

    /// Create with defaults + architecture constraint.
    pub fn with_architectures(archs: Vec<&'static str>) -> Self {
        let mut k = Self::with_defaults();
        k.register(Box::new(ArchitectureGate {
            allowed_architectures: archs,
        }));
        k
    }

    /// Set the persistent audit log file path. JSONL format.
    pub fn with_audit_path(mut self, path: impl Into<std::path::PathBuf>) -> Self {
        self.audit_path = Some(path.into());
        self
    }

    pub fn register(&mut self, gate: Box<dyn LayerGate>) {
        self.gates.push(gate);
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Write an audit entry to both in-memory log and optional file.
    fn audit(&self, id: &LayerId, gate: &str, phase: &str, decision: &str, reason: &str) {
        let entry = GateAuditEntry {
            timestamp: chrono::Utc::now().to_rfc3339(),
            layer_id: id.to_string(),
            gate: gate.to_string(),
            phase: phase.to_string(),
            decision: decision.to_string(),
            reason: reason.to_string(),
        };
        if let Ok(mut log) = self.audit_log.lock() {
            log.push(entry.clone());
        }
        if let Some(ref path) = self.audit_path {
            if let Ok(json) = serde_json::to_string(&entry) {
                if let Ok(mut file) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    use std::io::Write;
                    let _ = writeln!(file, "{json}");
                }
            }
        }
    }

    pub async fn validate_pre_load(
        &self,
        id: &LayerId,
        source: &dyn TensorSource,
    ) -> Result<(), LayerError> {
        if !self.enabled {
            return Ok(());
        }
        for gate in &self.gates {
            match gate.validate_pre_load(id, source).await {
                Ok(()) => self.audit(id, gate.name(), "pre-load", "pass", "gate check ok"),
                Err(e) => {
                    self.audit(id, gate.name(), "pre-load", "fail", &e.to_string());
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    pub async fn verify_post_swap(
        &self,
        id: &LayerId,
        before: &Embedding,
        after: &Embedding,
    ) -> Result<(), LayerError> {
        if !self.enabled {
            return Ok(());
        }
        for gate in &self.gates {
            match gate.verify_post_swap(id, before, after).await {
                Ok(()) => self.audit(id, gate.name(), "post-swap", "pass", "coherence check ok"),
                Err(e) => {
                    self.audit(id, gate.name(), "post-swap", "fail", &e.to_string());
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// Drain and return all audit entries.
    pub fn drain_audit(&self) -> Vec<GateAuditEntry> {
        if let Ok(mut log) = self.audit_log.lock() {
            log.drain(..).collect()
        } else {
            Vec::new()
        }
    }

    /// Print audit summary to stdout.
    pub fn print_audit_summary(&self) {
        let entries = self.drain_audit();
        if entries.is_empty() {
            println!("    (no audit entries)");
            return;
        }
        println!("    Gate audit log ({} entries):", entries.len());
        for entry in &entries {
            let icon = if entry.decision == "pass" {
                "✓"
            } else {
                "✗"
            };
            println!(
                "      {icon} layer={} gate={} phase={} reason={}",
                entry.layer_id, entry.gate, entry.phase, entry.reason
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::TensorSpec;
    use crate::layer::source::SafetensorsSource;

    fn dummy_source(embedding_dim: usize) -> SafetensorsSource {
        SafetensorsSource::new(
            "test-layer",
            "/tmp/test.safetensors",
            vec![TensorSpec::new(
                "test.weight",
                vec![embedding_dim, 768],
                "F32",
            )],
            embedding_dim,
            "bert",
        )
    }

    #[tokio::test]
    async fn test_tensor_shape_gate_passes() {
        let gate = TensorShapeGate::default();
        let source = dummy_source(384);
        let id = LayerId::new("test-layer");

        let result = gate.validate_pre_load(&id, &source).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_tensor_shape_gate_fails_empty_dim() {
        let gate = TensorShapeGate::default();
        let source = SafetensorsSource::new(
            "bad-layer",
            "/tmp/bad.safetensors",
            vec![TensorSpec::new("bad.weight", vec![0, 768], "F32")],
            0,
            "bert",
        );
        let id = LayerId::new("bad-layer");

        let result = gate.validate_pre_load(&id, &source).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_post_swap_dimension_check() {
        let gate = TensorShapeGate::default();
        let id = LayerId::new("test");

        let before = Embedding {
            data: vec![0.1; 384],
        };
        let after = Embedding {
            data: vec![0.2; 384],
        };
        assert!(gate.verify_post_swap(&id, &before, &after).await.is_ok());

        let bad_after = Embedding {
            data: vec![0.2; 512],
        };
        assert!(
            gate.verify_post_swap(&id, &before, &bad_after)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_architecture_gate() {
        let gate = ArchitectureGate {
            allowed_architectures: vec!["bert"],
        };
        let source = dummy_source(384);
        let id = LayerId::new("test");
        assert!(gate.validate_pre_load(&id, &source).await.is_ok());

        let qwen_source = SafetensorsSource::new(
            "qwen-layer",
            "/tmp/qwen.safetensors",
            vec![TensorSpec::new("qwen.weight", vec![768, 768], "F32")],
            768,
            "qwen3",
        );
        assert!(gate.validate_pre_load(&id, &qwen_source).await.is_err());
    }
}
