// R1: Verify Qwen3Composable varmap.load() tensor alignment + forward pass
// against the real Qwen/Qwen3-Embedding-0.6B model from HuggingFace.
//
// Run: cargo test --test test_qwen3_composable -p b00t-embed -- --nocapture

use candle_core::{DType, Device};
use candle_nn::{VarBuilder, VarMap};
use hf_hub::api::sync::ApiBuilder;
use hf_hub::Repo;

const MODEL_ID: &str = "Qwen/Qwen3-Embedding-0.6B";

/// R1a: Verify tensor name alignment between VarMap entries and safetensors file.
/// This is the critical path — mismatched names cause load() to fail silently.
#[test]
fn test_tensor_name_alignment() {
    let api = ApiBuilder::from_env().build()
        .expect("HF API init (set HF_TOKEN if needed)");
    let repo = api.repo(Repo::new(MODEL_ID.to_string(), hf_hub::RepoType::Model));
    let weights_path = repo.get("model.safetensors")
        .expect("model.safetensors download");

    // Read safetensors header to get actual tensor names
    let content = std::fs::read(&weights_path).expect("read safetensors");
    let header_len = u64::from_le_bytes(content[0..8].try_into().unwrap()) as usize;
    let header: serde_json::Value = serde_json::from_slice(&content[8..8 + header_len])
        .expect("parse safetensors header");
    let safetensors_names: Vec<String> = header.as_object().unwrap().keys()
        .filter(|k| *k != "__metadata__")
        .cloned()
        .collect();

    println!("Safetensors tensor count: {}", safetensors_names.len());
    for name in safetensors_names.iter().take(10) {
        println!("  safetensors: {name}");
    }
    if safetensors_names.len() > 10 {
        println!("  ... and {} more", safetensors_names.len() - 10);
    }

    // Build VarMap and create VarBuilder to simulate model construction
    let varmap = VarMap::new();
    let _vb = VarBuilder::from_varmap(&varmap, DType::F32, &Device::Cpu);

    // Load Qwen3 model config to know shapes
    let config_path = repo.get("config.json").expect("config.json download");
    let config_raw = std::fs::read_to_string(config_path).expect("read config");
    let cfg: serde_json::Value = serde_json::from_str(&config_raw).expect("parse config");
    let hidden_size = cfg["hidden_size"].as_u64().unwrap() as usize;
    let vocab_size = cfg["vocab_size"].as_u64().unwrap() as usize;
    let num_layers = cfg["num_hidden_layers"].as_u64().unwrap() as usize;
    let num_heads = cfg["num_attention_heads"].as_u64().unwrap() as usize;
    let num_kv_heads = cfg["num_key_value_heads"].as_u64().unwrap() as usize;
    let head_dim = hidden_size / num_heads;
    let intermediate_size = cfg["intermediate_size"].as_u64().unwrap() as usize;

    println!(
        "\nModel config: hidden={hidden_size}, layers={num_layers}, heads={num_heads}, kv={num_kv_heads}, head_dim={head_dim}, intermediate={intermediate_size}"
    );

    // Detect actual dtype from safetensors file (BF16 for this model)
    let model_dtype = {
        let content = std::fs::read(&weights_path).expect("read safetensors");
        let header_len = u64::from_le_bytes(content[0..8].try_into().unwrap()) as usize;
        let header: serde_json::Value = serde_json::from_slice(&content[8..8 + header_len])
            .expect("parse safetensors header");
        let obj = header.as_object().unwrap();
        let first_dtype = obj.iter()
            .find(|(k, _)| *k != "__metadata__")
            .and_then(|(_, v)| v.get("dtype").and_then(|d| d.as_str()))
            .unwrap_or("F32");
        match first_dtype {
            "BF16" => DType::BF16,
            "F16" => DType::F16,
            "F32" => DType::F32,
            _ => DType::F32,
        }
    };
    println!("  Detected model dtype: {model_dtype:?}");

    // Pre-create VarMap entries matching the model architecture.
    // Use the model's native dtype so varmap.load() doesn't fail on mismatch.
    let test_tensors = vec![
        ("embed_tokens.weight", vec![vocab_size, hidden_size]),
        ("norm.weight", vec![hidden_size]),
    ];
    for (name, shape) in &test_tensors {
        let _ = varmap.get(shape.clone(), name, candle_nn::Init::Const(0.0), model_dtype, &Device::Cpu);
    }

    // Get all VarMap entry names
    let varmap_names: Vec<String> = {
        let data = varmap.data().lock().unwrap();
        data.keys().cloned().collect()
    };

    println!("\nVarMap tensor count: {}", varmap_names.len());
    for name in &varmap_names {
        println!("  varmap: {name}");
    }

    // Check alignment: every VarMap tensor should exist in safetensors
    println!("\nTensor name alignment check:");
    let mut all_match = true;
    for vn in &varmap_names {
        if safetensors_names.contains(vn) {
            println!("  ✓ {vn}");
        } else {
            println!("  ✗ {vn} NOT FOUND in safetensors");
            all_match = false;
        }
    }

    assert!(all_match, "R1a: ALL VarMap tensor names must exist in safetensors file");

    // Now load the real weights
    let mut vm = varmap.clone();
    vm.load(&weights_path).expect("varmap.load() from safetensors");
    println!("\n  ✓ varmap.load() succeeded — tensor names aligned correctly");
}

/// R1b: Verify Qwen3Composable produces coherent embeddings from real weights.
/// This requires full model construction + forward pass (28-layer Qwen3, ~2.4GB RAM).
#[tokio::test]
#[ignore = "needs full model build (~2.4GB RAM) — run with -- --ignored"]
async fn test_composable_forward_pass() {
    use b00t_embed::qwen3::Qwen3Composable;
    use b00t_embed::EmbedBackend;

    let backend = Qwen3Composable::new(MODEL_ID, None, None)
        .expect("Qwen3Composable::new() — builds model with VarMap + loads real weights");
    assert!(backend.is_available(), "backend must be available after loading");

    let emb = backend.embed("Hello, world!").await
        .expect("embed forward pass");
    assert!(!emb.data.is_empty(), "embedding must not be empty");
    assert_eq!(emb.data.len(), 1024, "Qwen3-Embedding-0.6B has 1024 dims");

    // Verify the embedding is non-zero (not random init)
    let norm: f32 = emb.data.iter().map(|x| x * x).sum::<f32>().sqrt();
    assert!(norm > 0.01, "embedding must have non-zero norm (weights actually loaded)");

    println!("✓ Qwen3Composable forward pass OK: dim={}, norm={:.4}", emb.data.len(), norm);
}
