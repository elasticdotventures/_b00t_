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
use tokio::sync::Semaphore;
use ufo_types::{DataFormat, ModelCapability};
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
    /// Which data format(s) this backend's model(s) claim to serve — a
    /// registry seed, not yet consumed by any routing logic in this phase.
    /// Empty by default; existing `default_soul()` entries don't populate
    /// this (real per-model capability data is out of this task's scope).
    #[serde(default)]
    pub models: Vec<ModelCapability>,
    /// Caps concurrent in-flight requests proxied to this backend. `None`
    /// means unlimited (the pre-existing behavior for every backend before
    /// this field existed). Enforced in Task 2, not this task.
    #[serde(default)]
    pub max_concurrent: Option<u32>,
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
                LocalBackend { name: "mistralrs".into(), port: 8181, kind: "openai-compat".into(), enabled: true, models: Vec::new(), max_concurrent: None },
                LocalBackend { name: "llama-cpp".into(), port: 8080, kind: "openai-compat".into(), enabled: true, models: Vec::new(), max_concurrent: None },
                LocalBackend { name: "vllm".into(), port: 8000, kind: "openai-compat".into(), enabled: true, models: Vec::new(), max_concurrent: None },
                // 🤓 Windows Foundry Local — real Microsoft local inference
                // runtime, dynamic port (no fixed `port` value applies; the
                // literal 0 below is an unused sentinel — see discover_local's
                // foundry-local special case). Foundry Local's own docs state
                // it is single-user, not built for concurrent multi-client
                // serving, hence max_concurrent: Some(1).
                LocalBackend {
                    name: "foundry-local".into(),
                    port: 0,
                    kind: "openai-compat".into(),
                    enabled: true,
                    models: vec![ModelCapability::new(
                        FOUNDRY_LOCAL_MODEL,
                        vec![DataFormat::Json, DataFormat::PlainText],
                    )],
                    max_concurrent: Some(1),
                },
                // 🤓 b00t-candle-serve --serve — real local candle chat inference
                // (Phi-4 14B Q4_K GGUF), no external API key, no container. CPU-only
                // is slow (~0.53 tok/s, see _b00t_/phi-4-candle-local.model.ai.tomllmd)
                // so this is listed last among the openai-compat entries: only used
                // when none of mistralrs/llama-cpp/vllm are actually listening.
                LocalBackend { name: "candle-phi".into(), port: 8082, kind: "openai-compat".into(), enabled: true, models: Vec::new(), max_concurrent: None },
                // 🤓 b00t-embed-serve — real local candle embeddings (Qwen3-Embedding-0.6B),
                // no external API key. kind="embeddings" (not "openai-compat") because it
                // only implements /v1/embeddings, not /v1/chat/completions — discover_local()
                // filters on this so chat requests never get routed here by accident.
                LocalBackend { name: "qwen3-embed".into(), port: 8003, kind: "embeddings".into(), enabled: true, models: Vec::new(), max_concurrent: None },
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
        // Foundry Local has no fixed port (its `port` field is an unused
        // sentinel — see its default_soul() entry) — it needs a real
        // shell-out discovery step instead of the generic TCP probe below.
        if be.name == "foundry-local" {
            match discover_foundry_local_endpoint() {
                Ok(Some(endpoint)) => {
                    eprintln!("🔍 local backend (soul): {} (foundry-local dynamic discovery)", be.name);
                    return Some((be.name.clone(), format!("{}/v1", endpoint)));
                }
                Ok(None) => continue,
                Err(e) => {
                    eprintln!("⚠️  foundry-local discovery failed: {e}");
                    continue;
                }
            }
        }
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
/// The third return element is the resolved local backend's `max_concurrent`
/// (looked up by name in `soul.backends.local`); `None` for the explicit-URL
/// override, remote-backend, and no-upstream-configured fallback paths, none
/// of which have a concept of a per-backend concurrency cap in this file.
fn resolve_upstream(soul: &SoulConfig, for_embeddings: bool) -> (String, String, Option<u32>) {
    let explicit_url_var = if for_embeddings { "B00T_SERVER_EMBEDDINGS_UPSTREAM_URL" } else { "B00T_SERVER_UPSTREAM_URL" };
    if let Ok(url) = std::env::var(explicit_url_var) {
        if !url.is_empty() {
            let key = std::env::var("B00T_SERVER_UPSTREAM_KEY")
                .or_else(|_| std::env::var("OPENAI_API_KEY"))
                .unwrap_or_default();
            eprintln!("🌐 upstream (explicit): {}", url);
            return (url, key, None);
        }
    }
    if for_embeddings {
        // Prefer an embeddings-only local backend; fall back to a general
        // openai-compat local backend in case it also serves /v1/embeddings.
        if let Some((name, url)) = discover_local(soul, Some("embeddings")) {
            eprintln!("📍 upstream (soul/local {}, embeddings): {}", name, url);
            let max_concurrent = soul.backends.local.iter().find(|b| b.name == name).and_then(|b| b.max_concurrent);
            return (url, String::new(), max_concurrent);
        }
        if let Some((name, url)) = discover_local(soul, None) {
            eprintln!("📍 upstream (soul/local {}, chat backend used for embeddings): {}", name, url);
            let max_concurrent = soul.backends.local.iter().find(|b| b.name == name).and_then(|b| b.max_concurrent);
            return (url, String::new(), max_concurrent);
        }
    } else if let Some((name, url)) = discover_local(soul, None) {
        eprintln!("📍 upstream (soul/local {}): {}", name, url);
        let max_concurrent = soul.backends.local.iter().find(|b| b.name == name).and_then(|b| b.max_concurrent);
        return (url, String::new(), max_concurrent);
    }
    if let Some((name, key, url)) = discover_remote(soul) {
        eprintln!("🌐 upstream (soul/remote {}): {}", name, url);
        return (url, key, None);
    }
    eprintln!("⚠️  No upstream configured — populate ~/.b00t/{}", SOUL_PATH);
    ("http://localhost:8181/v1".to_string(), String::new(), None)
}

/// Real Windows Foundry Local model name this ecosystem already names
/// elsewhere (`ledgrrr`'s `ledgerr-host::internal_openai::FOUNDRY_LOCAL_MODEL`).
const FOUNDRY_LOCAL_MODEL: &str = "phi-4-mini";

/// Discovers Foundry Local's live REST endpoint. Unlike every other local
/// backend in `default_soul()`, Foundry Local does not listen on a fixed,
/// known port — it's assigned dynamically per-launch. Ported from
/// `ledgrrr`'s `ledgerr-host::internal_openai::discover_foundry_local_endpoint`
/// (same shell-out + parse approach, same env var override name kept for
/// operator familiarity across both codebases).
fn discover_foundry_local_endpoint() -> Result<Option<String>, String> {
    if let Ok(endpoint) = std::env::var("LEDGERR_FOUNDRY_LOCAL_ENDPOINT") {
        let trimmed = endpoint.trim();
        if !trimmed.is_empty() {
            return Ok(Some(normalize_foundry_endpoint(trimmed)));
        }
    }

    let output = std::process::Command::new("foundry")
        .args(["service", "status"])
        .output()
        .map_err(|error| format!("failed to run `foundry service status`: {error}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}\n{stderr}");

    if !output.status.success() {
        return Err(format!(
            "`foundry service status` exited with {}: {}",
            output.status,
            combined.trim()
        ));
    }

    let Some(endpoint) = parse_foundry_endpoint(&combined) else {
        return Ok(None);
    };

    Ok(Some(discover_foundry_rest_endpoint(&endpoint).unwrap_or(endpoint)))
}

fn parse_foundry_endpoint(raw: &str) -> Option<String> {
    raw.split(|ch: char| ch.is_whitespace() || matches!(ch, '"' | '\'' | ',' | '[' | ']'))
        .find_map(|token| {
            let endpoint = token
                .trim_matches(|ch| matches!(ch, '.' | ';' | ')' | '('))
                .trim_end_matches('/');
            if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
                Some(normalize_foundry_endpoint(endpoint))
            } else {
                None
            }
        })
}

fn discover_foundry_rest_endpoint(endpoint: &str) -> Option<String> {
    use std::time::Duration;

    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct FoundryStatus {
        endpoints: Vec<String>,
    }

    let status_url = format!("{}/openai/status", normalize_foundry_endpoint(endpoint));
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(2))
        .connect_timeout(Duration::from_secs(1))
        .build()
        .ok()?;
    let status = client.get(&status_url).send().ok()?.json::<FoundryStatus>().ok()?;
    status
        .endpoints
        .into_iter()
        .find(|endpoint| endpoint.starts_with("http://") || endpoint.starts_with("https://"))
        .map(|endpoint| normalize_foundry_endpoint(&endpoint))
}

fn normalize_foundry_endpoint(endpoint: &str) -> String {
    endpoint
        .trim()
        .trim_end_matches('/')
        .trim_end_matches("/v1/chat/completions")
        .trim_end_matches("/v1")
        .trim_end_matches("/openai")
        .to_string()
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
    /// `None` = unlimited (the behavior for every backend before this field
    /// existed, and always the case for `from_config`/`from_config_full` —
    /// only `LlmState::new()`'s real construction path can produce `Some`).
    pub chat_semaphore: Option<Arc<Semaphore>>,
    pub embeddings_semaphore: Option<Arc<Semaphore>>,
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
/// writes, from an already-read string. Split out of `load_keys_from_file` so
/// `write_keys_file_locked` can parse content it read itself through its own
/// locked file handle (see that function's doc comment for why that matters
/// on Windows) instead of re-opening the path via a second handle.
fn parse_keys_json(data: &str) -> HashMap<String, KeyEntry> {
    let mut keys = HashMap::new();
    if let Ok(parsed) = serde_json::from_str::<Value>(data) {
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
    keys
}

/// Parses the same `{"keys": {...}}` shape `commands/server.rs`'s `KeyAction::Create`
/// writes. Shared by `from_config` (initial load) and `reload_if_changed` (hot reload)
/// so the two never drift apart.
fn load_keys_from_file(keys_file: &std::path::Path) -> HashMap<String, KeyEntry> {
    match std::fs::read_to_string(keys_file) {
        Ok(data) => parse_keys_json(&data),
        Err(_) => HashMap::new(),
    }
}

/// Locked, read-merge-write, atomic-rename persist of the keys file — the fix
/// for #1128 (`LlmState::save_keys_to_file` non-atomic cross-process write
/// race). Two live processes writing around the same time — two server
/// instances, or this server plus a separate `b00t server key create`
/// invocation — no longer silently lose one side's key: the exclusive lock
/// serializes writers, and re-reading the current on-disk content before
/// merging in `new_keys` means whatever the other writer already persisted
/// survives. The write itself goes to a temp file then `rename`s over
/// `keys_file`, so any reader that doesn't take the lock
/// (`reload_if_changed`, the CLI's own `KeyAction::List`) never observes a
/// partial write.
///
/// 🤓 The read-side MUST go through the same handle that holds the lock
/// (`lock_file`), not a fresh `load_keys_from_file(keys_file)` open. On
/// Windows, `LockFileEx` locks are mandatory at the OS level and — per
/// Microsoft's own docs — are enforced against every *other* handle to that
/// file, including a second handle opened by the very same process. Opening
/// a brand-new handle here (as an earlier version of this function did, via
/// `load_keys_from_file`) hits that self-conflict: the read fails with a
/// lock-violation I/O error that `load_keys_from_file` swallows silently,
/// so `merged` comes back empty and `new_keys` clobbers whatever the other
/// writer had already persisted — reproducing #1128 on Windows despite the
/// lock. flock(2) on Unix is purely advisory and never had this problem,
/// which is why this only ever showed up on the Windows build machine. See
/// `test_concurrent_instances_do_not_lose_each_others_keys`.
fn write_keys_file_locked(
    keys_file: &std::path::Path,
    new_keys: &HashMap<String, KeyEntry>,
) -> std::io::Result<()> {
    use fs2::FileExt;
    use std::io::Read;

    if let Some(parent) = keys_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Both the lock target AND the read side of read-merge-write below — see
    // the doc comment above for why the read must go through this same
    // handle rather than a fresh open of `keys_file`.
    let mut lock_file = std::fs::OpenOptions::new().create(true).read(true).write(true).open(keys_file)?;
    lock_file.lock_exclusive()?;

    let mut existing = String::new();
    lock_file.read_to_string(&mut existing).unwrap_or(0);
    let mut merged = parse_keys_json(&existing);
    for (k, v) in new_keys {
        merged.insert(k.clone(), v.clone());
    }

    let mut map = serde_json::Map::new();
    for (k, v) in &merged {
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

    let tmp_path = keys_file.with_extension("json.tmp");
    std::fs::write(&tmp_path, data.to_string())?;
    std::fs::rename(&tmp_path, keys_file)?;

    // Lock releases when lock_file drops at function end.
    Ok(())
}

impl LlmState {
    pub fn new() -> Self {
        let soul = SoulConfig::load();
        let (url, key, chat_max_concurrent) = resolve_upstream(&soul, false);
        let (embed_url, embed_key, embeddings_max_concurrent) = resolve_upstream(&soul, true);
        Self::from_config_full_with_concurrency(&url, &key, &embed_url, &embed_key, chat_max_concurrent, embeddings_max_concurrent)
    }

    /// Convenience for tests / callers that don't care about a distinct
    /// embeddings backend — uses the same upstream for both. Always
    /// unlimited concurrency (`None, None`) — use
    /// `from_config_full_with_concurrency` directly if a test needs to
    /// exercise concurrency limiting.
    pub fn from_config(upstream_url: &str, upstream_key: &str) -> Self {
        Self::from_config_full(upstream_url, upstream_key, upstream_url, upstream_key)
    }

    pub fn from_config_full(
        upstream_url: &str,
        upstream_key: &str,
        embeddings_upstream_url: &str,
        embeddings_upstream_key: &str,
    ) -> Self {
        Self::from_config_full_with_concurrency(upstream_url, upstream_key, embeddings_upstream_url, embeddings_upstream_key, None, None)
    }

    pub fn from_config_full_with_concurrency(
        upstream_url: &str,
        upstream_key: &str,
        embeddings_upstream_url: &str,
        embeddings_upstream_key: &str,
        chat_max_concurrent: Option<u32>,
        embeddings_max_concurrent: Option<u32>,
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
            chat_semaphore: chat_max_concurrent.map(|n| Arc::new(Semaphore::new(n as usize))),
            embeddings_semaphore: embeddings_max_concurrent.map(|n| Arc::new(Semaphore::new(n as usize))),
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

    /// See #1208: the write failure is now surfaced (`Err`) rather than only
    /// logged, and a failed persist rolls the in-memory insert back — a key
    /// that didn't durably make it to disk must not silently "exist" for
    /// just this process. Callers can retry; a fresh `Uuid::new_v4()` key is
    /// minted each call, so there's nothing stale left to clean up.
    pub async fn create_key(&self, consumer: &str, access: &[String]) -> std::io::Result<String> {
        let key = format!("b00t-sk-{}", Uuid::new_v4().simple());
        let permissions: Vec<ClassPermission> = access.iter()
            .filter_map(|a| ClassPermission::parse(a))
            .collect();
        self.keys.write().await.insert(key.clone(), KeyEntry {
            consumer: consumer.to_string(),
            created_at: chrono::Utc::now(),
            access: permissions,
        });
        if let Err(e) = self.save_keys_to_file().await {
            self.keys.write().await.remove(&key);
            return Err(e);
        }
        Ok(key)
    }

    /// Persists this instance's in-memory keys to disk. Delegates to
    /// `write_keys_file_locked` (locked read-merge-write + atomic rename) so a
    /// concurrently-running second process (another `b00t-mcp --http --llm`, or
    /// a `b00t server key create` CLI invocation) never has its own key silently
    /// dropped by this process overwriting the file from its own stale view.
    /// See #1128. Locking is blocking I/O, so it runs off the async runtime via
    /// `spawn_blocking`.
    ///
    /// See #1208: previously swallowed the result entirely (`eprintln!` and
    /// return `()`) — a transient I/O failure on the write path (e.g. a full
    /// or unavailable filesystem) left the caller believing the key had been
    /// persisted when it hadn't. Now returns the error so `create_key` can
    /// surface and roll back on it, instead of that lost write staying
    /// invisible until the next process restart.
    async fn save_keys_to_file(&self) -> std::io::Result<()> {
        let keys_file = self.keys_file.clone();
        let in_memory = self.keys.read().await.clone();
        let result = tokio::task::spawn_blocking(move || write_keys_file_locked(&keys_file, &in_memory)).await;
        match result {
            Ok(Ok(())) => Ok(()),
            Ok(Err(e)) => {
                eprintln!("⚠️ save_keys_to_file: {e}");
                Err(e)
            }
            Err(e) => {
                eprintln!("⚠️ save_keys_to_file: blocking task panicked: {e}");
                Err(std::io::Error::other(format!("blocking task panicked: {e}")))
            }
        }
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

    let _permit = match &state.chat_semaphore {
        Some(sem) => Some(sem.clone().acquire_owned().await.expect("semaphore not closed")),
        None => None,
    };

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
    let _permit = match &state.chat_semaphore {
        Some(sem) => Some(sem.clone().acquire_owned().await.expect("semaphore not closed")),
        None => None,
    };

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
    let _permit = match &state.embeddings_semaphore {
        Some(sem) => Some(sem.clone().acquire_owned().await.expect("semaphore not closed")),
        None => None,
    };

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

    /// `tempfile::tempdir()` defaults to `std::env::temp_dir()`, which on some
    /// dev boxes is a size-limited `tmpfs` RAM disk (not real disk) — a
    /// plausible cause of #1208's one-off flaky failure (a transient write
    /// error under disk pressure, silently swallowed before the fix above).
    /// Root under this crate's own `target/` instead: real disk, and
    /// `CARGO_MANIFEST_DIR` is available at compile time for unit tests too
    /// (unlike `CARGO_TARGET_TMPDIR`, which Cargo only sets for integration
    /// test/bench binaries).
    fn temp_dir_base() -> std::path::PathBuf {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/tmp");
        let _ = std::fs::create_dir_all(&dir);
        dir
    }

    impl TempHome {
        fn new() -> Self {
            let guard = HOME_LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            let temp_dir = tempfile::Builder::new()
                .prefix("b00t-mcp-test-")
                .tempdir_in(temp_dir_base())
                .unwrap();
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
        let key = state.create_key("test-consumer", &[]).await.expect("key creation must succeed");
        assert!(key.starts_with("b00t-sk-"));
        let entry = state.validate_key(&key).await;
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().consumer, "test-consumer");
    }

    #[tokio::test]
    async fn concurrency_limit_serializes_requests_to_a_backend() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let _temp_home = TempHome::new();

        // Fake upstream that tracks the max number of simultaneously
        // in-flight requests it observed, via a slow handler that holds a
        // counter up for long enough to overlap if the semaphore didn't
        // actually serialize anything.
        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_observed = Arc::new(AtomicUsize::new(0));
        let concurrent_for_handler = concurrent.clone();
        let max_observed_for_handler = max_observed.clone();
        let app = Router::new().route(
            "/chat/completions",
            axum::routing::post(move |_body: Bytes| {
                let concurrent = concurrent_for_handler.clone();
                let max_observed = max_observed_for_handler.clone();
                async move {
                    let now = concurrent.fetch_add(1, Ordering::SeqCst) + 1;
                    max_observed.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                    concurrent.fetch_sub(1, Ordering::SeqCst);
                    Json(stop_upstream_response("ok"))
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let upstream_url = format!("http://{addr}");

        let state = Arc::new(LlmState::from_config_full_with_concurrency(
            &upstream_url, "", &upstream_url, "", Some(1), None,
        ));

        let request_body = json!({
            "model": "test-model",
            "messages": [{"role": "user", "content": "hi"}]
        });
        let body_bytes = Bytes::from(serde_json::to_vec(&request_body).unwrap());

        // Fire 3 concurrent requests through proxy_chat.
        let mut handles = Vec::new();
        for _ in 0..3 {
            let state = state.clone();
            let body = body_bytes.clone();
            handles.push(tokio::spawn(async move {
                proxy_chat(State((state, true)), HeaderMap::new(), body).await.into_response()
            }));
        }
        for h in handles {
            let resp = h.await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        assert_eq!(
            max_observed.load(Ordering::SeqCst),
            1,
            "max_concurrent=Some(1) must serialize requests to exactly 1 in flight at a time"
        );
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
        //
        // 🤓 #1113 follow-up: this test was missing the `TempHome` guard every
        // other test in this module has, so it ran against the REAL `$HOME`.
        // When it happened to overlap with another thread's TempHome-guarded
        // test (which temporarily repoints the process-wide `HOME` env var at
        // a temp dir, then deletes it on Drop), this test could construct its
        // `LlmState` mid-repoint and then fail its own `keys_file` write with
        // ENOENT once the other test's temp dir was gone — a real, reproduced
        // instance of exactly the order/concurrency-dependent flake class
        // #1113 investigated (the write below failed once, confirming this).
        let _temp_home = TempHome::new();
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

    /// Regression test for #1128: two `LlmState` instances sharing the same
    /// `keys_file` (simulating two live processes — e.g. two server instances,
    /// or this server plus a `b00t server key create` CLI invocation), each
    /// unaware of the other's key, both call `create_key`. Before the fix,
    /// the second instance's `save_keys_to_file` blindly overwrote the file
    /// with only its own in-memory map, silently dropping the first
    /// instance's key from disk. After the fix (locked read-merge-write),
    /// both keys must survive on disk.
    #[tokio::test]
    async fn test_concurrent_instances_do_not_lose_each_others_keys() {
        let _temp_home = TempHome::new();

        // Two instances constructed against the same (empty) keys_file — each
        // has its own independent in-memory `keys` map, exactly as two live
        // processes would.
        let instance_a = LlmState::from_config("http://localhost:8181/v1", "");
        let instance_b = LlmState::from_config("http://localhost:8181/v1", "");

        let key_a = instance_a.create_key("consumer-a", &[]).await.expect("instance A's key creation must succeed");
        let key_b = instance_b.create_key("consumer-b", &[]).await.expect("instance B's key creation must succeed");

        // Neither instance's own in-memory validate_key was affected by the
        // other — that was never the bug. The bug is what's actually on disk.
        let on_disk = load_keys_from_file(&instance_a.keys_file);
        assert!(
            on_disk.contains_key(&key_a),
            "instance A's key must survive on disk after instance B's later write"
        );
        assert!(on_disk.contains_key(&key_b), "instance B's key must be on disk");
        assert_eq!(on_disk.len(), 2, "both keys must coexist, neither overwritten");
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
            let key = state.create_key("consumer-in-first-home", &[]).await.expect("key creation must succeed");
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
    fn default_soul_includes_foundry_local_as_a_local_backend() {
        let soul = default_soul("test-host");
        let foundry = soul
            .backends
            .local
            .iter()
            .find(|b| b.name == "foundry-local")
            .expect("foundry-local must be a default local backend");
        assert_eq!(foundry.kind, "openai-compat");
        assert!(foundry.enabled);
        // Foundry Local is documented single-user, not built for concurrent
        // multi-client serving — default to serializing requests to it.
        assert_eq!(foundry.max_concurrent, Some(1));
        assert!(!foundry.models.is_empty(), "foundry-local should declare its known default model");
        assert_eq!(foundry.models[0].model_name, "phi-4-mini");
    }

    #[test]
    fn foundry_local_is_positioned_after_vllm_and_before_candle_phi() {
        // Preserves the existing fallback-priority ordering: real GPU/NPU
        // runtimes first, Foundry Local next (also hardware-accelerated when
        // present), candle-phi (CPU-only, ~0.53 tok/s) stays the absolute
        // last resort.
        let soul = default_soul("test-host");
        let names: Vec<&str> = soul.backends.local.iter().map(|b| b.name.as_str()).collect();
        let vllm_pos = names.iter().position(|n| *n == "vllm").unwrap();
        let foundry_pos = names.iter().position(|n| *n == "foundry-local").unwrap();
        let candle_pos = names.iter().position(|n| *n == "candle-phi").unwrap();
        assert!(vllm_pos < foundry_pos, "foundry-local must come after vllm");
        assert!(foundry_pos < candle_pos, "foundry-local must come before candle-phi");
    }

    #[test]
    fn parse_foundry_endpoint_extracts_http_url_from_cli_output() {
        // Real sample shape `foundry service status` produces (per the
        // proven ledgrrr port of this parser) — a human-readable line
        // containing a bare http:// URL among other text/punctuation.
        let raw = "Model management service is running on http://127.0.0.1:5273/\nSome other line.";
        let found = parse_foundry_endpoint(raw);
        assert_eq!(found.as_deref(), Some("http://127.0.0.1:5273"));
    }

    #[test]
    fn parse_foundry_endpoint_returns_none_when_no_url_present() {
        assert!(parse_foundry_endpoint("service is not running").is_none());
    }

    #[test]
    fn normalize_foundry_endpoint_strips_known_suffixes() {
        assert_eq!(normalize_foundry_endpoint("http://127.0.0.1:5273/"), "http://127.0.0.1:5273");
        assert_eq!(normalize_foundry_endpoint("http://127.0.0.1:5273/v1/chat/completions"), "http://127.0.0.1:5273");
        assert_eq!(normalize_foundry_endpoint("http://127.0.0.1:5273/v1"), "http://127.0.0.1:5273");
        assert_eq!(normalize_foundry_endpoint("http://127.0.0.1:5273/openai"), "http://127.0.0.1:5273");
    }

    #[test]
    fn local_backend_models_field_defaults_to_empty_and_roundtrips() {
        use ufo_types::{DataFormat, ModelCapability};

        // Default-constructed (as every existing default_soul() entry is)
        // gets an empty models list — this field is additive, not required.
        let soul = default_soul("test-host");
        let mistralrs = soul
            .backends
            .local
            .iter()
            .find(|b| b.name == "mistralrs")
            .expect("mistralrs must be a default local backend");
        assert!(mistralrs.models.is_empty());

        // A backend WITH models set roundtrips through TOML (the real
        // on-disk config format `SoulConfig::load` reads/writes).
        let mut backend = LocalBackend {
            name: "test-backend".into(),
            port: 9999,
            kind: "openai-compat".into(),
            enabled: true,
            models: Vec::new(),
            max_concurrent: None,
        };
        backend.models.push(
            ModelCapability::new("test-model", vec![DataFormat::Json, DataFormat::PlainText])
                .with_metadata("quantization", "int4"),
        );
        let toml_str = toml::to_string_pretty(&backend).unwrap();
        let back: LocalBackend = toml::from_str(&toml_str).unwrap();
        assert_eq!(back.models.len(), 1);
        assert_eq!(back.models[0].model_name, "test-model");
        assert_eq!(back.models[0].formats, vec![DataFormat::Json, DataFormat::PlainText]);
        assert_eq!(back.models[0].metadata.get("quantization"), Some(&"int4".to_string()));
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
