//! b00t-candle-serve — minimal CPU-friendly runner for quantized GGUF models via
//! huggingface/candle's `candle-transformers`. First (and currently only) target:
//! Microsoft Phi-4 (14B, MIT license) at Q4_K quantization.
//!
//! This intentionally does NOT reimplement a Phi forward pass — it drives
//! `candle_transformers::models::quantized_phi3::ModelWeights`, which candle already
//! ships and which upstream's own `quantized-phi` example uses for the Phi-4 variant
//! (Phi-4 shares the phi3 GGUF architecture tag). See _b00t_/phi-4-candle-local.model.ai.tomllmd
//! and _b00t_/phi.ai.tomllmd for the b00t datum wiring.
//!
//! CPU-only by design — the RTX 3090 on this box is time-shared/contended and is
//! already spoken for by ch0nky/vLLM; this binary exists specifically as the
//! no-GPU-required fallback.

use std::io::Write;

use anyhow::{Context, Result};
use candle_core::{quantized::gguf_file, Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_phi3::ModelWeights;
use clap::Parser;
use tokenizers::Tokenizer;

/// b00t candle-serve — run a quantized Phi-4 GGUF completion (CPU, no server yet)
#[derive(Parser, Debug)]
#[command(name = "b00t-candle-serve")]
struct Args {
    /// Prompt text
    #[arg(long, default_value = "Q: What is the capital of France?\nA:")]
    prompt: String,

    /// HF repo hosting the GGUF file
    #[arg(long, default_value = "microsoft/phi-4-gguf")]
    hf_repo: String,

    /// GGUF filename within hf_repo
    #[arg(long, default_value = "phi-4-Q4_K.gguf")]
    gguf_file: String,

    /// HF repo hosting tokenizer.json (Phi-4 GGUF repo ships weights only, not the tokenizer)
    #[arg(long, default_value = "microsoft/phi-4")]
    tokenizer_repo: String,

    /// Max new tokens to sample
    #[arg(long, default_value_t = 64)]
    sample_len: usize,

    #[arg(long, default_value_t = 0.2)]
    temperature: f64,

    #[arg(long, default_value_t = 1.1)]
    repeat_penalty: f32,

    #[arg(long, default_value_t = 64)]
    repeat_last_n: usize,

    #[arg(long, default_value_t = 299792458)]
    seed: u64,

    /// Print which device this binary would use (cuda/cpu) and exit immediately —
    /// no download, no model load. Used by `b00t whoami`'s dashboard to report
    /// candle CUDA capability without paying the multi-GB download/load cost.
    #[arg(long)]
    print_build_info: bool,
}

/// Picks the compute device: CUDA if this binary was built with the `cuda` feature
/// AND a CUDA device is actually available at runtime (falls back to CPU on any
/// failure — a stale driver, a busy/reserved GPU, etc — rather than hard-erroring).
/// Without the `cuda` feature, this is CPU always; the code path doesn't exist in
/// that build, so there's nothing to fall back from.
fn select_device() -> Device {
    #[cfg(feature = "cuda")]
    {
        match Device::new_cuda(0) {
            Ok(d) => {
                eprintln!("[b00t-candle-serve] device: cuda:0");
                return d;
            }
            Err(e) => {
                eprintln!("[b00t-candle-serve] cuda requested but unavailable ({e}), falling back to cpu");
            }
        }
    }
    eprintln!("[b00t-candle-serve] device: cpu");
    Device::Cpu
}

/// Build identifier for `--print-build-info`: "cuda" if compiled with the cuda
/// feature (regardless of whether a GPU is present right now — this reflects what
/// the BINARY can do, runtime availability is select_device()'s job), else "cpu".
fn build_info() -> &'static str {
    if cfg!(feature = "cuda") {
        "cuda"
    } else {
        "cpu"
    }
}

fn split_repo_id(id: &str) -> (&str, &str) {
    match id.split_once('/') {
        Some((owner, name)) => (owner, name),
        None => ("", id),
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.print_build_info {
        println!("{}", build_info());
        return Ok(());
    }
    let device = select_device();

    // hf-hub 1.0's blocking API: HFClientSync -> .model(owner, name) -> .download_file()
    // builder (matches the pattern already used in b00t-embed/src/qwen3.rs).
    let client = hf_hub::HFClientSync::new().context("failed to build HF client")?;

    eprintln!("[b00t-candle-serve] fetching {} / {}", args.hf_repo, args.gguf_file);
    let (owner, name) = split_repo_id(&args.hf_repo);
    let repo = client.model(owner, name);
    let model_path = repo
        .download_file()
        .filename(args.gguf_file.clone())
        .send()
        .with_context(|| format!("downloading {} from {}", args.gguf_file, args.hf_repo))?;

    eprintln!("[b00t-candle-serve] fetching tokenizer from {}", args.tokenizer_repo);
    let (tok_owner, tok_name) = split_repo_id(&args.tokenizer_repo);
    let tokenizer_path = client
        .model(tok_owner, tok_name)
        .download_file()
        .filename("tokenizer.json")
        .send()
        .context("downloading tokenizer.json")?;
    let tokenizer = Tokenizer::from_file(tokenizer_path)
        .map_err(|e| anyhow::anyhow!("tokenizer load failed: {e}"))?;

    eprintln!("[b00t-candle-serve] loading GGUF weights from {}", model_path.display());
    let mut file = std::fs::File::open(&model_path)?;
    let content = gguf_file::Content::read(&mut file)
        .with_context(|| format!("invalid gguf file at {}", model_path.display()))?;
    let mut total_size_in_bytes = 0;
    for (_, tensor) in content.tensor_infos.iter() {
        total_size_in_bytes +=
            tensor.shape.elem_count() * tensor.ggml_dtype.type_size() / tensor.ggml_dtype.block_size();
    }
    eprintln!(
        "[b00t-candle-serve] loaded {} tensors ({:.2} GB)",
        content.tensor_infos.len(),
        total_size_in_bytes as f64 / 1e9
    );

    // Phi-4 reuses the "phi3" GGUF architecture tag (confirmed via candle's own
    // quantized-phi example: `Which::Phi3 | Which::Phi4 => Model::Phi3(...)`).
    let mut model = ModelWeights::from_gguf(false, content, &mut file, &device)
        .context("building ModelWeights from gguf")?;

    let prompt_tokens = tokenizer
        .encode(args.prompt.as_str(), true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode failed: {e}"))?
        .get_ids()
        .to_vec();

    print!("{}", args.prompt);
    std::io::stdout().flush()?;

    let mut logits_processor =
        LogitsProcessor::new(args.seed, Some(args.temperature), None);
    let mut all_tokens = prompt_tokens.clone();
    let start_gen = std::time::Instant::now();

    // Prime the model on the full prompt.
    let input = Tensor::new(prompt_tokens.as_slice(), &device)?.unsqueeze(0)?;
    let logits = model.forward(&input, 0)?;
    let logits = logits.squeeze(0)?;
    let mut next_token = logits_processor.sample(&logits)?;
    all_tokens.push(next_token);
    print!("{}", tokenizer.decode(&[next_token], true).unwrap_or_default());
    std::io::stdout().flush()?;

    let eos_token = tokenizer
        .token_to_id("<|endoftext|>")
        .or_else(|| tokenizer.token_to_id("</s>"));

    let mut generated = 1usize;
    for index in 1..args.sample_len {
        if Some(next_token) == eos_token {
            break;
        }
        let input = Tensor::new(&[next_token], &device)?.unsqueeze(0)?;
        let logits = model.forward(&input, prompt_tokens.len() + index - 1)?;
        let logits = logits.squeeze(0)?;
        let logits = if args.repeat_penalty == 1.0 {
            logits
        } else {
            let start_at = all_tokens.len().saturating_sub(args.repeat_last_n);
            candle_transformers::utils::apply_repeat_penalty(
                &logits,
                args.repeat_penalty,
                &all_tokens[start_at..],
            )?
        };
        next_token = logits_processor.sample(&logits)?;
        all_tokens.push(next_token);
        print!("{}", tokenizer.decode(&[next_token], true).unwrap_or_default());
        std::io::stdout().flush()?;
        generated += 1;
    }
    let dt = start_gen.elapsed();
    println!();
    eprintln!(
        "[b00t-candle-serve] {generated} tokens generated in {:.2}s ({:.2} tok/s)",
        dt.as_secs_f64(),
        generated as f64 / dt.as_secs_f64()
    );
    Ok(())
}
