// b00t-cli/src/blessing/inference/llamacpp.rs
// llama.cpp-rs backend: CPU-optimized inference via llama.cpp wrapper
// Task 4: Sophisticated error handling, device detection (CPU-only), ModelCache trait, deprecation warning
// 🤓 DeviceInfo detection: Always CPU for Phase 7, Phase 8 may add GPU support
// 🤓 Deprecation: Set deprecated=true, log warning "⚠️ llama.cpp-rs backend loaded. This backend will be removed in Phase 9."

#[cfg(feature = "llamacpp-fallback")]
use super::{Embedding, LLMInference, ModelInfo};
#[cfg(feature = "llamacpp-fallback")]
use anyhow::{Context, Result, anyhow};
#[cfg(feature = "llamacpp-fallback")]
use async_trait::async_trait;
#[cfg(feature = "llamacpp-fallback")]
use chrono::{DateTime, Utc};
#[cfg(feature = "llamacpp-fallback")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "llamacpp-fallback")]
/// Device information: CPU-only in Phase 7 (Phase 8 may add GPU)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Device type: "cpu" (Phase 7), future: "cuda", "rocm"
    pub device_type: String,
    /// Version string (e.g., "llama.cpp-2.x")
    pub version: String,
}

#[cfg(feature = "llamacpp-fallback")]
impl DeviceInfo {
    /// Detect available device via CPU fallback
    /// Phase 7: Always CPU, Phase 8+ may add GPU detection
    pub fn available() -> String {
        // Phase 7: CPU-only fallback
        // Phase 8: Add nvidia-smi detection similar to Candle
        "cpu".to_string()
    }
}

#[cfg(feature = "llamacpp-fallback")]
/// Model cache statistics for Phase 8 refinement
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheStats {
    /// Model identifier (e.g., "all-MiniLM-L6-v2")
    pub model_id: String,
    /// Bytes on disk
    pub bytes_on_disk: u64,
    /// Quantization level (e.g., "q4", "q8")
    pub quantization: String,
    /// Last accessed timestamp
    pub last_accessed: DateTime<Utc>,
}

#[cfg(feature = "llamacpp-fallback")]
/// llama.cpp-rs backend for CPU-based inference
/// Uses GGUF quantized models for efficient inference
/// DEPRECATED: Will be removed in Phase 9. Use Candle backend for GPU or ripgrep for fallback.
pub struct LlamaCppBackend {
    model_info: ModelInfo,
    /// Timestamp when backend was loaded
    loaded_at: Option<DateTime<Utc>>,
    /// Device information (CPU-only in Phase 7)
    device: DeviceInfo,
    /// Deprecation flag: true, logs warning in new()
    deprecated: bool,
}

#[cfg(feature = "llamacpp-fallback")]
impl LlamaCppBackend {
    /// Create new llama.cpp backend instance with deprecation warning
    /// Sets loaded_at to current UTC time and logs deprecation notice
    pub fn new(model_id: String, embedding_dim: u32) -> Self {
        // Log deprecation warning
        eprintln!("⚠️ llama.cpp-rs backend loaded. This backend will be removed in Phase 9.");

        let device_str = DeviceInfo::available();
        let available = true; // Phase 7: CPU always available

        Self {
            model_info: ModelInfo {
                model_id,
                embedding_dim,
                backend_name: "llamacpp".to_string(),
                available,
            },
            loaded_at: Some(Utc::now()),
            device: DeviceInfo {
                device_type: device_str.clone(),
                version: "llama.cpp-2.x".to_string(), // Phase 8: detect actual version
            },
            deprecated: true,
        }
    }

    /// Get timestamp when backend was loaded
    pub fn loaded_at(&self) -> Option<DateTime<Utc>> {
        self.loaded_at
    }

    /// Get device information
    pub fn device(&self) -> &DeviceInfo {
        &self.device
    }

    /// Check if backend is deprecated (always true in Phase 7)
    pub fn is_deprecated(&self) -> bool {
        self.deprecated
    }

    /// Get cache statistics for model
    /// 🦨 TODO: Phase 8 - calculate from model_id + quantization
    /// Currently returns stub values with proper timestamp
    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            model_id: self.model_info.model_id.clone(),
            bytes_on_disk: 0, // 🦨 Phase 8: calculate from actual model size
            quantization: "q8".to_string(), // 🦨 Phase 8: detect quantization level
            last_accessed: Utc::now(),
        }
    }
}

#[cfg(feature = "llamacpp-fallback")]
#[async_trait]
impl LLMInference for LlamaCppBackend {
    async fn embed(&self, _text: &str) -> Result<Embedding> {
        // Task 4: Stub zero-vector for Phase 3
        // Phase 4: Use llama_cpp_sys to run GGUF model with sophisticated error handling
        // Will return anyhow::Result with rich error context
        Ok(Embedding {
            data: vec![0.0; self.model_info.embedding_dim as usize],
        })
    }

    async fn compose_layers(&mut self, _blessing_ids: &[&str]) -> Result<()> {
        // Task 4: Stub no-op
        // Phase 5: Use llama.cpp for multi-layer tensor operations
        Ok(())
    }

    fn is_available(&self) -> bool {
        // Phase 7: Check CPU availability (always true)
        // Phase 8: Add GPU detection via nvidia-smi or similar
        self.model_info.available
    }

    fn model_info(&self) -> ModelInfo {
        self.model_info.clone()
    }
}
