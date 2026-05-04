// b00t-cli/src/blessing/inference/candle.rs
// Candle backend: Meta's Rust ML framework with GPU acceleration
// Task 3: Sophisticated error handling, device detection via nvidia-smi, ModelCache trait
// 🤓 DeviceInfo detection: nvidia-smi → env vars → CPU fallback

#[cfg(feature = "candle")]
use super::{Embedding, LLMInference, ModelInfo};
#[cfg(feature = "candle")]
use anyhow::{anyhow, Context, Result};
#[cfg(feature = "candle")]
use async_trait::async_trait;
#[cfg(feature = "candle")]
use chrono::{DateTime, Utc};
#[cfg(feature = "candle")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "candle")]
use std::process::Command;

#[cfg(feature = "candle")]
/// Device information: GPU or CPU with version detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfo {
    /// Device type: "cuda", "rocm", "cpu"
    pub device_type: String,
    /// CUDA compute capability or version string
    pub version: String,
}

#[cfg(feature = "candle")]
impl DeviceInfo {
    /// Detect available device via sophisticated error handling
    /// Fallback chain: nvidia-smi → CUDA_VISIBLE_DEVICES env → CPU fallback
    pub fn available() -> String {
        // Phase 1: Try nvidia-smi for GPU detection
        if let Ok(output) = Command::new("nvidia-smi")
            .arg("--query-gpu=compute_cap")
            .arg("--format=csv,noheader")
            .output()
        {
            if output.status.success() {
                if let Ok(cuda_version) = String::from_utf8(output.stdout) {
                    let version = cuda_version.trim().to_string();
                    if !version.is_empty() {
                        return format!("cuda-{}", version);
                    }
                }
            }
        }

        // Phase 2: Check CUDA_VISIBLE_DEVICES environment variable
        if let Ok(cuda_devices) = std::env::var("CUDA_VISIBLE_DEVICES") {
            if !cuda_devices.is_empty() && cuda_devices != "-1" {
                return format!("cuda-env:{}", cuda_devices);
            }
        }

        // Phase 3: Check CUDA_HOME environment variable
        if let Ok(_cuda_home) = std::env::var("CUDA_HOME") {
            return "cuda-home".to_string();
        }

        // Final fallback: CPU inference
        "cpu".to_string()
    }
}

#[cfg(feature = "candle")]
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

#[cfg(feature = "candle")]
/// Candle-based embedding backend with GPU acceleration and sophisticated error handling
pub struct CandleBackend {
    model_info: ModelInfo,
    /// Timestamp when backend was loaded
    loaded_at: Option<DateTime<Utc>>,
    /// Device information with version detection
    device: DeviceInfo,
}

#[cfg(feature = "candle")]
impl CandleBackend {
    /// Create new Candle backend instance with device detection
    /// Sets loaded_at to current UTC time and detects GPU/CPU availability
    pub fn new(model_id: String, embedding_dim: u32) -> Self {
        let device_str = DeviceInfo::available();
        let available = !device_str.starts_with("cpu");

        Self {
            model_info: ModelInfo {
                model_id,
                embedding_dim,
                backend_name: "candle".to_string(),
                available,
            },
            loaded_at: Some(Utc::now()),
            device: DeviceInfo {
                device_type: device_str.split('-').next().unwrap_or("cpu").to_string(),
                version: device_str.to_string(),
            },
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

#[cfg(feature = "candle")]
#[async_trait]
impl LLMInference for CandleBackend {
    async fn embed(&self, _text: &str) -> Result<Embedding> {
        // Task 3: Stub zero-vector for Phase 3
        // Phase 4: Use Candle to run embedding model with sophisticated error handling
        // Will return anyhow::Result with rich error context
        Ok(Embedding {
            data: vec![0.0; self.model_info.embedding_dim as usize],
        })
    }

    async fn compose_layers(&mut self, _blessing_ids: &[&str]) -> Result<()> {
        // Task 3: Stub no-op
        // Phase 5: Use Candle for multi-layer tensor operations
        Ok(())
    }

    fn is_available(&self) -> bool {
        // Phase 3: Check actual GPU/CUDA availability via device detection
        self.model_info.available
    }

    fn model_info(&self) -> ModelInfo {
        self.model_info.clone()
    }
}

#[cfg(feature = "candle")]
/// Generate text using Candle inference with a HuggingFace model (Qwen2.5-0.5B-Instruct).
/// Uses DeviceInfo::available() for GPU detection (nvidia-smi → env → CPU fallback).
/// Downloads model weights via hf-hub on first run (cached in ~/.cache/huggingface/hub).
pub fn generate_text(prompt: &str) -> Result<String> {
    use candle_core::{Device, DType, Tensor};
    use candle_nn::VarBuilder;
    use candle_transformers::models::qwen2::{Config, Model};
    use hf_hub::api::sync::Api;
    use tokenizers::Tokenizer;

    // Phase 1: Device detection (GPU via nvidia-smi, CPU fallback)
    let device_str = DeviceInfo::available();
    let device = if device_str.starts_with("cuda") {
        Device::new_cuda(0).map_err(|e| anyhow!("CUDA init: {e}"))?
    } else {
        Device::Cpu
    };

    // Phase 2: Download/load model from Hugging Face hub
    let api = Api::new().map_err(|e| anyhow!("HF Hub init (check HF_TOKEN or network): {e}"))?;
    let repo = api.model("Qwen/Qwen2.5-0.5B-Instruct".to_string());

    // Phase 3: Load tokenizer
    let tokenizer_path = repo
        .get("tokenizer.json")
        .map_err(|e| anyhow!("Download tokenizer.json: {e}"))?;
    let tokenizer = Tokenizer::from_file(tokenizer_path)
        .map_err(|e| anyhow!("Tokenizer load: {e}"))?;

    // Phase 4: Load model config
    let config_path = repo
        .get("config.json")
        .map_err(|e| anyhow!("Download config.json: {e}"))?;
    let config_str = std::fs::read_to_string(config_path)?;
    let config: Config =
        serde_json::from_str(&config_str).map_err(|e| anyhow!("Parse Qwen2 config: {e}"))?;

    // Phase 5: Load model weights (single or sharded safetensors)
    let vb = if let Ok(path) = repo.get("model.safetensors") {
        // Single-file weights
        unsafe { VarBuilder::from_mmaped_safetensors(&[path], DType::F32, &device) }
            .map_err(|e| anyhow!("Load model.safetensors: {e}"))?
    } else {
        // Sharded weights — read index to discover all shard files
        let index_path = repo
            .get("model.safetensors.index.json")
            .map_err(|e| anyhow!("No model.safetensors or index.json: {e}"))?;
        let index_content = std::fs::read_to_string(index_path)?;
        let index: serde_json::Value = serde_json::from_str(&index_content)?;
        let weight_map = index["weight_map"]
            .as_object()
            .ok_or_else(|| anyhow!("Missing weight_map in model.safetensors.index.json"))?;

        let mut shards: Vec<std::path::PathBuf> = weight_map
            .values()
            .filter_map(|v| v.as_str())
            .map(|f| repo.get(f).unwrap_or_else(|_| std::path::PathBuf::from(f)))
            .collect();
        shards.sort();
        shards.dedup();

        unsafe { VarBuilder::from_mmaped_safetensors(&shards, DType::F32, &device) }
            .map_err(|e| anyhow!("Load {} shard files: {e}", shards.len()))?
    };

    // Phase 6: Build model from weights
    let mut model = Model::new(&config, vb).map_err(|e| anyhow!("Build Qwen2 model: {e}"))?;

    // Phase 7: Tokenize input with Qwen2.5 chat template
    let formatted = format!(
        "<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
        prompt
    );
    let encoding = tokenizer
        .encode(formatted, true)
        .map_err(|e| anyhow!("Tokenize: {e}"))?;
    let input_ids: Vec<u32> = encoding.get_ids().to_vec();
    let input_len = input_ids.len();
    let mut tokens: Vec<u32> = input_ids.clone();
    let eos_id: u32 = tokenizer.token_to_id("<|im_end|>").unwrap_or(151645);
    let max_new: usize = 200;

    // Phase 8: Autoregressive generation loop (greedy / argmax)
    let mut next_token: u32 = 0;
    for i in 0..max_new {
        // First step: full input; subsequent steps: single new token (KV cache)
        let input = if i == 0 {
            Tensor::new(input_ids.as_slice(), &device)
                .map_err(|e| anyhow!("Create input tensor: {e}"))?
                .unsqueeze(0)?
        } else {
            Tensor::new(&[next_token], &device)
                .map_err(|e| anyhow!("Create token tensor: {e}"))?
                .unsqueeze(0)?
        };

        let logits = model
            .forward(&input, i, None::<&candle_core::Tensor>)
            .map_err(|e| anyhow!("Forward pass step {i}: {e}"))?;

        // logits: [1, seq_len, vocab_size] → extract last token → [vocab_size]
        let next_logit = logits
            .squeeze(0)
            .map_err(|e| anyhow!("Squeeze batch dim step {i}: {e}"))?;
        let seq_len = next_logit.dims()[0];
        let next_logit = next_logit
            .narrow(0, seq_len - 1, 1)
            .map_err(|e| anyhow!("Narrow last token step {i}: {e}"))?
            .squeeze(0)
            .map_err(|e| anyhow!("Squeeze seq dim step {i}: {e}"))?;

        // argmax over vocab dimension → scalar token index (u32)
        next_token = next_logit
            .argmax(0)
            .map_err(|e| anyhow!("Argmax step {i}: {e}"))?
            .to_scalar::<u32>()
            .map_err(|e| anyhow!("Token scalar step {i}: {e}"))?;

        tokens.push(next_token);

        if next_token == eos_id {
            break;
        }
    }

    // Phase 9: Decode only the newly generated tokens
    let generated = &tokens[input_len..];
    let output = tokenizer
        .decode(generated, true)
        .map_err(|e| anyhow!("Decode output: {e}"))?;

    Ok(output.trim().to_string())
}
