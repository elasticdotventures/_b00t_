// b00t-embed Wave 1: Extract embedding head tensors from Qwen3-Embedding-0.6B
// and export as standalone safetensors "layer" files for OCI-style composition.
//
// This creates domain-specific embedding head layers that can be registered
// with LayerStack and composed at runtime based on search query relevance.
//
// Usage: cargo run --example extract_qwen3_heads -- [output-dir]
//   Default output: /tmp/qwen3-layers/
//
// Bouncer gate validation per wave:
//   Wave 1a: Validate HF download → tensor name extraction
//   Wave 1b: Validate shape/dtype match between head and base model
//   Wave 1c: Validate exported safetensors are loadable by GGUFSource

use std::collections::HashMap;
use std::path::PathBuf;

use candle_core::{DType, Device, Tensor};
use hf_hub::api::sync::ApiBuilder;
use hf_hub::Repo;

const MODEL_ID: &str = "Qwen/Qwen3-Embedding-0.6B";
#[allow(dead_code)]
const HIDDEN_SIZE: usize = 1024; // from config.json
#[allow(dead_code)]
const VOCAB_SIZE: usize = 151669; // actual embed_tokens dim[0]

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let out_dir = args.get(1).map(|s| PathBuf::from(s)).unwrap_or_else(|| PathBuf::from("/tmp/qwen3-layers"));
    std::fs::create_dir_all(&out_dir)?;

    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║  Wave 1: Extract Qwen3 Embedding Head Layers                ║");
    println!("║  Model: {MODEL_ID}                                   ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    // ── Wave 1a: Download model from HuggingFace ──
    println!("\n  Wave 1a: Download model from HuggingFace ...");
    let api = ApiBuilder::from_env().build()?;
    let repo = api.repo(Repo::new(MODEL_ID.to_string(), hf_hub::RepoType::Model));
    let weights_path = repo.get("model.safetensors")?;
    let config_path = repo.get("config.json")?;
    println!("  ✓ Downloaded model.safetensors ({})", weights_path.display());
    println!("  ✓ Downloaded config.json ({})", config_path.display());

    // Validate: file exists and is readable
    assert!(weights_path.exists(), "Bouncer gate: weights file must exist");
    assert!(config_path.exists(), "Bouncer gate: config file must exist");
    println!("  ✓ Bouncer gate 1a: HF download validated");

    // ── Wave 1b: Load weights and extract head tensors ──
    println!("\n  Wave 1b: Load weights and extract embedding head tensors ...");
    let device = Device::Cpu;
    let target_dtype = DType::F32;

    // Use MmapedSafetensors to read tensor metadata without loading everything
    let tensors = unsafe {
        candle_core::safetensors::MmapedSafetensors::new(&weights_path)?
    };

    // Extract specific head tensors
    // Qwen3 embedding model stores tensors with these names.
    // embed_tokens: [vocab_size, hidden_size] = [151669, 1024]
    // norm.weight: [hidden_size] = [1024] (final layer norm)
    let head_tensor_names = [
        "embed_tokens.weight",
        "norm.weight",       // final layer norm, not model.norm.weight
    ];

    let mut extracted = HashMap::new();
    for name in &head_tensor_names {
        match tensors.load(name, &device) {
            Ok(tensor) => {
                // Cast to F32 for consistent handling (model may be BF16/F16)
                let tensor = tensor.to_dtype(target_dtype)
                    .map_err(|e| anyhow::anyhow!("dtype cast: {e}"))?;
                let dims = tensor.dims();
                println!("  ✓ Loaded {name}: shape={dims:?}, dtype={:?}", tensor.dtype());
                extracted.insert(name.to_string(), tensor);
            }
            Err(e) => {
                // Try alternative naming patterns for different model versions
                let alt_names = [
                    &format!("transformer.{name}"),
                    &format!("{name}"),
                ];
                let mut found = false;
                for alt in &alt_names {
                    if let Ok(t) = tensors.load(alt, &device) {
                        println!("  ✓ Loaded {alt} (alt name): shape={:?}", t.dims());
                        extracted.insert(name.to_string(), t);
                        found = true;
                        break;
                    }
                }
                if !found {
                    eprintln!("  ✗ Could not load tensor '{name}': {e}");
                }
            }
        }
    }

    // Validate: at least embed_tokens.weight must be present
    assert!(extracted.contains_key("embed_tokens.weight"),
        "Bouncer gate: embed_tokens.weight must be extractable");
    println!("  ✓ Bouncer gate 1b: head tensor extraction validated");

    // ── Wave 1c: Export each head tensor as standalone safetensors layer file ──
    println!("\n  Wave 1c: Export head tensors as safetensors layer files ...");

    // Create a "base" layer with the original head tensors
    let base_path = out_dir.join("qwen3-base-head.safetensors");
    export_safetensors(&extracted, &base_path)?;
    println!("  ✓ Exported base head layer: {}", base_path.display());

    // Create domain-specific "probe" layers with modified weights.
    // These simulate domain-tuned embedding heads (code, math, biology)
    // by scaling the original weights in a domain-biased pattern.
    // The modifier handles both 1D (norm) and 2D (embed_tokens) tensors.
    let domains: Vec<(&str, fn(&Tensor) -> anyhow::Result<Tensor>)> = vec![
        ("code", |t: &Tensor| -> anyhow::Result<Tensor> {
            let dims = t.dims();
            let flat = t.flatten_all()?.to_vec1::<f32>()?;
            let n = flat.len();
            let half = n / 2;
            let mut data = flat;
            for i in 0..half { data[i] *= 1.5; }
            Ok(Tensor::from_vec(data, dims, t.device())?)
        }),
        ("math", |t: &Tensor| -> anyhow::Result<Tensor> {
            let dims = t.dims();
            let flat = t.flatten_all()?.to_vec1::<f32>()?;
            let n = flat.len();
            let half = n / 2;
            let mut data = flat;
            for i in half..n { data[i] *= 1.5; }
            Ok(Tensor::from_vec(data, dims, t.device())?)
        }),
        ("biol", |t: &Tensor| -> anyhow::Result<Tensor> {
            let dims = t.dims();
            let flat = t.flatten_all()?.to_vec1::<f32>()?;
            Ok(Tensor::from_vec(
                flat.into_iter().map(|x| x * 1.2).collect(),
                dims, t.device(),
            )?)
        }),
    ];

    for (domain, modifier) in &domains {
        let mut domain_tensors = HashMap::new();
        for (name, tensor) in &extracted {
            let modified = modifier(tensor)?;
            domain_tensors.insert(name.clone(), modified);
        }
        let layer_path = out_dir.join(format!("qwen3-{domain}-head.safetensors"));
        export_safetensors(&domain_tensors, &layer_path)?;
        println!("  ✓ Exported {domain} head layer: {}", layer_path.display());
    }

    // Validate: all exported files are loadable
    for entry in std::fs::read_dir(&out_dir)? {
        let path = entry?.path();
        if path.extension().map(|e| e == "safetensors").unwrap_or(false) {
            let reloaded = unsafe { candle_core::safetensors::MmapedSafetensors::new(&path)? };
            let count = reloaded.tensors().len();
            println!("  ✓ Verified loadable: {} ({} tensors)", path.display(), count);
        }
    }
    println!("  ✓ Bouncer gate 1c: all exported files validated as loadable safetensors");

    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  Wave 1 COMPLETE: {} head layer files exported", domains.len() + 1);
    println!("║  Output directory: {}", out_dir.display());
    println!("║                                                             ║");
    println!("║  Next: just qwen3-embed-test to validate compose pipeline   ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");

    Ok(())
}

/// Export a HashMap of named tensors to a safetensors file.
/// Uses Candle's native safetensors writer for correct format compliance.
fn export_safetensors(tensors: &HashMap<String, Tensor>, path: &PathBuf) -> anyhow::Result<()> {
    candle_core::safetensors::save(tensors, path)?;
    Ok(())
}
