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
//!
//! Two modes, sharing the same model-load/generate core:
//! - default: one-shot CLI completion, prompt in, tokens streamed to stdout, exit.
//! - `--serve`: persistent OpenAI-compatible `/v1/chat/completions` HTTP server
//!   (axum, same shape as `b00t-embed-serve`'s `/v1/embeddings` layer) — this is
//!   the `candle-phi` local backend `b00t-server`'s soul config
//!   (`b00t-mcp/src/server_llm.rs::default_soul()`) discovers on port 8082.

use std::io::Write as _;

use anyhow::{Context, Result};
use candle_core::{quantized::gguf_file, Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_phi3::ModelWeights;
use clap::Parser;
use tokenizers::Tokenizer;

/// b00t candle-serve — run a quantized Phi-4 GGUF completion (CPU by default), or
/// serve it as an OpenAI-compatible HTTP endpoint with `--serve`.
#[derive(Parser, Debug)]
#[command(name = "b00t-candle-serve")]
struct Args {
    /// Prompt text (one-shot CLI mode only — ignored with --serve)
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

    /// Max new tokens to sample (one-shot CLI mode; --serve takes max_tokens per-request)
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

    /// Load the model once and serve it as an OpenAI-compatible HTTP server
    /// (POST /v1/chat/completions, GET /v1/models) instead of a one-shot completion.
    #[arg(long)]
    serve: bool,

    /// --serve only: bind host
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// --serve only: bind port. Matches the "candle-phi" local backend entry in
    /// b00t-mcp/src/server_llm.rs::default_soul().
    #[arg(long, default_value_t = 8082)]
    port: u16,
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

/// Downloads (or reuses the HF cache of) the GGUF weights + tokenizer, and builds
/// the candle `ModelWeights`. Shared by both the one-shot CLI path and `--serve`.
fn load_model(args: &Args, device: &Device) -> Result<(ModelWeights, Tokenizer)> {
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
    let model = ModelWeights::from_gguf(false, content, &mut file, device)
        .context("building ModelWeights from gguf")?;

    Ok((model, tokenizer))
}

/// Generation parameters shared by both call sites (CLI flags map straight onto
/// this; --serve builds one per HTTP request from the OpenAI-style request body).
struct GenParams {
    sample_len: usize,
    temperature: f64,
    repeat_penalty: f32,
    repeat_last_n: usize,
    seed: u64,
}

/// Runs the autoregressive sampling loop over an already-encoded prompt. Returns
/// the generated token ids (prompt not included) plus the prompt's own token count,
/// so callers can report prompt/completion/total token usage without re-tokenizing.
fn generate_tokens(
    model: &mut ModelWeights,
    tokenizer: &Tokenizer,
    device: &Device,
    prompt: &str,
    params: &GenParams,
) -> Result<(Vec<u32>, usize)> {
    let prompt_tokens = tokenizer
        .encode(prompt, true)
        .map_err(|e| anyhow::anyhow!("tokenizer encode failed: {e}"))?
        .get_ids()
        .to_vec();

    let mut logits_processor = LogitsProcessor::new(params.seed, Some(params.temperature), None);
    let mut all_tokens = prompt_tokens.clone();

    // Prime the model on the full prompt.
    let input = Tensor::new(prompt_tokens.as_slice(), device)?.unsqueeze(0)?;
    let logits = model.forward(&input, 0)?;
    let logits = logits.squeeze(0)?;
    let mut next_token = logits_processor.sample(&logits)?;
    all_tokens.push(next_token);

    // Phi-3/Phi-4's instruct chat template terminates a turn with "<|end|>"; also
    // accept the more generic candle/GPT-style EOS spellings as a fallback for
    // non-instruct-tuned checkpoints.
    let eos_token = tokenizer
        .token_to_id("<|end|>")
        .or_else(|| tokenizer.token_to_id("<|endoftext|>"))
        .or_else(|| tokenizer.token_to_id("</s>"));

    let mut output_ids = vec![next_token];
    for index in 1..params.sample_len {
        if Some(next_token) == eos_token {
            break;
        }
        let input = Tensor::new(&[next_token], device)?.unsqueeze(0)?;
        let logits = model.forward(&input, prompt_tokens.len() + index - 1)?;
        let logits = logits.squeeze(0)?;
        let logits = if params.repeat_penalty == 1.0 {
            logits
        } else {
            let start_at = all_tokens.len().saturating_sub(params.repeat_last_n);
            candle_transformers::utils::apply_repeat_penalty(
                &logits,
                params.repeat_penalty,
                &all_tokens[start_at..],
            )?
        };
        next_token = logits_processor.sample(&logits)?;
        all_tokens.push(next_token);
        if Some(next_token) == eos_token {
            break;
        }
        output_ids.push(next_token);
    }

    Ok((output_ids, prompt_tokens.len()))
}

/// Microsoft's documented Phi-3/Phi-4 instruct chat template:
/// `<|{role}|>\n{content}<|end|>\n` per turn, then a bare `<|assistant|>\n` to cue
/// the completion. See https://huggingface.co/microsoft/Phi-4 model card.
fn build_phi_chat_prompt(messages: &[server::ChatMessage]) -> String {
    let mut prompt = String::new();
    for m in messages {
        let role = match m.role.as_str() {
            "system" => "system",
            "assistant" => "assistant",
            _ => "user",
        };
        prompt.push_str(&format!("<|{role}|>\n{}<|end|>\n", m.content));
    }
    prompt.push_str("<|assistant|>\n");
    prompt
}

/// One-shot CLI path: encode+prime+sample, streaming tokens to stdout as they're
/// generated (nicer interactive UX than the batched --serve response), then report
/// tok/s to stderr — matches the behavior verified in
/// _b00t_/phi-4-candle-local.model.ai.tomllmd's service_contract evidence.
fn run_cli(args: &Args, mut model: ModelWeights, tokenizer: Tokenizer, device: Device) -> Result<()> {
    print!("{}", args.prompt);
    std::io::stdout().flush()?;

    let params = GenParams {
        sample_len: args.sample_len,
        temperature: args.temperature,
        repeat_penalty: args.repeat_penalty,
        repeat_last_n: args.repeat_last_n,
        seed: args.seed,
    };
    let start_gen = std::time::Instant::now();
    let (output_ids, _prompt_tokens) =
        generate_tokens(&mut model, &tokenizer, &device, &args.prompt, &params)?;
    let dt = start_gen.elapsed();

    print!(
        "{}",
        tokenizer.decode(&output_ids, true).unwrap_or_default()
    );
    std::io::stdout().flush()?;

    let generated = output_ids.len();
    println!();
    eprintln!(
        "[b00t-candle-serve] {generated} tokens generated in {:.2}s ({:.2} tok/s)",
        dt.as_secs_f64(),
        generated as f64 / dt.as_secs_f64()
    );
    Ok(())
}

mod server {
    use std::sync::Arc;

    use anyhow::{Context, Result};
    use axum::{
        extract::State,
        http::StatusCode,
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use candle_core::Device;
    use candle_transformers::models::quantized_phi3::ModelWeights;
    use serde::{Deserialize, Serialize};
    use tokenizers::Tokenizer;
    use tokio::sync::Mutex;

    use super::{build_phi_chat_prompt, generate_tokens, GenParams};

    /// The name this server reports itself as / expects in requests — matches
    /// server_llm.rs's `default_soul()` local-backend entry name "candle-phi".
    const MODEL_ALIAS: &str = "candle-phi";

    #[derive(Debug, Clone, Deserialize, Serialize)]
    pub struct ChatMessage {
        pub role: String,
        pub content: String,
    }

    #[derive(Debug, Deserialize)]
    struct ChatCompletionsRequest {
        #[serde(default)]
        #[allow(dead_code)]
        model: Option<String>,
        messages: Vec<ChatMessage>,
        #[serde(default = "default_max_tokens")]
        max_tokens: usize,
        #[serde(default = "default_temperature")]
        temperature: f64,
    }
    fn default_max_tokens() -> usize {
        256
    }
    fn default_temperature() -> f64 {
        0.2
    }

    #[derive(Debug, Serialize)]
    struct ChatCompletionsResponse {
        id: String,
        object: &'static str,
        model: String,
        choices: Vec<ChatChoice>,
        usage: Usage,
    }
    #[derive(Debug, Serialize)]
    struct ChatChoice {
        index: usize,
        message: ChatMessage,
        finish_reason: &'static str,
    }
    #[derive(Debug, Serialize)]
    struct Usage {
        prompt_tokens: usize,
        completion_tokens: usize,
        total_tokens: usize,
    }

    pub struct AppState {
        pub model: Mutex<ModelWeights>,
        pub tokenizer: Tokenizer,
        pub device: Device,
        pub repeat_penalty: f32,
        pub repeat_last_n: usize,
        pub seed: u64,
    }

    async fn chat_completions(
        State(state): State<Arc<AppState>>,
        Json(req): Json<ChatCompletionsRequest>,
    ) -> impl IntoResponse {
        let prompt = build_phi_chat_prompt(&req.messages);
        let params = GenParams {
            sample_len: req.max_tokens,
            temperature: req.temperature,
            repeat_penalty: state.repeat_penalty,
            repeat_last_n: state.repeat_last_n,
            seed: state.seed,
        };

        // Sequential single-model serving: hold the lock for the whole generation
        // (this binary exists to serve one local consumer at a time, not to fan
        // out concurrent requests over one CPU-bound forward pass).
        let mut model = state.model.lock().await;
        match generate_tokens(&mut model, &state.tokenizer, &state.device, &prompt, &params) {
            Ok((output_ids, prompt_tokens)) => {
                let text = state
                    .tokenizer
                    .decode(&output_ids, true)
                    .unwrap_or_default();
                let completion_tokens = output_ids.len();
                (
                    StatusCode::OK,
                    Json(ChatCompletionsResponse {
                        id: format!("candle-phi-{}", uuid_like()),
                        object: "chat.completion",
                        model: MODEL_ALIAS.to_string(),
                        choices: vec![ChatChoice {
                            index: 0,
                            message: ChatMessage {
                                role: "assistant".to_string(),
                                content: text,
                            },
                            finish_reason: "stop",
                        }],
                        usage: Usage {
                            prompt_tokens,
                            completion_tokens,
                            total_tokens: prompt_tokens + completion_tokens,
                        },
                    }),
                )
                    .into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("generation failed: {e:#}")})),
            )
                .into_response(),
        }
    }

    /// Not a real UUID (no extra dependency for it) — timestamp + pid is unique
    /// enough for a request-id-shaped log correlation string.
    fn uuid_like() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        format!("{nanos:x}-{}", std::process::id())
    }

    async fn list_models(State(_state): State<Arc<AppState>>) -> impl IntoResponse {
        Json(serde_json::json!({
            "object": "list",
            "data": [{
                "id": MODEL_ALIAS,
                "object": "model",
                "owned_by": "b00t",
            }]
        }))
    }

    pub async fn run(host: &str, port: u16, state: Arc<AppState>) -> Result<()> {
        let app = Router::new()
            .route("/v1/chat/completions", post(chat_completions))
            .route("/v1/models", get(list_models))
            .with_state(state);

        let addr = format!("{host}:{port}");
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .with_context(|| format!("failed to bind {addr}"))?;
        eprintln!("[b00t-candle-serve] listening on http://{addr}  (/v1/chat/completions, /v1/models)");
        axum::serve(listener, app).await.context("server error")?;
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn chat_completions_request_parses_openai_shape() {
            let body = serde_json::json!({
                "model": "candle-phi",
                "messages": [
                    {"role": "system", "content": "You are terse."},
                    {"role": "user", "content": "2+2?"}
                ],
                "max_tokens": 16,
                "temperature": 0.1,
            });
            let req: ChatCompletionsRequest = serde_json::from_value(body).unwrap();
            assert_eq!(req.messages.len(), 2);
            assert_eq!(req.messages[1].content, "2+2?");
            assert_eq!(req.max_tokens, 16);
        }

        #[test]
        fn chat_completions_request_defaults_max_tokens_and_temperature() {
            let body = serde_json::json!({
                "messages": [{"role": "user", "content": "hi"}]
            });
            let req: ChatCompletionsRequest = serde_json::from_value(body).unwrap();
            assert_eq!(req.max_tokens, 256);
            assert_eq!(req.temperature, 0.2);
        }

        #[test]
        fn chat_completions_response_serializes_openai_shape() {
            let resp = ChatCompletionsResponse {
                id: "candle-phi-1".to_string(),
                object: "chat.completion",
                model: MODEL_ALIAS.to_string(),
                choices: vec![ChatChoice {
                    index: 0,
                    message: ChatMessage {
                        role: "assistant".to_string(),
                        content: "4".to_string(),
                    },
                    finish_reason: "stop",
                }],
                usage: Usage {
                    prompt_tokens: 10,
                    completion_tokens: 1,
                    total_tokens: 11,
                },
            };
            let v = serde_json::to_value(&resp).unwrap();
            assert_eq!(v["choices"][0]["message"]["content"], "4");
            assert_eq!(v["choices"][0]["message"]["role"], "assistant");
            assert_eq!(v["usage"]["total_tokens"], 11);
        }
    }
}

#[test]
fn build_phi_chat_prompt_wraps_each_turn_and_cues_assistant() {
    let messages = vec![
        server::ChatMessage {
            role: "system".to_string(),
            content: "be terse".to_string(),
        },
        server::ChatMessage {
            role: "user".to_string(),
            content: "hi".to_string(),
        },
    ];
    let prompt = build_phi_chat_prompt(&messages);
    assert_eq!(
        prompt,
        "<|system|>\nbe terse<|end|>\n<|user|>\nhi<|end|>\n<|assistant|>\n"
    );
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.print_build_info {
        println!("{}", build_info());
        return Ok(());
    }
    let device = select_device();
    let (model, tokenizer) = load_model(&args, &device)?;

    if args.serve {
        let state = std::sync::Arc::new(server::AppState {
            model: tokio::sync::Mutex::new(model),
            tokenizer,
            device: device.clone(),
            repeat_penalty: args.repeat_penalty,
            repeat_last_n: args.repeat_last_n,
            seed: args.seed,
        });
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to build tokio runtime")?;
        return rt.block_on(server::run(&args.host, args.port, state));
    }

    run_cli(&args, model, tokenizer, device)
}
