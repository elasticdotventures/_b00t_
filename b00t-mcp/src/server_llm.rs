// 🤓 b00t-server: OpenAI-compatible REST API router — transparent proxy
//    Validates b00t-issued API keys → forwards to upstream LLM backend →
//    emits Spotlight usage events. Mounts on the existing b00t-mcp axum server.
//
//    Backend discovery via runtime config: ~/.b00t/server-soul.tomllm
//    (local uncommitted — lists local ports + remote profiles with key_env).
//    No hardcoded IPs or server names in the binary.
//    Reference: vendor/irontology-mcp/crates/provider-openai/src/lib.rs:299-354

use axum::{
    Router,
    body::Bytes,
    extract::State,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
};
use axum::http::header;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::net::{SocketAddr, TcpStream};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

// ── Soul config (runtime backend registry) ─────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulConfig {
    pub soul: SoulMeta,
    pub backends: BackendsSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulMeta {
    #[serde(default)]
    pub hostname: String,
    #[serde(default)]
    pub blessings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendsSection {
    #[serde(default)]
    pub local: Vec<LocalBackend>,
    #[serde(default)]
    pub remote: Vec<RemoteBackend>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalBackend {
    pub name: String,
    pub port: u16,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteBackend {
    pub name: String,
    pub key_env: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

fn default_kind() -> String { "openai-compat".into() }
fn default_enabled() -> bool { true }

const SOUL_PATH: &str = "server-soul.tomllm";

fn default_soul(hostname: &str) -> SoulConfig {
    SoulConfig {
        soul: SoulMeta {
            hostname: hostname.to_string(),
            blessings: vec!["rust-doc".into()],
        },
        backends: BackendsSection {
            local: vec![
                LocalBackend { name: "mistralrs".into(), port: 8181, kind: "openai-compat".into(), enabled: true },
                LocalBackend { name: "llama-cpp".into(), port: 8080, kind: "openai-compat".into(), enabled: true },
                LocalBackend { name: "vllm".into(), port: 8000, kind: "openai-compat".into(), enabled: true },
            ],
            remote: vec![
                RemoteBackend { name: "openai".into(), key_env: "OPENAI_API_KEY".into(), base_url: None },
                RemoteBackend { name: "openrouter".into(), key_env: "OPENROUTER_API_KEY".into(), base_url: Some("https://openrouter.ai/api/v1".into()) },
            ],
        },
    }
}

impl SoulConfig {
    pub fn load() -> Self {
        let home = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));
        let path = home.join(SOUL_PATH);
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(cfg) = toml::from_str::<SoulConfig>(&data) {
                if !cfg.backends.local.is_empty() || !cfg.backends.remote.is_empty() {
                    return cfg;
                }
            }
        }
        let hostname = whoami::fallible::hostname().unwrap_or_else(|_| "unknown".into());
        let cfg = default_soul(&hostname);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let header = concat!(
            "# b00t server soul — runtime backend registry (local, uncommitted)\n",
            "# Edit to add/remove backends. Remote entries reference env var keys.\n",
            "# b00t:map v1\n# summary: b00t-server backend discovery config\n",
            "# tags: server, soul, backends\n# tier: sm0l\n",
            "# cmds: b00t server start\n# complexity: 2\n#\n",
        );
        if let Ok(toml_str) = toml::to_string_pretty(&cfg) {
            let _ = std::fs::write(&path, format!("{}{}", header, toml_str));
        }
        cfg
    }
}

fn discover_local(soul: &SoulConfig) -> Option<(String, String)> {
    for be in &soul.backends.local {
        if !be.enabled { continue; }
        let addr: SocketAddr = format!("127.0.0.1:{}", be.port).parse().ok()?;
        if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
            eprintln!("🔍 local backend (soul): {} :{}", be.name, be.port);
            return Some((be.name.clone(), format!("http://127.0.0.1:{}/v1", be.port)));
        }
    }
    None
}

fn discover_remote(soul: &SoulConfig) -> Option<(String, String, String)> {
    for be in &soul.backends.remote {
        if let Ok(key) = std::env::var(&be.key_env) {
            if key.is_empty() { continue; }
            let url = be.base_url.clone().unwrap_or_else(|| "https://api.openai.com/v1".into());
            return Some((be.name.clone(), key, url));
        }
    }
    None
}

fn resolve_upstream(soul: &SoulConfig) -> (String, String) {
    if let Ok(url) = std::env::var("B00T_SERVER_UPSTREAM_URL") {
        if !url.is_empty() {
            let key = std::env::var("B00T_SERVER_UPSTREAM_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .unwrap_or_default();
            eprintln!("🌐 upstream (explicit): {}", url);
            return (url, key);
        }
    }
    if let Some((name, url)) = discover_local(soul) {
        eprintln!("📍 upstream (soul/local {}): {}", name, url);
        return (url, String::new());
    }
    if let Some((name, key, url)) = discover_remote(soul) {
        eprintln!("🌐 upstream (soul/remote {}): {}", name, url);
        return (url, key);
    }
    eprintln!("⚠️  No upstream configured — populate ~/.b00t/{}", SOUL_PATH);
    ("http://localhost:8181/v1".to_string(), String::new())
}

// ── State ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action { Read, Write, Execute }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassPermission {
    pub class: String,
    pub action: Action,
}

impl ClassPermission {
    pub fn parse(s: &str) -> Option<Self> {
        // Format: "b00t:EmbeddingModel:execute"
        let parts: Vec<&str> = s.rsplitn(2, ':').collect();
        if parts.len() != 2 { return None; }
        let action = match parts[0] {
            "read" => Action::Read,
            "write" => Action::Write,
            "execute" => Action::Execute,
            _ => return None,
        };
        Some(ClassPermission { class: parts[1].to_string(), action })
    }

    pub fn to_hydra_scope(&self) -> String {
        HYDRA_SCOPE_MAP
            .iter()
            .find(|(class, action, _)| *class == self.class && *action == self.action)
            .map(|(_, _, scope)| scope.to_string())
            .unwrap_or_else(|| {
                format!("{}.{}",
                    self.class.strip_prefix("b00t:").unwrap_or(&self.class).to_lowercase(),
                    format!("{:?}", self.action).to_lowercase(),
                )
            })
    }
}

static HYDRA_SCOPE_MAP: &[(&str, Action, &str)] = &[
    ("b00t:ChatModel", Action::Execute, "chat.execute"),
    ("b00t:EmbeddingModel", Action::Execute, "embedding.execute"),
    ("b00t:Model", Action::Read, "model.read"),
    ("b00t:Model", Action::Write, "model.write"),
    ("b00t:Store", Action::Read, "store.read"),
    ("b00t:Store", Action::Write, "store.write"),
];

#[derive(Clone, Serialize, Deserialize)]
pub struct KeyEntry {
    pub consumer: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub access: Vec<ClassPermission>,
}

impl KeyEntry {
    pub fn hydra_scopes(&self) -> String {
        self.access
            .iter()
            .map(|p| p.to_hydra_scope())
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn hydra_client_payload(&self, client_id: &str) -> serde_json::Value {
        serde_json::json!({
            "client_id": client_id,
            "client_name": self.consumer,
            "grant_types": ["client_credentials"],
            "scope": self.hydra_scopes(),
            "token_endpoint_auth_method": "client_secret_basic",
        })
    }
}

pub struct LlmState {
    pub upstream_url: String,
    pub upstream_key: String,
    pub keys: Arc<RwLock<HashMap<String, KeyEntry>>>,
    pub keys_file: std::path::PathBuf,
    pub spotlight_log: std::path::PathBuf,
}

impl LlmState {
    pub fn new() -> Self {
        let soul = SoulConfig::load();
        let (url, key) = resolve_upstream(&soul);
        Self::from_config(&url, &key)
    }

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
                                    access: Vec::new(),
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

    pub async fn check_access(&self, token: &str, class: &str, action: Action) -> bool {
        if let Some(entry) = self.validate_key(token).await {
            if entry.access.is_empty() {
                return true; // empty access = full access (backwards compat)
            }
            return entry.access.iter().any(|p| p.class == class && matches!(p.action, Action::Execute) || matches!(p.action, Action::Read));
        }
        false
    }

    pub async fn create_key(&self, consumer: &str, access: &[String]) -> String {
        let key = format!("b00t-sk-{}", Uuid::new_v4().simple());
        let permissions: Vec<ClassPermission> = access.iter()
            .filter_map(|a| ClassPermission::parse(a))
            .collect();
        self.keys.write().await.insert(key.clone(), KeyEntry {
            consumer: consumer.to_string(),
            created_at: chrono::Utc::now(),
            access: permissions,
        });
        self.save_keys_to_file().await;
        key
    }

    async fn save_keys_to_file(&self) {
        let keys = self.keys.read().await;
        let mut map = serde_json::Map::new();
        for (k, v) in keys.iter() {
            let access_json: Vec<Value> = v.access.iter().map(|p| json!({
                "class": p.class,
                "action": serde_json::to_value(&p.action).unwrap_or(json!("execute")),
            })).collect();
            map.insert(k.clone(), json!({
                "consumer": v.consumer,
                "created_at": v.created_at.to_rfc3339(),
                "access": access_json,
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
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.spotlight_log) {
            use std::io::Write;
            let _ = writeln!(f, "{}", event);
        }
    }
}

fn dirs_next() -> Option<std::path::PathBuf> {
    std::env::var("HOME").ok().map(|h| std::path::PathBuf::from(h).join(".b00t"))
}

// ── Auth provider selection ──────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthProvider {
    /// Dev mode — bypass all auth, use hardcoded dev-key
    Dev,
    /// Basic auth — API keys from server-keys.json + ClassPermission ACL
    Basic,
    /// OAuth 2.1 — Hydra token introspection + ClassPermission ACL
    Hydra,
}

impl AuthProvider {
    pub fn from_env_or_default() -> Self {
        if std::env::var("B00T_SERVER_DEV").map_or(false, |v| v == "1") {
            return AuthProvider::Dev;
        }
        if std::env::var("HYDRA_ADMIN_URL").is_ok() {
            return AuthProvider::Hydra;
        }
        AuthProvider::Basic
    }
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

// ── Endpoint → ontology class mapping ─────────────────────────────────────

fn class_for_path(path: &str) -> (&str, Action) {
    if path.contains("chat/completions") {
        ("b00t:ChatModel", Action::Execute)
    } else if path.contains("embeddings") {
        ("b00t:EmbeddingModel", Action::Execute)
    } else {
        ("b00t:Model", Action::Read)
    }
}

// ── Handlers ───────────────────────────────────────────────────────────────

async fn list_models(
    State((state, dev_mode)): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let token = extract_bearer_token(&headers, dev_mode).unwrap_or_default();
    let consumer = state.validate_key(&token).await.map(|k| k.consumer).unwrap_or_else(|| "unknown".to_string());
    if !dev_mode && !state.check_access(&token, "b00t:ChatModel", Action::Execute).await {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "access denied: b00t:ChatModel:execute"}))).into_response();
    }

    if !dev_mode && !state.check_access(&token, "b00t:Model", Action::Read).await {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "access denied: b00t:Model:read"}))).into_response();
    }
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
            (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("upstream unreachable: {}", e)}))).into_response()
        }
    }
}

async fn proxy_chat(
    State((state, dev_mode)): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let token = extract_bearer_token(&headers, dev_mode).unwrap_or_default();
    let consumer = state.validate_key(&token).await.map(|k| k.consumer).unwrap_or_else(|| "unknown".to_string());
    if !dev_mode && !state.check_access(&token, "b00t:EmbeddingModel", Action::Execute).await {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "access denied: b00t:EmbeddingModel:execute"}))).into_response();
    }
    let model = serde_json::from_slice::<Value>(&body)
        .ok().and_then(|v| v.get("model").and_then(|m| m.as_str().map(|s| s.to_string())))
        .unwrap_or_else(|| "unknown".to_string());
    let url = format!("{}/chat/completions", state.upstream_url);
    let client = reqwest::Client::new();
    let mut req = client.post(&url).header("Content-Type", "application/json").body(body.clone());
    if !state.upstream_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", state.upstream_key));
    }
    if let Some(ct) = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
        req = req.header("Content-Type", ct);
    }
    let start = std::time::Instant::now();
    match req.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = resp.status();
            let body = resp.bytes().await.unwrap_or_default();
            state.emit_spotlight(&consumer, "chat_completions", &model, latency).await;
            (status, body).into_response()
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            state.emit_spotlight(&consumer, "chat_completions", &model, latency).await;
            (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("upstream unreachable: {}", e)}))).into_response()
        }
    }
}

async fn proxy_embeddings(
    State((state, dev_mode)): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let token = extract_bearer_token(&headers, dev_mode).unwrap_or_default();
    let consumer = state.validate_key(&token).await.map(|k| k.consumer).unwrap_or_else(|| "unknown".to_string());
    let model = serde_json::from_slice::<Value>(&body)
        .ok().and_then(|v| v.get("model").and_then(|m| m.as_str().map(|s| s.to_string())))
        .unwrap_or_else(|| "unknown".to_string());
    let url = format!("{}/embeddings", state.upstream_url);
    let client = reqwest::Client::new();
    let mut req = client.post(&url).header("Content-Type", "application/json").body(body.clone());
    if !state.upstream_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", state.upstream_key));
    }
    let start = std::time::Instant::now();
    match req.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = resp.status();
            let body = resp.bytes().await.unwrap_or_default();
            state.emit_spotlight(&consumer, "embeddings", &model, latency).await;
            (status, body).into_response()
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            state.emit_spotlight(&consumer, "embeddings", &model, latency).await;
            (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("upstream unreachable: {}", e)}))).into_response()
        }
    }
}

async fn fallback_not_found(
    State((_state, _dev_mode)): State<AppState>,
    method: Method,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> impl IntoResponse {
    (StatusCode::NOT_FOUND, Json(json!({
        "error": format!("{} /v1/{} not found (try /v1/models, /v1/chat/completions, /v1/embeddings)", method, path)
    })))
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
