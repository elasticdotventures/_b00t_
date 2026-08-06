//! b00t-embed-serve — OpenAI-compatible `/v1/embeddings` HTTP server wrapping
//! b00t-embed's `Qwen3Composable` (real local candle inference, no external API
//! key). This is the "qwen3-embed" local backend `b00t-server`'s soul config
//! (`b00t-mcp/src/server_llm.rs::default_soul()`) discovers on port 8003 — the
//! default embeddings target for MCP servers like `rust-doc` that declare
//! `[b00t.ai_provision] scope = "b00t:EmbeddingModel:execute"`.
//!
//! Unlike `b00t-candle-serve` (one-shot CLI, chat completion), this is a
//! persistent server — one model load at startup, served across every request.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use b00t_embed::{EmbedBackend, qwen3::Qwen3Composable};
use clap::Parser;
use serde::{Deserialize, Serialize};

const MODEL_ID: &str = "Qwen/Qwen3-Embedding-0.6B";
/// The name this server reports itself as / expects in requests — matches
/// rust-doc.mcp.toml's `EMBEDDING_MODEL = "qwen3-embed"` and
/// server_llm.rs's `default_soul()` local-backend entry name.
const MODEL_ALIAS: &str = "qwen3-embed";

#[derive(Parser, Debug)]
#[command(name = "b00t-embed-serve")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 8003)]
    port: u16,
}

struct AppState {
    model: Qwen3Composable,
}

/// OpenAI `/v1/embeddings` request shape. `input` accepts either a single
/// string or a batch — matches what `async-openai` (used by rust-doc's vendor
/// fork) sends.
#[derive(Debug, Deserialize)]
struct EmbeddingsRequest {
    #[serde(deserialize_with = "deserialize_input")]
    input: Vec<String>,
    #[serde(default)]
    #[allow(dead_code)]
    model: Option<String>,
}

fn deserialize_input<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    })
}

#[derive(Debug, Serialize)]
struct EmbeddingsResponse {
    object: &'static str,
    data: Vec<EmbeddingData>,
    model: String,
    usage: Usage,
}

#[derive(Debug, Serialize)]
struct EmbeddingData {
    object: &'static str,
    embedding: Vec<f32>,
    index: usize,
}

#[derive(Debug, Serialize)]
struct Usage {
    prompt_tokens: usize,
    total_tokens: usize,
}

async fn embeddings(
    State(state): State<Arc<AppState>>,
    Json(req): Json<EmbeddingsRequest>,
) -> impl IntoResponse {
    let texts: Vec<&str> = req.input.iter().map(|s| s.as_str()).collect();
    match state.model.embed_batch(&texts).await {
        Ok(embeddings) => {
            let prompt_tokens: usize = req.input.iter().map(|s| s.split_whitespace().count()).sum();
            let data = embeddings
                .into_iter()
                .enumerate()
                .map(|(index, e)| EmbeddingData {
                    object: "embedding",
                    embedding: e.data,
                    index,
                })
                .collect();
            (
                StatusCode::OK,
                Json(EmbeddingsResponse {
                    object: "list",
                    data,
                    model: MODEL_ALIAS.to_string(),
                    usage: Usage {
                        prompt_tokens,
                        total_tokens: prompt_tokens,
                    },
                }),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            // {:#} = anyhow's alternate Display, shows the full `.context()` chain
            // (e.g. "embedding failed: qwen3 forward pass: unsupported dtype...")
            // instead of just the top-level context string.
            Json(serde_json::json!({"error": format!("embedding failed: {e:#}")})),
        )
            .into_response(),
    }
}

async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "object": "list",
        "data": [{
            "id": MODEL_ALIAS,
            "object": "model",
            "owned_by": "b00t",
            "b00t_model_id": state.model.model_id(),
            "embedding_dim": state.model.embedding_dim(),
        }]
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    eprintln!("[b00t-embed-serve] loading {MODEL_ID} (this downloads on first run)...");
    // Qwen3Composable::new is synchronous/blocking (HF download + candle model
    // build) — run it on a blocking thread so it doesn't stall the async runtime
    // during startup.
    let model = tokio::task::spawn_blocking(|| Qwen3Composable::new(MODEL_ID, None, None))
        .await
        .context("model load task panicked")?
        .context("failed to load Qwen3 embedding model")?;
    eprintln!(
        "[b00t-embed-serve] loaded {} ({}-dim)",
        model.model_id(),
        model.embedding_dim()
    );

    let state = Arc::new(AppState { model });
    let app = Router::new()
        .route("/v1/embeddings", post(embeddings))
        .route("/v1/models", get(list_models))
        .with_state(state);

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    eprintln!("[b00t-embed-serve] listening on http://{addr}  (/v1/embeddings, /v1/models)");
    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}
