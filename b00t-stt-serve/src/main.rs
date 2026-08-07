//! b00t-stt-serve — stable OpenAI-Whisper-compatible `/v1/audio/transcriptions`
//! proxy in front of a containerized [`audio.cpp`](https://github.com/0xShug0/audio.cpp)
//! STT engine (see `stt-baseline.hive.toml`). Unlike `b00t-embed-serve` (which
//! hosts real candle inference in-process), this crate forwards to a separate
//! container — audio.cpp already speaks an OpenAI-Whisper-shaped API
//! (`docs/asr.md` in that repo), so there's no inference to reimplement here.
//! The value this proxy adds: one stable endpoint/model-id for callers,
//! regardless of which underlying model backend (`parakeet_tdt`, `qwen3_asr`,
//! ...) is actually running behind it — more backends are expected over time.

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{
    Json, Router,
    extract::{Multipart, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "b00t-stt-serve")]
struct Args {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    #[arg(long, default_value_t = 8004)]
    port: u16,

    /// Base URL of the containerized audio.cpp engine (internal-only port
    /// per stt-baseline.hive.toml, e.g. http://127.0.0.1:18004).
    #[arg(long, env = "B00T_STT_UPSTREAM", default_value = "http://127.0.0.1:18004")]
    upstream: String,

    /// Model id reported to callers and forwarded upstream when the caller
    /// doesn't specify one — matches the `id` in
    /// stt-baseline-server-config.json's `models[]` entry.
    #[arg(long, default_value = "stt-baseline")]
    model_id: String,
}

struct AppState {
    client: reqwest::Client,
    upstream: String,
    default_model_id: String,
}

/// Forwards a multipart `/v1/audio/transcriptions` request to the audio.cpp
/// engine untouched, except: if the caller omitted `model`, fill in the
/// configured default so callers never need to know the underlying model id.
async fn transcriptions(
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut form = reqwest::multipart::Form::new();
    let mut saw_model = false;

    loop {
        let field = match multipart.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("multipart parsing failed: {e}")})),
                )
                    .into_response();
            }
        };

        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let file_name = field.file_name().unwrap_or("audio").to_string();
            let content_type = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();
            let bytes = match field.bytes().await {
                Ok(b) => b,
                Err(e) => {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": format!("reading file bytes failed: {e}")})),
                    )
                        .into_response();
                }
            };
            let part = reqwest::multipart::Part::bytes(bytes.to_vec())
                .file_name(file_name)
                .mime_str(&content_type)
                .unwrap_or_else(|_| reqwest::multipart::Part::bytes(bytes.to_vec()));
            form = form.part("file", part);
        } else if let Ok(text) = field.text().await {
            if name == "model" {
                saw_model = true;
            }
            form = form.text(name, text);
        }
    }

    if !saw_model {
        form = form.text("model", state.default_model_id.clone());
    }

    let url = format!("{}/v1/audio/transcriptions", state.upstream);
    match state.client.post(&url).multipart(form).send().await {
        Ok(resp) => {
            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::BAD_GATEWAY);
            match resp.bytes().await {
                Ok(body) => (status, body).into_response(),
                Err(e) => (
                    StatusCode::BAD_GATEWAY,
                    Json(serde_json::json!({"error": format!("reading upstream response failed: {e}")})),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({"error": format!("upstream unreachable: {e:#}")})),
        )
            .into_response(),
    }
}

async fn list_models(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    Json(serde_json::json!({
        "object": "list",
        "data": [{
            "id": state.default_model_id,
            "object": "model",
            "owned_by": "b00t",
        }]
    }))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let client = reqwest::Client::builder()
        .build()
        .context("failed to build reqwest client")?;

    eprintln!(
        "[b00t-stt-serve] proxying to {} (model_id={})",
        args.upstream, args.model_id
    );

    let state = Arc::new(AppState {
        client,
        upstream: args.upstream,
        default_model_id: args.model_id,
    });

    let app = Router::new()
        .route("/v1/audio/transcriptions", post(transcriptions))
        .route("/v1/models", get(list_models))
        .with_state(state);

    let addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("failed to bind {addr}"))?;
    eprintln!(
        "[b00t-stt-serve] listening on http://{addr}  (/v1/audio/transcriptions, /v1/models)"
    );
    axum::serve(listener, app).await.context("server error")?;
    Ok(())
}
