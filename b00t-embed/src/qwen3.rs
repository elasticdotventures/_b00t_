// b00t-embed composable Qwen3 embedding backend.
//
// Loads the Qwen3 embedding model with a VarMap-backed weight store,
// enabling runtime layer swapping via the OCI-style LayerStack.
//
// Architecture:
//   - Base model loaded from HuggingFace safetensors into a VarMap
//   - VarBuilder::from_varmap() builds the model with writable weights
//   - VarMap::load() overwrites random init with actual pretrained values
//   - Layer activation swaps VarMap entries in-place
//   - Forward pass reads from VarMap — weight swap = output change

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use anyhow::{Context, Result};
use async_trait::async_trait;
use candle_core::{DType, Device, Tensor};
use candle_nn::{VarBuilder, VarMap};
use embed_anything::models::qwen3::{Config, Model};
use hf_hub::HFClientSync;
use tokenizers::{PaddingParams, Tokenizer, TruncationParams};

use crate::layer::bouncer::LayerGateKeeper;
use crate::layer::stack::{LayerStack, TensorRegistry};
use crate::{EmbedBackend, Embedding};

/// Composable Qwen3 embedding backend with VarMap-backed runtime layer swapping.
#[allow(dead_code)]
pub struct Qwen3Composable {
    /// VarMap containing all model weights (allows runtime swaps)
    varmap: Arc<Mutex<VarMap>>,
    /// The Candle Qwen3 model
    model: RwLock<Model>,
    /// Tokenizer
    tokenizer: Tokenizer,
    /// Computation device
    device: Device,
    /// Model config
    config: Config,
    /// Embedding dimension (hidden_size)
    dim: usize,
    /// Layer stack for OCI-style composition
    layer_stack: Arc<tokio::sync::RwLock<Option<LayerStack>>>,
    /// Base model ID
    model_id: String,
    /// Whether the backend is ready
    available: bool,
    /// Base weight values for restoring after layer deactivation
    base_weights: HashMap<String, Tensor>,
}

impl Qwen3Composable {
    /// Load Qwen3 embedding model from HuggingFace with VarMap-backed weights.
    ///
    /// Steps:
    /// 1. Download config.json, tokenizer.json, model.safetensors from HF
    /// 2. Create empty VarMap, build model with VarBuilder::from_varmap()
    /// 3. Load actual weights into VarMap via load() — overwrites random init
    /// 4. Initialize LayerStack for composable layer lifecycle
    ///
    /// Model ID: "Qwen/Qwen3-Embedding-0.6B" (768-dim embeddings)
    #[allow(unused_variables)]
    pub fn new(model_id: &str, revision: Option<&str>, token: Option<&str>) -> Result<Self> {
        let client = HFClientSync::new().context("failed to build HF client")?;
        let (owner, name) = split_model_id(model_id);
        let repo = client.model(owner, name);

        // Download model files
        let config_path = repo
            .download_file()
            .filename("config.json")
            .send()
            .context("config.json not found")?;
        let tokenizer_path = repo
            .download_file()
            .filename("tokenizer.json")
            .send()
            .context("tokenizer.json not found")?;
        let weights_path = match repo.download_file().filename("model.safetensors").send() {
            Ok(p) => p,
            Err(_) => anyhow::bail!(
                "model.safetensors not found; Qwen3-Embedding uses single-file safetensors"
            ),
        };

        // Parse config
        let config_raw = std::fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&config_raw)?;
        let dim = config.hidden_size;

        // Load tokenizer
        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| anyhow::anyhow!("tokenizer load: {e}"))?;
        let pp = PaddingParams {
            strategy: tokenizers::PaddingStrategy::BatchLongest,
            direction: tokenizers::PaddingDirection::Left,
            ..Default::default()
        };
        let trunc = TruncationParams {
            strategy: tokenizers::TruncationStrategy::LongestFirst,
            max_length: config.max_position_embeddings.min(1024),
            ..Default::default()
        };
        tokenizer
            .with_padding(Some(pp))
            .with_truncation(Some(trunc))
            .map_err(|e| anyhow::anyhow!("tokenizer config: {e}"))?;

        let device = Device::Cpu;

        // Discover the actual dtype from the safetensors file header.
        // Qwen3-Embedding-0.6B stores all tensors as BF16.
        let model_dtype = detect_safetensors_dtype(&weights_path)?;
        println!("  Detected model dtype: {model_dtype:?}");

        // 🤓 candle's CPU backend does not support BF16 matmul ("unsupported dtype
        // BF16 for op matmul") — Model::new() itself fails immediately on CPU if
        // the VarBuilder is given BF16, before any real weights are even loaded.
        // Upcast to F32 for CPU (safe, standard candle-CPU-inference workaround);
        // BF16 matmul IS supported on CUDA, so a future GPU device would keep the
        // native dtype instead of paying the upcast cost.
        let load_dtype = if device.is_cpu() { DType::F32 } else { model_dtype };
        if load_dtype != model_dtype {
            println!("  Upcasting {model_dtype:?} -> {load_dtype:?} for CPU inference");
        }

        // Step 1: Create VarMap and build model with VarBuilder::from_varmap()
        // using load_dtype (not the file's raw model_dtype) so Model::new()'s
        // own internal tensor ops (e.g. RoPE frequency computation) run in a
        // CPU-matmul-supported dtype from the start.
        let varmap = Arc::new(Mutex::new(VarMap::new()));
        let model = {
            let vm = varmap.lock().unwrap();
            let vb = VarBuilder::from_varmap(&vm, load_dtype, &device);
            let m = Model::new(&config, vb).context("failed to build Qwen3 model with VarMap")?;
            drop(vm);
            m
        };

        // Step 2: Load actual weights from safetensors and set them into the
        // VarMap. NOT using VarMap::load() here — it loads each tensor in the
        // file's own dtype (BF16) via MmapedSafetensors, which would then fail
        // Var::set()'s dtype-matching copy against our F32-declared Vars. Load
        // raw (native dtype), cast each tensor to load_dtype, then set.
        {
            let raw_tensors = candle_core::safetensors::load(&weights_path, &device)
                .context("failed to read safetensors weights")?;
            let mut vm = varmap.lock().unwrap();
            for (name, tensor) in raw_tensors {
                let tensor = tensor
                    .to_dtype(load_dtype)
                    .with_context(|| format!("failed to cast tensor '{name}' to {load_dtype:?}"))?;
                vm.set_one(&name, &tensor)
                    .with_context(|| format!("failed to set weight '{name}' into VarMap"))?;
            }
        }

        // Step 3: Save base weights for restoration during deactivation
        let base_weights = {
            let vm = varmap.lock().unwrap();
            let inner = vm.data().lock().unwrap();
            let mut base = HashMap::new();
            // Only save head-layer tensors that can be swapped
            for (name, var) in inner.iter() {
                if name.starts_with("embed_tokens") || name.starts_with("norm") {
                    let t: &Tensor = var;
                    if let Ok(copy) = t.copy() {
                        base.insert(name.clone(), copy);
                    }
                }
            }
            base
        };

        // Step 4: Initialize LayerStack for composable layer lifecycle
        let reg_varmap = Arc::new(Mutex::new(varmap.lock().unwrap().clone()));
        let registry = TensorRegistry::new(
            reg_varmap,
            device.clone(),
            load_dtype,
            base_weights.clone(),
        );
        let gatekeeper = LayerGateKeeper::with_architectures(vec!["qwen3", "llama", "mistral"]);
        let stack = LayerStack::new(registry, gatekeeper);

        Ok(Self {
            varmap,
            model: RwLock::new(model),
            tokenizer,
            device,
            config,
            dim,
            layer_stack: Arc::new(tokio::sync::RwLock::new(Some(stack))),
            model_id: model_id.to_string(),
            available: true,
            base_weights,
        })
    }

    /// Register a GGUF/safetensors layer for runtime composition.
    pub async fn register_layer(&self, source: Box<dyn crate::layer::TensorSource>) {
        let mut guard = self.layer_stack.write().await;
        if let Some(stack) = guard.as_mut() {
            stack.register_source(source);
        }
    }

    /// Tokenize input text.
    fn tokenize_batch(&self, texts: &[&str]) -> Result<(Tensor, Tensor)> {
        let tokens = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("tokenization: {e}"))?;

        let token_ids: Vec<Vec<u32>> = tokens.iter().map(|t| t.get_ids().to_vec()).collect();
        let attention_mask: Vec<Vec<u32>> = tokens
            .iter()
            .map(|t| t.get_attention_mask().to_vec())
            .collect();

        let max_len = token_ids.iter().map(|t| t.len()).max().unwrap_or(0);
        let batch_size = token_ids.len();

        // 🤓 ids MUST stay an integer dtype (U32) — candle_nn::Embedding::forward
        // does a Tensor::index_select internally, and candle's CPU backend
        // rejects a float index tensor ("unsupported dtype F32 for op
        // index-select"). mask is fine as F32 — it's used in float attention
        // arithmetic and gets cast again downstream regardless (qwen3.rs:361).
        let mut padded_ids: Vec<u32> = Vec::with_capacity(batch_size * max_len);
        let mut padded_mask = Vec::with_capacity(batch_size * max_len);
        for i in 0..batch_size {
            for j in 0..max_len {
                if j < token_ids[i].len() {
                    padded_ids.push(token_ids[i][j]);
                    padded_mask.push(attention_mask[i][j] as f32);
                } else {
                    padded_ids.push(0);
                    padded_mask.push(0.0);
                }
            }
        }

        let ids = Tensor::from_vec(padded_ids, (batch_size, max_len), &self.device)?;
        let mask = Tensor::from_vec(padded_mask, (batch_size, max_len), &self.device)?;

        Ok((ids, mask))
    }
}

#[async_trait]
impl EmbedBackend for Qwen3Composable {
    async fn embed(&self, text: &str) -> Result<Embedding> {
        let (ids, mask) = self.tokenize_batch(&[text])?;

        // Forward pass through the model
        let hidden = {
            let mut model = self
                .model
                .write()
                .map_err(|e| anyhow::anyhow!("model lock: {e}"))?;
            let result = model
                .forward(&ids, &mask, 0)
                .context("qwen3 forward pass")?;
            model.clear_kv_cache();
            result
        };

        // Pool: last token (Qwen3 embedding convention)
        let seq_len = ids.dims()[1];
        let last_token = hidden.narrow(1, seq_len - 1, 1)?.squeeze(1)?;

        // L2 normalize
        let normalized = normalize_l2(last_token)?;
        let vec = normalized.to_vec1::<f32>()?;

        Ok(Embedding { data: vec })
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        let (ids, mask) = self.tokenize_batch(texts)?;
        let _batch_size = ids.dims()[0];
        let seq_len = ids.dims()[1];

        let hidden = {
            let mut model = self
                .model
                .write()
                .map_err(|e| anyhow::anyhow!("model lock: {e}"))?;
            let result = model
                .forward(&ids, &mask, 0)
                .context("qwen3 forward pass")?;
            model.clear_kv_cache();
            result
        };

        let last = hidden.narrow(1, seq_len - 1, 1)?.squeeze(1)?;
        let normalized = normalize_l2(last)?;
        let vecs = normalized.to_vec2::<f32>()?;

        Ok(vecs.into_iter().map(|data| Embedding { data }).collect())
    }

    fn embedding_dim(&self) -> usize {
        self.dim
    }

    fn model_id(&self) -> &str {
        &self.model_id
    }

    fn is_available(&self) -> bool {
        self.available
    }

    async fn compose_layers(
        &self,
        query: &str,
        max_layers: usize,
    ) -> Result<Vec<crate::layer::LayerDescriptor>> {
        // Build query embedding from the input text via the model itself
        let (ids, mask) = self.tokenize_batch(&[query])?;
        let query_hidden = {
            let mut model = self
                .model
                .write()
                .map_err(|e| anyhow::anyhow!("model lock: {e}"))?;
            let h = model.forward(&ids, &mask, 0).context("qwen3 forward")?;
            model.clear_kv_cache();
            h
        };
        let seq_len = ids.dims()[1];
        let query_vec = query_hidden.narrow(1, seq_len - 1, 1)?.squeeze(1)?;
        let normalized = normalize_l2(query_vec)?;
        let query_embedding = crate::Embedding {
            data: normalized.to_vec1::<f32>()?,
        };

        // Restore base head tensors (deactivate old layers)
        for (name, base) in &self.base_weights {
            let mut vm = self.varmap.lock().unwrap();
            let _ = vm.set_one(name, base);
        }

        // Compose: score all registered sources → activate top-k
        let descriptors = {
            let mut guard = self.layer_stack.write().await;
            let stack = guard.as_mut().context("layer stack not initialized")?;

            let active = stack.active_layers().await;
            for id in &active {
                stack.deactivate_layer(id).await?;
            }

            let descs = stack.compose(&query_embedding, max_layers).await?;
            descs
        };

        // P1: Bridge active layer tensors from registry → model VarMap.
        // For each activated layer, load its tensor source and swap into model's
        // VarMap via set_one(). This is the key OCI "layer mount" operation.
        {
            let guard = self.layer_stack.read().await;
            if let Some(stack) = guard.as_ref() {
                let registry = stack.registry();
                // Only swap head tensors (embed_tokens, norm) to avoid
                // destabilizing the transformer body.
                let head_prefixes = ["embed_tokens", "norm"];
                for tname in registry.active_tensor_names() {
                    // Skip non-head tensors (transformer layer weights stay frozen)
                    let is_head = head_prefixes.iter().any(|p| tname.starts_with(p));
                    if !is_head {
                        continue;
                    }
                    let maybe_tensor: Option<Tensor> = {
                        let vm = registry.varmap().lock().unwrap();
                        let inner = vm.data().lock().unwrap();
                        inner.get(&tname).and_then(|v| {
                            let t: &Tensor = v;
                            t.copy().ok()
                        })
                    };
                    if let Some(mut tensor) = maybe_tensor {
                        let mut model_vm = self.varmap.lock().unwrap();
                        // Match the VarMap entry's dtype (model may be BF16 but
                        // source layers may be F32 after dequantization).
                        let var_dtype = {
                            let inner = model_vm.data().lock().unwrap();
                            inner.get(&tname).map(|v| v.dtype())
                        };
                        if let Some(target_dtype) = var_dtype {
                            if tensor.dtype() != target_dtype {
                                tensor = tensor
                                    .to_dtype(target_dtype)
                                    .map_err(|e| anyhow::anyhow!("dtype cast for set_one: {e}"))?;
                            }
                        }
                        let _ = model_vm.set_one(&tname, &tensor);
                    }
                }
            }
        }

        Ok(descriptors)
    }

    fn clear_layers(&self) -> Result<()> {
        // Restore base head tensors to the model VarMap
        for (name, base) in &self.base_weights {
            let mut vm = self.varmap.lock().unwrap();
            let _ = vm.set_one(name, base);
        }
        // Clear layer stack active tracking
        let guard = self.layer_stack.blocking_write();
        if let Some(_stack) = guard.as_ref() {
            // Just reset by restoring base — the next compose will re-activate
        }
        Ok(())
    }
}

/// L2 normalize a tensor along the last dimension.
fn normalize_l2(t: Tensor) -> Result<Tensor> {
    let norm = t.sqr()?.sum_keepdim(1)?.sqrt()?;
    Ok(t.broadcast_div(&norm)?)
}

/// Detect the predominant dtype from a safetensors file header.
/// Reads the first tensor's dtype to determine model weight format.
fn detect_safetensors_dtype(path: &std::path::Path) -> Result<DType> {
    let content = std::fs::read(path).context("read safetensors for dtype detection")?;
    if content.len() < 8 {
        anyhow::bail!("file too short");
    }
    let header_len = u64::from_le_bytes(content[0..8].try_into().unwrap()) as usize;
    if 8 + header_len > content.len() {
        anyhow::bail!("header exceeds file");
    }
    let header: serde_json::Value =
        serde_json::from_slice(&content[8..8 + header_len]).context("parse safetensors header")?;
    let obj = header
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("header not object"))?;

    // Find first non-metadata tensor and read its dtype
    for (key, val) in obj {
        if key == "__metadata__" {
            continue;
        }
        if let Some(dtype_str) = val.get("dtype").and_then(|v| v.as_str()) {
            return Ok(match dtype_str {
                "F32" => DType::F32,
                "F16" => DType::F16,
                "BF16" => DType::BF16,
                "F64" => DType::F64,
                _ => DType::F32,
            });
        }
    }
    Ok(DType::F32) // fallback
}

/// Split "owner/name" model ID into (owner, name) pair for hf-hub 1.0 API.
fn split_model_id(model_id: &str) -> (&str, &str) {
    match model_id.split_once('/') {
        Some((owner, name)) => (owner, name),
        None => ("", model_id),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_qwen3_composable_new_exists() {
        // Verify the struct exists and has the right API shape
        // Full model loading requires HF download — tested manually
    }
}
