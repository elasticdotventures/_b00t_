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
                // 🤓 b00t-candle-serve --serve — real local candle chat inference
                // (Phi-4 14B Q4_K GGUF), no external API key, no container. CPU-only
                // is slow (~0.53 tok/s, see _b00t_/phi-4-candle-local.model.ai.tomllmd)
                // so this is listed last among the openai-compat entries: only used
                // when none of mistralrs/llama-cpp/vllm are actually listening.
                LocalBackend { name: "candle-phi".into(), port: 8082, kind: "openai-compat".into(), enabled: true },
                // 🤓 b00t-embed-serve — real local candle embeddings (Qwen3-Embedding-0.6B),
                // no external API key. kind="embeddings" (not "openai-compat") because it
                // only implements /v1/embeddings, not /v1/chat/completions — discover_local()
                // filters on this so chat requests never get routed here by accident.
                LocalBackend { name: "qwen3-embed".into(), port: 8003, kind: "embeddings".into(), enabled: true },
            ],
            remote: vec![
                RemoteBackend { name: "openai".into(), key_env: "OPENAI_API_KEY".into(), base_url: None },
                RemoteBackend { name: "openrouter".into(), key_env: "OPENROUTER_API_KEY".into(), base_url: Some("https://openrouter.ai/api/v1".into()) },
                // 🤓 Telnyx Inference — OpenAI-Chat-Completions-compatible
                // (confirmed live: capability-forge/src/judge.rs's
                // OpenAiJudge::with_base_url uses the identical endpoint
                // shape). Appended last, after the two pre-existing entries,
                // to preserve their relative priority on any host that
                // happens to have multiple keys set — it's the one that
                // actually has a working credential anywhere in this hive
                // today, not necessarily the "best" provider in general.
                RemoteBackend { name: "telnyx".into(), key_env: "TELNYX_API_KEY".into(), base_url: Some("https://api.telnyx.com/v2/ai".into()) },
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

/// `want_kind`: when `Some("embeddings")`, only considers backends whose `kind`
/// is exactly "embeddings" (e.g. b00t-embed-serve, which has no /v1/chat/completions
/// at all) — chat/models discovery must never land on an embeddings-only backend.
/// When `None`, only considers "openai-compat" backends (the historical default,
/// used for chat/models) — an embeddings-only backend is never a valid chat target.
fn discover_local(soul: &SoulConfig, want_kind: Option<&str>) -> Option<(String, String)> {
    let target_kind = want_kind.unwrap_or("openai-compat");
    for be in &soul.backends.local {
        if !be.enabled || be.kind != target_kind { continue; }
        let addr: SocketAddr = format!("127.0.0.1:{}", be.port).parse().ok()?;
        if TcpStream::connect_timeout(&addr, Duration::from_millis(500)).is_ok() {
            eprintln!("🔍 local backend (soul): {} :{} (kind={})", be.name, be.port, be.kind);
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

/// Resolves the upstream to proxy to. `for_embeddings` matters because a local
/// backend can be embeddings-only (b00t-embed-serve, kind="embeddings") — a
/// chat request must never land there, and an embeddings request should prefer
/// it over a general chat backend that may not implement /v1/embeddings at all.
fn resolve_upstream(soul: &SoulConfig, for_embeddings: bool) -> (String, String) {
    let explicit_url_var = if for_embeddings { "B00T_SERVER_EMBEDDINGS_UPSTREAM_URL" } else { "B00T_SERVER_UPSTREAM_URL" };
    if let Ok(url) = std::env::var(explicit_url_var) {
        if !url.is_empty() {
            let key = std::env::var("B00T_SERVER_UPSTREAM_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .unwrap_or_default();
            eprintln!("🌐 upstream (explicit): {}", url);
            return (url, key);
        }
    }
    if for_embeddings {
        // Prefer an embeddings-only local backend; fall back to a general
        // openai-compat local backend in case it also serves /v1/embeddings.
        if let Some((name, url)) = discover_local(soul, Some("embeddings")) {
            eprintln!("📍 upstream (soul/local {}, embeddings): {}", name, url);
            return (url, String::new());
        }
        if let Some((name, url)) = discover_local(soul, None) {
            eprintln!("📍 upstream (soul/local {}, chat backend used for embeddings): {}", name, url);
            return (url, String::new());
        }
    } else if let Some((name, url)) = discover_local(soul, None) {
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
    // 🤓 Resolved independently from upstream_url — a local backend can be
    // embeddings-only (b00t-embed-serve), so /v1/embeddings must not blindly
    // share the chat upstream. See resolve_upstream's for_embeddings param.
    pub embeddings_upstream_url: String,
    pub embeddings_upstream_key: String,
    pub keys: Arc<RwLock<HashMap<String, KeyEntry>>>,
    pub keys_file: std::path::PathBuf,
    pub spotlight_log: std::path::PathBuf,
    // 🤓 `b00t server key create` writes keys_file directly from a separate process
    // (commands/server.rs) — it does not talk to a running LlmState at all. Without
    // tracking the file's mtime and reloading on change, a key minted while the
    // server is already running would never authenticate until restart.
    keys_file_mtime: Arc<RwLock<Option<std::time::SystemTime>>>,
}

/// Parses the same `{"keys": {...}}` shape `commands/server.rs`'s `KeyAction::Create`
/// writes. Shared by `from_config` (initial load) and `reload_if_changed` (hot reload)
/// so the two never drift apart.
fn load_keys_from_file(keys_file: &std::path::Path) -> HashMap<String, KeyEntry> {
    let mut keys = HashMap::new();
    if let Ok(data) = std::fs::read_to_string(keys_file) {
        if let Ok(parsed) = serde_json::from_str::<Value>(&data) {
            if let Some(obj) = parsed.get("keys").and_then(|k| k.as_object()) {
                for (k, v) in obj {
                    if let (Some(consumer), Some(created_at)) = (
                        v.get("consumer").and_then(|c| c.as_str()),
                        v.get("created_at").and_then(|c| c.as_str()),
                    ) {
                        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(created_at) {
                            let access = v.get("access").and_then(|a| a.as_array())
                                .map(|arr| arr.iter().filter_map(|p| {
                                    let class = p.get("class").and_then(|c| c.as_str())?;
                                    let action = match p.get("action").and_then(|a| a.as_str())? {
                                        "read" => Action::Read,
                                        "write" => Action::Write,
                                        _ => Action::Execute,
                                    };
                                    Some(ClassPermission { class: class.to_string(), action })
                                }).collect())
                                .unwrap_or_default();
                            keys.insert(k.clone(), KeyEntry {
                                consumer: consumer.to_string(),
                                created_at: ts.with_timezone(&chrono::Utc),
                                access,
                            });
                        }
                    }
                }
            }
        }
    }
    keys
}

impl LlmState {
    pub fn new() -> Self {
        let soul = SoulConfig::load();
        let (url, key) = resolve_upstream(&soul, false);
        let (embed_url, embed_key) = resolve_upstream(&soul, true);
        Self::from_config_full(&url, &key, &embed_url, &embed_key)
    }

    /// Convenience for tests / callers that don't care about a distinct
    /// embeddings backend — uses the same upstream for both.
    pub fn from_config(upstream_url: &str, upstream_key: &str) -> Self {
        Self::from_config_full(upstream_url, upstream_key, upstream_url, upstream_key)
    }

    pub fn from_config_full(
        upstream_url: &str,
        upstream_key: &str,
        embeddings_upstream_url: &str,
        embeddings_upstream_key: &str,
    ) -> Self {
        let home = dirs_next().unwrap_or_else(|| std::path::PathBuf::from("."));
        let keys_file = home.join("server-keys.json");
        let keys = load_keys_from_file(&keys_file);
        let mtime = std::fs::metadata(&keys_file).and_then(|m| m.modified()).ok();
        Self {
            upstream_url: upstream_url.trim_end_matches('/').to_string(),
            upstream_key: upstream_key.to_string(),
            embeddings_upstream_url: embeddings_upstream_url.trim_end_matches('/').to_string(),
            embeddings_upstream_key: embeddings_upstream_key.to_string(),
            keys: Arc::new(RwLock::new(keys)),
            keys_file,
            spotlight_log: home.join("spotlight.jsonl"),
            keys_file_mtime: Arc::new(RwLock::new(mtime)),
        }
    }

    /// Re-reads keys_file if its mtime changed since we last loaded it — cheap
    /// (one stat call) on the common case where nothing changed. Called before
    /// every validate_key/check_access so keys minted by a separate `b00t server
    /// key create` invocation while this server is already running actually work.
    async fn reload_if_changed(&self) {
        let current_mtime = std::fs::metadata(&self.keys_file).and_then(|m| m.modified()).ok();
        let mut cached = self.keys_file_mtime.write().await;
        if current_mtime != *cached {
            *self.keys.write().await = load_keys_from_file(&self.keys_file);
            *cached = current_mtime;
        }
    }

    pub async fn validate_key(&self, token: &str) -> Option<KeyEntry> {
        self.reload_if_changed().await;
        self.keys.read().await.get(token).cloned()
    }

    pub async fn check_access(&self, token: &str, class: &str, action: Action) -> bool {
        if let Some(entry) = self.validate_key(token).await {
            if entry.access.is_empty() {
                return true; // empty access = full access (backwards compat)
            }
            return entry.access.iter().any(|p| p.class == class && p.action == action);
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
        self.emit_spotlight_with_usage(consumer, endpoint, model, latency_ms, None, None).await;
    }

    /// Same as `emit_spotlight`, plus opportunistic token counts. `prompt_tokens`/
    /// `completion_tokens` come from the proxied response's OpenAI-shaped `usage`
    /// object when present — best-effort, callers pass `None` if the upstream
    /// didn't include one or it failed to parse; never blocks the response.
    async fn emit_spotlight_with_usage(
        &self,
        consumer: &str,
        endpoint: &str,
        model: &str,
        latency_ms: u64,
        prompt_tokens: Option<u64>,
        completion_tokens: Option<u64>,
    ) {
        let mut event = json!({
            "ts": chrono::Utc::now().to_rfc3339(),
            "event": format!("spotlight.llm.{}", endpoint),
            "consumer": consumer,
            "model": model,
            "latency_ms": latency_ms,
        });
        if let Some(pt) = prompt_tokens {
            event["prompt_tokens"] = json!(pt);
        }
        if let Some(ct) = completion_tokens {
            event["completion_tokens"] = json!(ct);
        }
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&self.spotlight_log) {
            use std::io::Write;
            let _ = writeln!(f, "{}", event);
        }
    }
}

/// Best-effort extraction of an OpenAI-shaped `usage.{prompt_tokens,completion_tokens}`
/// object from a raw proxied response body. Returns `(None, None)` on any parse
/// failure or missing fields — callers must never fail the request over this.
fn extract_usage_tokens(body: &[u8]) -> (Option<u64>, Option<u64>) {
    let Ok(parsed) = serde_json::from_slice::<Value>(body) else {
        return (None, None);
    };
    let usage = parsed.get("usage");
    let prompt = usage.and_then(|u| u.get("prompt_tokens")).and_then(|v| v.as_u64());
    let completion = usage.and_then(|u| u.get("completion_tokens")).and_then(|v| v.as_u64());
    (prompt, completion)
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

/// POST one chat-completion body upstream, honoring the caller's upstream
/// key. Shared by the plain forward path and the verify tool loop's re-entry
/// sends (`verify_tool_loop::run_tool_loop`'s `send` parameter).
async fn send_upstream_chat(upstream_url: &str, upstream_key: &str, body: Value) -> anyhow::Result<Value> {
    let url = format!("{upstream_url}/chat/completions");
    let client = reqwest::Client::new();
    let mut req = client.post(&url).header("Content-Type", "application/json").json(&body);
    if !upstream_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {upstream_key}"));
    }
    let resp = req.send().await?;
    let status = resp.status();
    let parsed: Value = resp.json().await?;
    if !status.is_success() {
        anyhow::bail!("upstream {status}: {parsed}");
    }
    Ok(parsed)
}

async fn proxy_chat(
    State((state, dev_mode)): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> impl IntoResponse {
    let token = extract_bearer_token(&headers, dev_mode).unwrap_or_default();
    let consumer = state.validate_key(&token).await.map(|k| k.consumer).unwrap_or_else(|| "unknown".to_string());
    if !dev_mode && !state.check_access(&token, "b00t:ChatModel", Action::Execute).await {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "access denied: b00t:ChatModel:execute"}))).into_response();
    }

    // Non-JSON or unparseable bodies can't carry tool injection / the verify
    // loop — forward verbatim exactly as before (#596/#597 wiring is
    // best-effort, never a hard requirement for basic proxying to work).
    let Ok(mut request) = serde_json::from_slice::<Value>(&body) else {
        return forward_chat_verbatim(&state, &consumer, &headers, body).await;
    };
    let model = request["model"].as_str().unwrap_or("unknown").to_string();
    let is_streaming = request["stream"].as_bool().unwrap_or(false);

    // Opt-in tool injection (default off — see verify_tool_loop doc comment).
    if std::env::var("B00T_VERIFY_TOOL_INJECT").as_deref() == Ok("1") {
        crate::verify_tool_loop::inject_verify_tool(&mut request);
    }

    let start = std::time::Instant::now();
    let first = match send_upstream_chat(&state.upstream_url, &state.upstream_key, request.clone()).await {
        Ok(v) => v,
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            state.emit_spotlight(&consumer, "chat_completions", &model, latency).await;
            return (StatusCode::BAD_GATEWAY, Json(json!({"error": format!("upstream unreachable: {}", e)}))).into_response();
        }
    };

    // Streaming bypasses the loop entirely (SSE bytes aren't JSON messages to
    // splice into) — the doc comment on verify_tool_loop covers this as
    // known follow-up work, not a bug.
    let mut final_response = if is_streaming {
        first
    } else {
        crate::verify_tool_loop::run_tool_loop(
            &request,
            first,
            |next_body| send_upstream_chat(&state.upstream_url, &state.upstream_key, next_body),
            crate::verify_tool_loop::execute_verify_call,
        )
        .await
    };

    // Grammar-shape audit (#596 bridge): if content contains
    // `[tool_call: verify assertion="X"] -> [result: R] ->`, re-run the
    // assertion through real Z3 and correct any hallucinated result token.
    // No-op (regex doesn't match) for the overwhelming majority of requests
    // that never produced grammar-shaped output.
    let content = final_response["choices"][0]["message"]["content"]
        .as_str()
        .map(str::to_string);
    if let Some(content) = content {
        if let Some((audited, _summary)) =
            crate::verify_tool_loop::audit_grammar_content(&content, crate::verify_tool_loop::z3_result_of)
        {
            final_response["choices"][0]["message"]["content"] = Value::String(audited);
        }
    }

    let latency = start.elapsed().as_millis() as u64;
    state.emit_spotlight(&consumer, "chat_completions", &model, latency).await;
    Json(final_response).into_response()
}

/// Pre-#596/#597 behavior: forward the raw request body upstream unmodified
/// and return the upstream response verbatim (status + bytes). Used when the
/// body isn't valid JSON, so there's no `Value` to inject tools into or loop
/// over.
async fn forward_chat_verbatim(
    state: &LlmState,
    consumer: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> axum::response::Response {
    let model = "unknown".to_string();
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
            let (prompt_tokens, completion_tokens) = extract_usage_tokens(&body);
            state.emit_spotlight_with_usage(consumer, "chat_completions", &model, latency, prompt_tokens, completion_tokens).await;
            (status, body).into_response()
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            state.emit_spotlight(consumer, "chat_completions", &model, latency).await;
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
    if !dev_mode && !state.check_access(&token, "b00t:EmbeddingModel", Action::Execute).await {
        return (StatusCode::FORBIDDEN, Json(json!({"error": "access denied: b00t:EmbeddingModel:execute"}))).into_response();
    }
    let model = serde_json::from_slice::<Value>(&body)
        .ok().and_then(|v| v.get("model").and_then(|m| m.as_str().map(|s| s.to_string())))
        .unwrap_or_else(|| "unknown".to_string());
    let url = format!("{}/embeddings", state.embeddings_upstream_url);
    let client = reqwest::Client::new();
    let mut req = client.post(&url).header("Content-Type", "application/json").body(body.clone());
    if !state.embeddings_upstream_key.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", state.embeddings_upstream_key));
    }
    let start = std::time::Instant::now();
    match req.send().await {
        Ok(resp) => {
            let latency = start.elapsed().as_millis() as u64;
            let status = resp.status();
            let body = resp.bytes().await.unwrap_or_default();
            let (prompt_tokens, _) = extract_usage_tokens(&body);
            state.emit_spotlight_with_usage(&consumer, "embeddings", &model, latency, prompt_tokens, None).await;
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
    use std::sync::{Mutex, MutexGuard};

    // ── Sandbox tests from the real $HOME (#1113 hygiene fix) ───────────────
    //
    // `LlmState::from_config` resolves `keys_file`/`spotlight_log` against
    // `dirs_next()`, which reads the process-wide `HOME` env var. Without
    // sandboxing, every test in this module reads/writes the developer's
    // real `~/.b00t/server-keys.json` and `~/.b00t/spotlight.jsonl`. This
    // mirrors the `TempHome` pattern already used independently in
    // `b00t-c0re-lib/src/events.rs` and `b00t-cli/src/lib.rs`.

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct TempHome {
        _guard: MutexGuard<'static, ()>,
        old_home: Option<String>,
        _temp_dir: tempfile::TempDir,
    }

    impl TempHome {
        fn new() -> Self {
            let guard = HOME_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let temp_dir = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(temp_dir.path().join(".b00t")).unwrap();
            let old_home = std::env::var("HOME").ok();
            unsafe {
                std::env::set_var("HOME", temp_dir.path().to_str().unwrap());
            }

            Self {
                _guard: guard,
                old_home,
                _temp_dir: temp_dir,
            }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            if let Some(old) = &self.old_home {
                unsafe {
                    std::env::set_var("HOME", old);
                }
            } else {
                unsafe {
                    std::env::remove_var("HOME");
                }
            }
        }
    }

    #[tokio::test]
    async fn test_key_create_and_validate() {
        let _temp_home = TempHome::new();
        let state = Arc::new(LlmState::from_config("http://localhost:8181/v1", ""));
        let key = state.create_key("test-consumer", &[]).await;
        assert!(key.starts_with("b00t-sk-"));
        let entry = state.validate_key(&key).await;
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().consumer, "test-consumer");
    }

    #[tokio::test]
    async fn test_unknown_key_is_none() {
        let _temp_home = TempHome::new();
        let state = Arc::new(LlmState::from_config("http://localhost:8181/v1", ""));
        let entry = state.validate_key("bogus-key").await;
        assert!(entry.is_none());
    }

    #[tokio::test]
    async fn test_key_created_externally_is_picked_up_without_restart() {
        // Simulates `b00t server key create` (commands/server.rs) writing directly
        // to keys_file from a SEPARATE process while this LlmState is already
        // running — this is the exact scenario the reload-on-mtime-change fix
        // targets (Phase A.2). Writes the same {"keys": {...}} shape
        // commands/server.rs produces, bypassing state.create_key() entirely.
        let state = Arc::new(LlmState::from_config("http://localhost:8181/v1", ""));

        // Confirm the key genuinely doesn't exist yet (no accidental collision).
        let external_key = format!("b00t-sk-test-external-{}", Uuid::new_v4().simple());
        assert!(state.validate_key(&external_key).await.is_none());

        // Read-modify-write the real keys_file exactly like KeyAction::Create does,
        // without going through this LlmState's in-memory map at all.
        let mut data: Value = serde_json::from_str(
            &std::fs::read_to_string(&state.keys_file).unwrap_or_default(),
        )
        .unwrap_or_else(|_| serde_json::json!({"keys": {}}));
        data["keys"][&external_key] = serde_json::json!({
            "consumer": "external-process-consumer",
            "created_at": chrono::Utc::now().to_rfc3339(),
            "access": [],
        });
        // Ensure mtime actually advances even on filesystems with coarse mtime
        // resolution — sleep briefly before writing.
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        std::fs::write(&state.keys_file, serde_json::to_string(&data).unwrap()).unwrap();

        let entry = state.validate_key(&external_key).await;
        assert!(entry.is_some(), "key written externally must be picked up without restart");
        assert_eq!(entry.unwrap().consumer, "external-process-consumer");
    }

    #[tokio::test]
    async fn test_spotlight_emit() {
        let _temp_home = TempHome::new();
        let state = Arc::new(LlmState::from_config("http://localhost:8181/v1", ""));
        state.emit_spotlight("test-consumer", "chat_completions", "test-model", 42).await;
        let content = std::fs::read_to_string(&state.spotlight_log).unwrap_or_default();
        assert!(content.contains("spotlight.llm.chat_completions"));
        assert!(content.contains("test-consumer"));
        assert!(content.contains("test-model"));
    }

    /// Proves the sandboxing actually isolates instances from each other via
    /// the filesystem, not just that tests pass. Two sequential `TempHome`
    /// scopes: a key created under the first is invisible to a fresh
    /// `LlmState` constructed under the second, because `validate_key` is a
    /// pure in-memory read of the `keys` map populated at construction time
    /// (it never re-reads `keys_file`), and the second `TempHome` points
    /// `HOME` at an entirely different, empty temp directory.
    #[tokio::test]
    async fn test_temp_home_isolates_state_across_instances() {
        let key = {
            let _first_home = TempHome::new();
            let state = LlmState::from_config("http://localhost:8181/v1", "");
            let key = state.create_key("consumer-in-first-home", &[]).await;
            assert!(state.validate_key(&key).await.is_some(), "key must be visible within its own instance");
            key
            // _first_home dropped here: HOME restored, temp dir removed
        };

        let _second_home = TempHome::new();
        let fresh_state = LlmState::from_config("http://localhost:8181/v1", "");
        assert!(
            fresh_state.validate_key(&key).await.is_none(),
            "a key created under one TempHome must not leak into a fresh LlmState \
             constructed under a different TempHome"
        );
    }

    // ── remote backend discovery — Telnyx addition ──────────────────────────

    static REMOTE_BACKEND_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn clear_remote_backend_env() {
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
            std::env::remove_var("OPENROUTER_API_KEY");
            std::env::remove_var("TELNYX_API_KEY");
        }
    }

    #[test]
    fn default_soul_includes_candle_phi_as_a_local_chat_backend() {
        let soul = default_soul("test-host");
        let candle = soul
            .backends
            .local
            .iter()
            .find(|b| b.name == "candle-phi")
            .expect("candle-phi must be a default local backend");
        assert_eq!(candle.port, 8082);
        assert_eq!(candle.kind, "openai-compat");
        assert!(candle.enabled);
    }

    #[test]
    fn default_soul_includes_telnyx_as_a_remote_backend() {
        let soul = default_soul("test-host");
        let telnyx = soul
            .backends
            .remote
            .iter()
            .find(|b| b.name == "telnyx")
            .expect("telnyx must be a default remote backend");
        assert_eq!(telnyx.key_env, "TELNYX_API_KEY");
        assert_eq!(telnyx.base_url.as_deref(), Some("https://api.telnyx.com/v2/ai"));
    }

    #[test]
    fn discover_remote_picks_telnyx_when_it_is_the_only_key_set() {
        let _guard = REMOTE_BACKEND_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        clear_remote_backend_env();
        unsafe {
            std::env::set_var("TELNYX_API_KEY", "test-telnyx-key");
        }

        let soul = default_soul("test-host");
        let found = discover_remote(&soul);
        clear_remote_backend_env();

        let (name, key, url) = found.expect("a remote backend must be found");
        assert_eq!(name, "telnyx");
        assert_eq!(key, "test-telnyx-key");
        assert_eq!(url, "https://api.telnyx.com/v2/ai");
    }

    #[test]
    fn discover_remote_prefers_openai_over_telnyx_when_both_are_set() {
        // Telnyx was appended AFTER the pre-existing entries specifically to
        // preserve their relative priority on any host that happens to have
        // multiple keys configured — this proves that ordering held.
        let _guard = REMOTE_BACKEND_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        clear_remote_backend_env();
        unsafe {
            std::env::set_var("OPENAI_API_KEY", "test-openai-key");
            std::env::set_var("TELNYX_API_KEY", "test-telnyx-key");
        }

        let soul = default_soul("test-host");
        let found = discover_remote(&soul);
        clear_remote_backend_env();

        let (name, ..) = found.expect("a remote backend must be found");
        assert_eq!(name, "openai");
    }

    #[test]
    fn discover_remote_finds_nothing_when_no_keys_are_set() {
        let _guard = REMOTE_BACKEND_ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        clear_remote_backend_env();

        let soul = default_soul("test-host");
        assert!(discover_remote(&soul).is_none());
    }

    // ── proxy_chat #596/#597 wiring — end-to-end against a fake upstream ─────

    /// Minimal fake upstream: first call returns a `verify` tool_calls
    /// response, second call (the loop's re-entry) returns a plain stop
    /// response. Proves `proxy_chat` actually drives `run_tool_loop` over a
    /// real HTTP round trip, not just that the loop functions are unit-correct
    /// in isolation.
    async fn spawn_fake_upstream(responses: Vec<Value>) -> String {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let responses = Arc::new(responses);
        let call_count = Arc::new(AtomicUsize::new(0));
        let app = Router::new().route(
            "/chat/completions",
            axum::routing::post({
                let responses = responses.clone();
                let call_count = call_count.clone();
                move |_body: Bytes| {
                    let responses = responses.clone();
                    let call_count = call_count.clone();
                    async move {
                        let i = call_count.fetch_add(1, Ordering::SeqCst);
                        Json(responses[i.min(responses.len() - 1)].clone())
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        format!("http://{addr}")
    }

    fn tool_call_upstream_response(assertion: &str) -> Value {
        json!({"choices": [{"finish_reason": "tool_calls", "message": {
            "role": "assistant",
            "tool_calls": [{"id": "call_1", "type": "function", "function": {
                "name": "verify",
                "arguments": format!("{{\"assertion\": \"{assertion}\"}}"),
            }}]
        }}]})
    }

    fn stop_upstream_response(content: &str) -> Value {
        json!({"choices": [{"finish_reason": "stop", "message": {"role": "assistant", "content": content}}]})
    }

    #[tokio::test]
    async fn proxy_chat_drives_verify_tool_loop_end_to_end() {
        let _temp_home = TempHome::new();
        let upstream_url = spawn_fake_upstream(vec![
            tool_call_upstream_response("(assert true)(check-sat)"),
            stop_upstream_response("verified via loop"),
        ])
        .await;
        let state = Arc::new(LlmState::from_config(&upstream_url, ""));
        let request_body = json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "is this satisfiable?"}]
        });
        let body = Bytes::from(serde_json::to_vec(&request_body).unwrap());

        let resp = proxy_chat(State((state, true)), HeaderMap::new(), body)
            .await
            .into_response();
        assert_eq!(resp.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            parsed["choices"][0]["message"]["content"],
            json!("verified via loop"),
            "proxy_chat must return the loop's final response, not the first tool_calls hop: {parsed}"
        );
    }

    #[tokio::test]
    async fn proxy_chat_audits_hallucinated_grammar_shaped_result() {
        // execute_verify_call shells out to `b00t-cli admin verify`, which
        // shells out to `z3` — skip gracefully when it's not in PATH rather
        // than asserting against an "error"/"unknown" result (matches the
        // z3-subprocess skip convention in b00t-datum-core's own tests).
        if std::process::Command::new("z3").arg("--version").output().is_err() {
            eprintln!("skipping proxy_chat_audits_hallucinated_grammar_shaped_result: z3 not in PATH");
            return;
        }
        let _temp_home = TempHome::new();
        // Model emits grammar-shaped content claiming "sat" for an assertion
        // that is actually unsat — proxy_chat must correct it before
        // returning to the client (#596 grammar-shape audit bridge).
        let claim = "[tool_call: verify assertion=\"(declare-const x Int)(assert (and (> x 0) (< x 0)))(check-sat)\"] → [result: sat] → contradiction holds.";
        let upstream_url = spawn_fake_upstream(vec![stop_upstream_response(claim)]).await;
        let state = Arc::new(LlmState::from_config(&upstream_url, ""));
        let request_body = json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "can x be both positive and negative?"}]
        });
        let body = Bytes::from(serde_json::to_vec(&request_body).unwrap());

        let resp = proxy_chat(State((state, true)), HeaderMap::new(), body)
            .await
            .into_response();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        let content = parsed["choices"][0]["message"]["content"].as_str().unwrap();
        assert!(
            content.contains("[result: unsat]"),
            "hallucinated 'sat' must be corrected to real z3 result: {content}"
        );
        assert!(content.contains("contradiction holds."), "surrounding text preserved: {content}");
    }
}
