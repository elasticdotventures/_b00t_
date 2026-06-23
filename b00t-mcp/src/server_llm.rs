// 🤓 b00t-server: OpenAI-compatible REST API router — transparent proxy
//    Validates b00t-issued API keys → forwards to upstream LLM backend →
//    emits Spotlight usage events. Mounts on the existing b00t-mcp axum server.
//
//    Auto-discovery: probes local backends (mistralrs :8181, llama.cpp :8080,
//    vLLM :8000) on startup. Falls back to remote API keys from env.
//    Reference: vendor/irontology-mcp/crates/provider-openai/src/lib.rs:299-354

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, Method},
    response::{IntoResponse, Json},
    routing::{get, post},
};
use axum::http::header;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

// ── Auto-discovery of upstream backends ────────────────────────────────────

const LOCAL_BACKENDS: &[(&str, u16)] = &[
    ("mistral.rs", 8181),
    ("llama.cpp", 8080),
    ("vLLM", 8000),
    ("vLLM-alt", 8001),
];

/// Probe known local ports via TCP connect (no runtime needed).
/// Returns (name, base_url) of the first live listener.
fn discover_local_backend() -> Option<(String, String)> {
    for &(name, port) in LOCAL_BACKENDS {
        if TcpStream::connect_timeout(
            &format!("127.0.0.1:{}", port).parse().ok()?,
            Duration::from_millis(500),
        ).is_ok()
        {
            eprintln!("🔍 discovered local backend: {} (port {})", name, port);
            return Some((name.to_string(), format!("http://127.0.0.1:{}/v1", port)));
        }
    }
    None
}

/// Resolve the upstream key from environment in priority order.
fn resolve_upstream_key() -> Option<(String, String)> {
    // Explicit server config
    if let Ok(key) = std::env::var("B00T_SERVER_UPSTREAM_KEY") {
        if !key.is_empty() {
            return Some((key, "env:B00T_SERVER_UPSTREAM_KEY".into()));
        }
    }
    // Standard OpenAI
    if let Ok(key) = std::env::var("OPENAI_API_KEY") {
        if !key.is_empty() {
            return Some((key, "env:OPENAI_API_KEY".into()));
        }
    }
    // b00t tier keys (for forwarding)
    if let Ok(key) = std::env::var("B00T_AI_CH0NKY_KEY") {
        if !key.is_empty() {
            if let Ok(url) = std::env::var("B00T_AI_CH0NKY_BASE") {
                if !url.is_empty() {
                    return Some((key, url));
                }
            }
        }
    }
    if let Ok(key) = std::env::var("B00T_AI_FRONTIER_KEY") {
        if !key.is_empty() {
            if let Ok(url) = std::env::var("B00T_AI_FRONTIER_BASE") {
                if !url.is_empty() {
                    return Some((key, url));
                }
            }
        }
    }
    // OpenRouter
    if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
        if !key.is_empty() {
            return Some((key, "https://openrouter.ai/api/v1".into()));
        }
    }
    None
}

/// Resolve upstream: explicit env var > local auto-discovery > remote key.
fn resolve_upstream() -> (String, String) {
    // 1. Explicit URL from env (takes precedence)
    if let Ok(url) = std::env::var("B00T_SERVER_UPSTREAM_URL") {
        if !url.is_empty() {
            let key = std::env::var("B00T_SERVER_UPSTREAM_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .unwrap_or_default();
            eprintln!("🌐 upstream (explicit): {}", url);
            return (url, key);
        }
    }

    // 2. Auto-discover local backend
    if let Some((name, url)) = discover_local_backend() {
        eprintln!("📍 upstream (auto-discovered {}): {}", name, url);
        return (url, String::new()); // local backends don't need keys
    }

    // 3. Remote key profile
    if let Some((key, source)) = resolve_upstream_key() {
        let url = if source.starts_with("http") {
            source.clone()
        } else {
            "https://api.openai.com/v1".to_string()
        };
        eprintln!("🌐 upstream (remote {}): {}", source, url);
        return (url, key);
    }

    // 4. Nothing available — start in degraded mode
    eprintln!("⚠️  No upstream configured. Set B00T_SERVER_UPSTREAM_KEY or OPENAI_API_KEY.");
    eprintln!("   Also probes: localhost:{:?} for mistral.rs/llama.cpp/vLLM.", LOCAL_BACKENDS.iter().map(|(_,p)| p).collect::<Vec<_>>());
    ("http://localhost:8181/v1".to_string(), String::new())
}

// ── State ──────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct KeyEntry {
    pub consumer: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub struct LlmState {
    pub upstream_url: String,
    pub upstream_key: String,
    pub keys: Arc<RwLock<HashMap<String, KeyEntry>>>,
    pub keys_file: std::path::PathBuf,
    pub spotlight_log: std::path::PathBuf,
}

impl LlmState {
    /// Create state with auto-discovered upstream: local probe > env key > explicit URL.
    pub fn new() -> Self {
        let (url, key) = resolve_upstream();
        Self::from_config(&url, &key)
    }

    /// Create state with explicit upstream config (for tests, embedded use).
    pub fn from_config(upstream_url: &str, upstream_key: &str) -> Self {
        let home = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));
        let keys_file = home.join("server-keys.json");
        let mut keys = HashMap::new();

        if let Ok(data) = std::fs::read_to_string(&keys_file) {
            if let Ok(parsed) = serde_json::from_str::<Value>(&data) {
                if let Some(obj) = parsed.get("keys").and_then(|k| k.as_object()) {
                    for (k, v) in obj {
                        if let (Some(consumer), Some(created_at)) = (
                            v.get("consumer").and_then(|c| c.as_str()),
                            v.get("created_at").and_then(|c| c.as_str()),
                        ) {
                            if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(created_at) {
                                keys.insert(k.clone(), KeyEntry {
                                    consumer: consumer.to_string(),
                                    created_at: ts.with_timezone(&chrono::Utc),
                                });
                            }
                        }
                    }
                }
            }
        }

        Self {
            upstream_url: upstream_url.trim_end_matches('/').to_string(),
            upstream_key: upstream_key.to_string(),
            keys: Arc::new(RwLock::new(keys)),
            keys_file,
            spotlight_log: home.join("spotlight.jsonl"),
        }
    }

    pub async fn validate_key(&self, token: &str) -> Option<KeyEntry> {
        self.keys.read().await.get(token).cloned()
    }

    pub async fn create_key(&self, consumer: &str) -> String {
        let key = format!("b00t-sk-{}", Uuid::new_v4().simple());
        self.keys.write().await.insert(key.clone(), KeyEntry {
            consumer: consumer.to_string(),
            created_at: chrono::Utc::now(),
        });
        self.save_keys_to_file().await;
        key
    }

    async fn save_keys_to_file(&self) {
        let keys = self.keys.read().await;
        let mut map = serde_json::Map::new();
        for (k, v) in keys.iter() {
            map.insert(k.clone(), json!({
                "consumer": v.consumer,
                "created_at": v.created_at.to_rfc3339(),
            }));
        }
        let data = json!({"keys": map});
        if let Some(parent) = self.keys_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&self.keys_file, data.to_string());
    }

    async fn emit_spotlight(&self, consumer: &str, endpoint: &str, model: &str, latency_ms: u64) {
        let event = json!({
            "ts": chrono::Utc::now().to_rfc3339(),
            "event": format!("spotlight.llm.{}", endpoint),
            "consumer": consumer,
            "model": model,
            "latency_ms": latency_ms,
        });
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.spotlight_log)
        {
            use std::io::Write;
            let _ = writeln!(f, "{}", event);
        }
    }
}

fn dirs_next() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join(".b00t"))
}

// ── Router ─────────────────────────────────────────────────────────────────

pub fn llm_router(state: Arc<LlmState>, dev_mode: bool) -> Router {
    Router::new()
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(proxy_chat))
        .route("/v1/embeddings", post(proxy_embeddings))
        .route("/v1/{*_}", axum::routing::any(fallback_not_found))
        .with_state((state, dev_mode))
}

// ── Key checking middleware ────────────────────────────────────────────────

type AppState = (Arc<LlmState>, bool);

fn extract_bearer_token(headers: &HeaderMap, dev_mode: bool) -> Option<String> {
    if dev_mode {
        return Some("dev-key".to_string());
    }
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

// ── Handlers ───────────────────────────────────────────────────────────────

async fn list_models(
    State((state, dev_mode)): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = extract_bearer_token(&headers, dev_mode).unwrap_or_default();
    let consumer = state
        .validate_key(&token)
        .await
        .map(|k| k.consumer)
        .unwrap_or_else(|| "unknown".to_string());

    let url = format!("{}/models", state.upstream_url);
    let client = reqwest::Client::new();
    let mut req = client.get(&url);
    if !state.upstream_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", state.upstream_key));
    }

    let start = std::time::Instant::now();
    match req.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = resp.status();
            let body = resp.bytes().await.unwrap_or_default();
            state.emit_spotlight(&consumer, "models", "*", latency).await;
            (status, body).into_response()
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            state.emit_spotlight(&consumer, "models", "*", latency).await;
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("upstream unreachable: {}", e)})),
            ).into_response()
        }
    }
}

async fn proxy_chat(
    State((state, dev_mode)): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let token = extract_bearer_token(&headers, dev_mode).unwrap_or_default();
    let consumer = state
        .validate_key(&token)
        .await
        .map(|k| k.consumer)
        .unwrap_or_else(|| "unknown".to_string());

    let model = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str().map(|s| s.to_string())))
        .unwrap_or_else(|| "unknown".to_string());

    let url = format!("{}/chat/completions", state.upstream_url);
    let client = reqwest::Client::new();
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body.clone());
    if !state.upstream_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", state.upstream_key));
    }

    // Forward select headers (strip client auth, keep content-type)
    if let Some(ct) = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        req = req.header("Content-Type", ct);
    }

    let start = std::time::Instant::now();
    match req.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = resp.status();
            let resp_body = resp.bytes().await.unwrap_or_default();
            state.emit_spotlight(&consumer, "chat_completions", &model, latency).await;
            (status, resp_body).into_response()
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            state.emit_spotlight(&consumer, "chat_completions", &model, latency).await;
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("upstream unreachable: {}", e)})),
            ).into_response()
        }
    }
}

async fn proxy_embeddings(
    State((state, dev_mode)): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let token = extract_bearer_token(&headers, dev_mode).unwrap_or_default();
    let consumer = state
        .validate_key(&token)
        .await
        .map(|k| k.consumer)
        .unwrap_or_else(|| "unknown".to_string());

    let model = serde_json::from_slice::<Value>(&body)
        .ok()
        .and_then(|v| v.get("model").and_then(|m| m.as_str().map(|s| s.to_string())))
        .unwrap_or_else(|| "unknown".to_string());

    let url = format!("{}/embeddings", state.upstream_url);
    let client = reqwest::Client::new();
    let mut req = client
        .post(&url)
        .header("Content-Type", "application/json")
        .body(body.clone());
    if !state.upstream_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", state.upstream_key));
    }

    let start = std::time::Instant::now();
    match req.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = resp.status();
            let resp_body = resp.bytes().await.unwrap_or_default();
            state.emit_spotlight(&consumer, "embeddings", &model, latency).await;
            (status, resp_body).into_response()
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            state.emit_spotlight(&consumer, "embeddings", &model, latency).await;
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({"error": format!("upstream unreachable: {}", e)})),
            ).into_response()
        }
    }
}

async fn fallback_not_found(
    State((_state, _dev_mode)): State<AppState>,
    method: Method,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": format!("{} /v1/{} not found (try /v1/models, /v1/chat/completions, /v1/embeddings)", method, path)
        })),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_key_create_and_validate() {
        let state = Arc::new(LlmState::from_config("http://localhost:8181/v1", ""));
        let key = state.create_key("test-consumer").await;
        assert!(key.starts_with("b00t-sk-"));
        let entry = state.validate_key(&key).await;
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().consumer, "test-consumer");
    }

    #[tokio::test]
    async fn test_unknown_key_is_none() {
        let state = Arc::new(LlmState::from_config("http://localhost:8181/v1", ""));
        let entry = state.validate_key("bogus-key").await;
        assert!(entry.is_none());
    }

    #[tokio::test]
    async fn test_spotlight_emit() {
        let state = Arc::new(LlmState::from_config("http://localhost:8181/v1", ""));
        state.emit_spotlight("test-consumer", "chat_completions", "test-model", 42).await;
        let content = std::fs::read_to_string(&state.spotlight_log).unwrap_or_default();
        assert!(content.contains("spotlight.llm.chat_completions"));
        assert!(content.contains("test-consumer"));
        assert!(content.contains("test-model"));
    }
}
