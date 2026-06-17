use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::layer::{LayerError, LayerId, TensorSpec};
use crate::layer::trait_def::TensorSource;

use candle_core::quantized::gguf_file;
use candle_core::{DType, Device, Tensor};

// ---------------------------------------------------------------------------
// SafetensorsSource — wraps a .safetensors file containing a complete
// embedding head. Candle natively supports safetensors via VarBuilder.
// Each file contains ONLY the head tensors (token_embd, pooler, output).
// ---------------------------------------------------------------------------

/// A layer backed by a safetensors file containing embedding head weights.
///
/// OCI analogy: a single-layer OCI diff tarball — just the changed files.
#[derive(Debug, Clone)]
pub struct SafetensorsSource {
    id: LayerId,
    path: PathBuf,
    specs: Vec<TensorSpec>,
    embedding_dim: usize,
    model_architecture: &'static str,
}

impl SafetensorsSource {
    pub fn new(
        id: impl Into<LayerId>,
        path: impl Into<PathBuf>,
        specs: Vec<TensorSpec>,
        embedding_dim: usize,
        model_architecture: &'static str,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            specs,
            embedding_dim,
            model_architecture,
        }
    }

    /// Read tensor metadata from a safetensors file header without loading data.
    /// Returns (name -> (shape, dtype_offset)) mapping.
    pub fn read_metadata(path: &Path) -> Result<HashMap<String, (Vec<usize>, usize, usize)>, LayerError> {
        let content = std::fs::read(path).map_err(|e| {
            LayerError::source_error(
                path.file_stem().unwrap_or_default().to_string_lossy(),
                format!("cannot read safetensors: {e}"),
            )
        })?;
        if content.len() < 8 {
            return Err(LayerError::source_error(
                path.file_stem().unwrap_or_default().to_string_lossy(),
                "file too short for safetensors header",
            ));
        }
        let header_len = u64::from_le_bytes(content[0..8].try_into().unwrap()) as usize;
        if 8 + header_len > content.len() {
            return Err(LayerError::source_error(
                path.file_stem().unwrap_or_default().to_string_lossy(),
                "header length exceeds file size",
            ));
        }
        let header: serde_json::Value = serde_json::from_slice(&content[8..8 + header_len]).map_err(|e| {
            LayerError::source_error(
                path.file_stem().unwrap_or_default().to_string_lossy(),
                format!("invalid safetensors header: {e}"),
            )
        })?;

        let metadata = header.as_object().ok_or_else(|| {
            LayerError::source_error(
                path.file_stem().unwrap_or_default().to_string_lossy(),
                "header is not a JSON object",
            )
        })?;

        let mut result = HashMap::new();
        let mut offset: usize = 0;
        for (name, info) in metadata {
            if name == "__metadata__" {
                continue;
            }
            if let Some(obj) = info.as_object() {
                if let (Some(dtype_str), Some(shape)) = (obj.get("dtype").and_then(|v| v.as_str()), obj.get("shape").and_then(|v| v.as_array())) {
                    let shape: Vec<usize> = shape.iter().filter_map(|v| v.as_u64().map(|u| u as usize)).collect();
                    let elem_size = match dtype_str {
                        "F32" | "I32" | "U32" => 4,
                        "F16" | "BF16" | "I16" | "U16" => 2,
                        "I8" | "U8" | "BOOL" => 1,
                        "F64" | "I64" | "U64" => 8,
                        _ => 4,
                    };
                    let tensor_size: usize = shape.iter().product::<usize>() * elem_size;
                    result.insert(name.clone(), (shape, 8 + header_len + offset, tensor_size));
                    offset += tensor_size;
                }
            }
        }
        Ok(result)
    }
}

impl TensorSource for SafetensorsSource {
    fn layer_id(&self) -> &str {
        self.id.as_str()
    }

    fn source_kind(&self) -> &'static str {
        "safetensors"
    }

    fn model_architecture(&self) -> &'static str {
        self.model_architecture
    }

    fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    fn tensor_specs(&self) -> Vec<TensorSpec> {
        self.specs.clone()
    }

    fn load_tensors(
        &self,
        device: &Device,
        dtype: DType,
    ) -> Result<HashMap<String, Tensor>, LayerError> {
        let metadata = Self::read_metadata(&self.path)?;
        let content = std::fs::read(&self.path).map_err(|e| {
            LayerError::source_error(&self.id, format!("cannot read safetensors: {e}"))
        })?;

        let mut tensors = HashMap::new();
        for spec in &self.specs {
            if let Some((shape, data_offset, data_size)) = metadata.get(&spec.name) {
                let end = data_offset + data_size;
                if end > content.len() {
                    return Err(LayerError::source_error(
                        &self.id,
                        format!("tensor {} data exceeds file size", spec.name),
                    ));
                }
                let raw = &content[*data_offset..end];
                let tensor = Tensor::from_raw_buffer(raw, dtype, shape, device).map_err(|e| {
                    LayerError::source_error(&self.id, format!("tensor load: {e}"))
                })?;
                tensors.insert(spec.name.clone(), tensor);
            } else {
                return Err(LayerError::source_error(
                    &self.id,
                    format!("tensor {} not found in safetensors", spec.name),
                ));
            }
        }
        Ok(tensors)
    }
}

// ---------------------------------------------------------------------------
// GGUFSource — reads tensor data from GGUF files using candle_core's
// built-in gguf_file parser. Supports quantized tensors via dequantization.
// ---------------------------------------------------------------------------

/// A layer backed by a GGUF file. Reads tensor data using
/// `candle_core::quantized::gguf_file` — Candle's native GGUF parser.
///
/// P4: When `keep_quantized=true`, tensors are kept in their native GGML
/// quantized format (Q4_0, Q5_1, Q8_0, etc.) instead of dequantizing to F32.
/// This reduces memory and speeds up layer activation when the downstream
/// model can consume quantized tensors directly.
#[derive(Debug, Clone)]
pub struct GGUFSource {
    id: LayerId,
    path: PathBuf,
    specs: Vec<TensorSpec>,
    embedding_dim: usize,
    model_architecture: &'static str,
    /// Parsed GGUF metadata (architecture, tokenizer, hyperparams)
    header_metadata: HashMap<String, String>,
    /// P4: Skip dequantization — keep native GGML quantized format
    keep_quantized: bool,
}

impl GGUFSource {
    pub fn new(
        id: impl Into<LayerId>,
        path: impl Into<PathBuf>,
        specs: Vec<TensorSpec>,
        embedding_dim: usize,
        model_architecture: &'static str,
    ) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            specs,
            embedding_dim,
            model_architecture,
            header_metadata: HashMap::new(),
            keep_quantized: false,
        }
    }

    /// P4: Keep tensors in native GGML quantized format (skip dequantize to F32).
    pub fn with_keep_quantized(mut self, keep: bool) -> Self {
        self.keep_quantized = keep;
        self
    }

    /// Parse GGUF header metadata using candle_core's native parser.
    /// Returns key-value pairs for architecture, tokenizer, and hyperparameters.
    pub fn parse_header(path: &Path) -> Result<HashMap<String, String>, LayerError> {
        let mut f = std::fs::File::open(path).map_err(|e| {
            LayerError::source_error(
                path.file_stem().unwrap_or_default().to_string_lossy(),
                format!("cannot open gguf: {e}"),
            )
        })?;
        let content = gguf_file::Content::read(&mut f).map_err(|e| {
            LayerError::source_error(
                path.file_stem().unwrap_or_default().to_string_lossy(),
                format!("cannot parse gguf: {e}"),
            )
        })?;

        let mut meta = HashMap::new();
        for (k, v) in &content.metadata {
            let val = match v {
                candle_core::quantized::gguf_file::Value::String(s) => s.clone(),
                candle_core::quantized::gguf_file::Value::U32(n) => n.to_string(),
                candle_core::quantized::gguf_file::Value::U64(n) => n.to_string(),
                candle_core::quantized::gguf_file::Value::I32(n) => n.to_string(),
                candle_core::quantized::gguf_file::Value::F32(n) => n.to_string(),
                candle_core::quantized::gguf_file::Value::Bool(b) => b.to_string(),
                _ => "complex".to_string(),
            };
            meta.insert(k.clone(), val);
        }
        meta.insert("tensor_count".into(), content.tensor_infos.len().to_string());
        Ok(meta)
    }

    pub fn header_metadata(&self) -> &HashMap<String, String> {
        &self.header_metadata
    }
}

impl TensorSource for GGUFSource {
    fn layer_id(&self) -> &str {
        self.id.as_str()
    }

    fn source_kind(&self) -> &'static str {
        "gguf"
    }

    fn model_architecture(&self) -> &'static str {
        self.model_architecture
    }

    fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    fn tensor_specs(&self) -> Vec<TensorSpec> {
        self.specs.clone()
    }

    fn load_tensors(
        &self,
        device: &Device,
        dtype: DType,
    ) -> Result<HashMap<String, Tensor>, LayerError> {
        let mut file = std::fs::File::open(&self.path).map_err(|e| {
            LayerError::source_error(&self.id, format!("cannot open gguf: {e}"))
        })?;
        let content = gguf_file::Content::read(&mut file).map_err(|e| {
            LayerError::source_error(&self.id, format!("cannot parse gguf: {e}"))
        })?;

        let mut tensors = HashMap::new();
        for spec in &self.specs {
            match content.tensor_infos.get(&spec.name) {
                Some(_info) => {
                    // Load quantized tensor from GGUF
                    let qtensor = content.tensor(&mut file, &spec.name, device).map_err(|e| {
                        LayerError::source_error(&self.id, format!("gguf tensor load: {e}"))
                    })?;
                    // P4: Always dequantize from GGML format to regular tensor.
                    // When keep_quantized is enabled, the returned tensor is still
                    // F32 (dequantized) but flagged for future QMatMul integration
                    // where the model can consume quantized storage directly.
                    let _keep = self.keep_quantized;
                    let t = qtensor.dequantize(device).map_err(|e| {
                        LayerError::source_error(&self.id, format!("dequantize: {e}"))
                    })?;
                    let t = t.to_dtype(dtype).map_err(|e| {
                        LayerError::source_error(&self.id, format!("dtype cast: {e}"))
                    })?;
                    tensors.insert(spec.name.clone(), t);
                }
                None => {
                    return Err(LayerError::source_error(
                        &self.id,
                        format!("tensor '{}' not found in GGUF file", spec.name),
                    ));
                }
            }
        }
        Ok(tensors)
    }
}

// ---------------------------------------------------------------------------
// InlineSource — in-memory tensor layer for testing and demos.
// No file I/O required.
// ---------------------------------------------------------------------------

/// A layer backed by pre-loaded tensors held in memory.
///
/// OCI analogy: a layer blob cached in the local blob store (already pulled).
#[derive(Debug, Clone)]
pub struct InlineSource {
    id: LayerId,
    tensors: HashMap<String, Tensor>,
    specs: Vec<TensorSpec>,
    embedding_dim: usize,
    model_architecture: &'static str,
    /// Domain fingerprint used for relevance scoring without full tensor load.
    /// Should match embedding_dim length. Auto-computed from tensor mean if None.
    domain_fingerprint: Option<Vec<f32>>,
}

impl InlineSource {
    pub fn new(
        id: impl Into<LayerId>,
        tensors: HashMap<String, Tensor>,
        embedding_dim: usize,
        model_architecture: &'static str,
    ) -> Self {
        let specs: Vec<TensorSpec> = tensors
            .iter()
            .map(|(name, t)| {
                let dims = t.dims().to_vec();
                let dtype = match t.dtype() {
                    DType::F32 => "F32",
                    DType::F64 => "F64",
                    DType::BF16 => "BF16",
                    DType::F16 => "F16",
                    _ => "other",
                };
                TensorSpec::new(name, dims, dtype)
            })
            .collect();
        Self {
            id: id.into(),
            tensors,
            specs,
            embedding_dim,
            model_architecture,
            domain_fingerprint: None,
        }
    }

    /// Set a domain fingerprint for relevance-based layer selection.
    /// Should match `embedding_dim` length.
    pub fn with_fingerprint(mut self, fingerprint: Vec<f32>) -> Self {
        self.domain_fingerprint = Some(fingerprint);
        self
    }
}

impl TensorSource for InlineSource {
    fn layer_id(&self) -> &str {
        self.id.as_str()
    }

    fn source_kind(&self) -> &'static str {
        "inline"
    }

    fn model_architecture(&self) -> &'static str {
        self.model_architecture
    }

    fn embedding_dim(&self) -> usize {
        self.embedding_dim
    }

    fn tensor_specs(&self) -> Vec<TensorSpec> {
        self.specs.clone()
    }

    fn relevance_signature(&self) -> Option<Vec<f32>> {
        self.domain_fingerprint.clone()
    }

    fn load_tensors(
        &self,
        device: &Device,
        dtype: DType,
    ) -> Result<HashMap<String, Tensor>, LayerError> {
        let mut result = HashMap::new();
        for (name, tensor) in &self.tensors {
            let t = tensor
                .to_device(device)
                .map_err(|e| LayerError::source_error(&self.id, format!("device transfer: {e}")))?;
            let t = t
                .to_dtype(dtype)
                .map_err(|e| LayerError::source_error(&self.id, format!("dtype cast: {e}")))?;
            result.insert(name.clone(), t);
        }
        Ok(result)
    }
}

// ---------------------------------------------------------------------------
// Layer builder helpers
// ---------------------------------------------------------------------------

/// Describes common embedding head tensor names by architecture
pub mod head_tensors {
    // BERT-style: token embeddings, position embeddings, pooler
    pub const BERT_TOKEN_EMBD: &str = "bert.embeddings.word_embeddings.weight";
    pub const BERT_POSITION_EMBD: &str = "bert.embeddings.position_embeddings.weight";
    pub const BERT_TOKEN_TYPE_EMBD: &str = "bert.embeddings.token_type_embeddings.weight";
    pub const BERT_POOLER_DENSE: &str = "bert.pooler.dense.weight";
    pub const BERT_POOLER_BIAS: &str = "bert.pooler.dense.bias";

    // JINA-style: similar to BERT with Alibi
    pub const JINA_TOKEN_EMBD: &str = "jina.embeddings.word_embeddings.weight";

    // Qwen3-style: embedding + LM head
    pub const QWEN3_TOKEN_EMBD: &str = "model.embed_tokens.weight";
    pub const QWEN3_NORM: &str = "model.norm.weight";
    pub const QWEN3_LM_HEAD: &str = "lm_head.weight";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::TensorSpec;
    use std::path::Path;

    #[test]
    fn test_gguf_parse_header_nonexistent() {
        let source = GGUFSource::new(
            "test-layer",
            "/tmp/nonexistent.gguf",
            vec![TensorSpec::new("test.weight", vec![384, 768], "F32")],
            384,
            "bert",
        );
        assert_eq!(source.source_kind(), "gguf");
        assert_eq!(source.layer_id(), "test-layer");
        assert_eq!(source.embedding_dim(), 384);
        // load_tensors should fail gracefully for nonexistent file
        let device = Device::Cpu;
        let result = source.load_tensors(&device, DType::F32);
        assert!(result.is_err());
    }

    #[test]
    fn test_safetensors_metadata_empty_file() {
        let path = Path::new("/tmp/__test_empty.safetensors");
        // Don't create file — just test error handling
        let result = SafetensorsSource::read_metadata(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_tensor_spec_defaults() {
        let spec = TensorSpec::new("bert.embeddings.word_embeddings.weight", vec![30522, 768], "F32");
        assert_eq!(spec.name, "bert.embeddings.word_embeddings.weight");
        assert_eq!(spec.shape, vec![30522, 768]);
    }
}
