use crate::datum_ai_model::AiModelDatumEntry;
use crate::traits::DatumChecker; // 🦨 Fix: trait needed for is_installed() method
use crate::{check_command_available, get_expanded_path};
use anyhow::{Context, Result, anyhow};
use duct::cmd;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::net::IpAddr;
use std::path::PathBuf;
use std::time::Duration;
use url::Url;

const STATE_DIR: &str = "~/.b00t/models";
const ACTIVE_MODEL_FILE: &str = "active-model";
const DEFAULT_IMAGE: &str = "vllm/vllm-openai:latest";
const DEFAULT_DTYPE: &str = "float16";
const DEFAULT_PORT: u16 = 8000;
const MODEL_SUFFIXES: [&str; 2] = [".model.toml", ".ai_model.toml"];

/// Container runtime selection: prefer docker as safe default, with optional podman support.
/// Podman uses CDI spec: --device nvidia.com/gpu=all --security-opt=label=disable
/// Docker uses: --gpus all
#[derive(Debug, Clone, PartialEq)]
enum ContainerRuntime {
    Docker,
    Podman,
}

fn detect_container_runtime(override_hint: Option<&str>) -> ContainerRuntime {
    // 🤓 datum metadata `container_runtime = "podman"` takes precedence
    if let Some(hint) = override_hint {
        if hint.to_lowercase() == "podman" {
            return ContainerRuntime::Podman;
        }
        if hint.to_lowercase() == "docker" {
            return ContainerRuntime::Docker;
        }
    }
    // auto-detect: prefer docker as the safe default; fall back to podman if docker is unavailable
    if check_command_available("docker") {
        return ContainerRuntime::Docker;
    }
    if check_command_available("podman") {
        return ContainerRuntime::Podman;
    }
    // default to docker if neither is discoverable; caller will surface any runtime errors
    ContainerRuntime::Docker
}

fn runtime_bin(rt: &ContainerRuntime) -> &'static str {
    match rt {
        ContainerRuntime::Docker => "docker",
        ContainerRuntime::Podman => "podman",
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelRecord {
    pub name: String,
    pub hint: String,
    pub provider: String,
    pub size: String,
    pub capabilities: Vec<String>,
    pub repo: Option<String>,
    pub cache_dir: Option<String>,
    pub container_path: Option<String>,
    pub dtype: Option<String>,
    pub rpm_limit: Option<u32>,
    pub context_window: Option<u32>,
    pub installed: bool,
    pub active: bool,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelOperation {
    pub name: String,
    pub repo: Option<String>,
    pub cache_dir: Option<String>,
    pub activated: bool,
    pub downloaded: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelServeResult {
    pub container: String,
    pub port: u16,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ServedModelRecord {
    pub id: String,
    pub object: Option<String>,
    pub owned_by: Option<String>,
    pub created: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ServedEndpointRecord {
    pub base_url: String,
    pub source_models: Vec<String>,
    pub gpu: Option<String>,
    pub port: Option<u16>,
    pub models: Vec<ServedModelRecord>,
}

#[derive(Debug, Clone)]
struct ServedEndpointCandidate {
    base_url: String,
    api_key: Option<String>,
    source_models: BTreeSet<String>,
    gpu: Option<String>,
    port: Option<u16>,
}

#[derive(Debug, Clone)]
pub struct ServeOptions {
    pub dtype: Option<String>,
    pub port: Option<u16>,
    pub image: Option<String>,
    pub container_name: Option<String>,
    pub tensor_parallel_size: Option<u32>,
    pub extra_args: Vec<String>,
    pub gpus: bool,
    pub force_replace: bool,
}

impl Default for ServeOptions {
    fn default() -> Self {
        Self {
            dtype: None,
            port: None,
            image: None,
            container_name: None,
            tensor_parallel_size: Some(1),
            extra_args: Vec::new(),
            gpus: true,
            force_replace: true,
        }
    }
}

fn state_dir() -> PathBuf {
    PathBuf::from(shellexpand::tilde(STATE_DIR).to_string())
}

fn ensure_state_dir() -> Result<PathBuf> {
    let dir = state_dir();
    fs::create_dir_all(&dir)
        .with_context(|| format!("Failed to create model state directory {}", dir.display()))?;
    Ok(dir)
}

fn active_model_path() -> PathBuf {
    state_dir().join(ACTIVE_MODEL_FILE)
}

fn read_active_model() -> Option<String> {
    fs::read_to_string(active_model_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn write_active_model(name: &str) -> Result<()> {
    let path = active_model_path();
    ensure_state_dir()?;
    fs::write(&path, format!("{}\n", name))
        .with_context(|| format!("Failed to persist active model to {}", path.display()))
}

fn clear_active_model_if(name: &str) -> Result<()> {
    if read_active_model()
        .map(|current| current == name)
        .unwrap_or(false)
    {
        let path = active_model_path();
        if path.exists() {
            fs::remove_file(&path)
                .with_context(|| format!("Failed to clear active model file {}", path.display()))?;
        }
    }
    Ok(())
}

fn enumerate_model_files(base_path: &str) -> Result<Vec<PathBuf>> {
    let dir = get_expanded_path(base_path)?;
    let mut files = Vec::new();
    if dir.exists() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path
                .file_name()
                .and_then(|s| s.to_str())
                .map(is_model_filename)
                .unwrap_or(false)
            {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn is_model_filename(name: &str) -> bool {
    MODEL_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

fn load_models(base_path: &str) -> Result<Vec<AiModelDatumEntry>> {
    enumerate_model_files(base_path)?
        .into_iter()
        .map(|path| AiModelDatumEntry::from_file(&path))
        .collect()
}

fn matches_model_name(entry: &AiModelDatumEntry, query: &str) -> bool {
    if entry.datum.name == query {
        return true;
    }
    entry
        .datum
        .aliases
        .as_ref()
        .map(|aliases| aliases.iter().any(|alias| alias == query))
        .unwrap_or(false)
}

fn select_model<'a>(base_path: &str, requested: Option<&'a str>) -> Result<AiModelDatumEntry> {
    let models = load_models(base_path)?;
    if models.is_empty() {
        anyhow::bail!(
            "No AI model datums were found under _b00t_. Add *.model.toml or *.ai_model.toml files first."
        );
    }

    if let Some(name) = requested {
        return models
            .into_iter()
            .find(|m| matches_model_name(m, name))
            .ok_or_else(|| anyhow!("Model '{}' not found", name));
    }

    if let Some(active) = read_active_model() {
        if let Some(index) = models.iter().position(|m| matches_model_name(m, &active)) {
            return Ok(models.into_iter().nth(index).unwrap());
        }
    }

    Ok(models
        .into_iter()
        .next()
        .expect("models collection cannot be empty here"))
}

fn record_from_entry(entry: &AiModelDatumEntry, active: bool) -> ModelRecord {
    let capabilities = entry
        .model
        .capabilities
        .iter()
        .map(|c| format!("{:?}", c).to_lowercase())
        .collect::<Vec<_>>();

    let aliases = entry.datum.aliases.clone().unwrap_or_else(Vec::new);

    ModelRecord {
        name: entry.datum.name.clone(),
        hint: entry.datum.hint.clone(),
        provider: format!("{:?}", entry.model.provider).to_lowercase(),
        size: format!("{:?}", entry.model.size).to_lowercase(),
        capabilities,
        repo: entry.huggingface_repo(),
        cache_dir: entry.cache_dir().map(|p| p.display().to_string()),
        container_path: entry.container_path(),
        dtype: entry.dtype(),
        rpm_limit: entry.model.rpm_limit,
        context_window: entry.model.context_window,
        installed: entry.is_installed(),
        active,
        aliases,
    }
}

pub fn list_models(path: &str) -> Result<Vec<ModelRecord>> {
    let active = read_active_model();
    let models = load_models(path)?
        .into_iter()
        .map(|entry| {
            let is_active = active
                .as_ref()
                .map(|name| matches_model_name(&entry, name))
                .unwrap_or(false);
            record_from_entry(&entry, is_active)
        })
        .collect();
    Ok(models)
}

pub async fn list_served_models(path: &str) -> Result<Vec<ServedEndpointRecord>> {
    let candidates = discover_served_endpoint_candidates(path)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .context("Failed to construct local inference HTTP client")?;

    let mut endpoints = Vec::new();
    let mut probe_errors = Vec::new();

    for candidate in candidates {
        match fetch_served_endpoint(&client, &candidate).await {
            Ok(record) => {
                if !record.models.is_empty() {
                    endpoints.push(record);
                }
            }
            Err(err) => {
                probe_errors.push((candidate.base_url.clone(), err.to_string()));
            }
        }
    }

    for (base_url, err) in &probe_errors {
        eprintln!(
            "warning: failed to probe served model endpoint {}: {}",
            base_url, err
        );
    }
    endpoints.sort_by(|left, right| left.base_url.cmp(&right.base_url));
    Ok(endpoints)
}

pub fn describe_model(path: &str, name: Option<&str>) -> Result<ModelRecord> {
    let entry = select_model(path, name)?;
    let active = read_active_model()
        .map(|current| matches_model_name(&entry, &current))
        .unwrap_or(false);
    Ok(record_from_entry(&entry, active))
}

fn discover_served_endpoint_candidates(path: &str) -> Result<Vec<ServedEndpointCandidate>> {
    let mut candidates: BTreeMap<String, ServedEndpointCandidate> = BTreeMap::new();

    for entry in load_models(path)? {
        let urls = local_base_urls_for_entry(&entry);
        let api_key = api_key_for_entry(&entry);
        let gpu = entry.model.metadata.get("gpu").cloned();
        let metadata_port = entry
            .model
            .metadata
            .get("port")
            .and_then(|value| value.parse::<u16>().ok());

        for base_url in urls {
            let candidate =
                candidates
                    .entry(base_url.clone())
                    .or_insert_with(|| ServedEndpointCandidate {
                        base_url: base_url.clone(),
                        api_key: api_key.clone(),
                        source_models: BTreeSet::new(),
                        gpu: gpu.clone(),
                        port: port_from_base_url(&base_url).or(metadata_port),
                    });
            candidate.source_models.insert(entry.datum.name.clone());
            if candidate.api_key.is_none() {
                candidate.api_key = api_key.clone();
            }
            if candidate.gpu.is_none() {
                candidate.gpu = gpu.clone();
            }
            if candidate.port.is_none() {
                candidate.port = port_from_base_url(&base_url).or(metadata_port);
            }
        }
    }

    for (name, value) in local_env_base_urls() {
        let candidate =
            candidates
                .entry(value.clone())
                .or_insert_with(|| ServedEndpointCandidate {
                    base_url: value.clone(),
                    api_key: None,
                    source_models: BTreeSet::new(),
                    gpu: None,
                    port: port_from_base_url(&value),
                });
        candidate.source_models.insert(format!("env:{}", name));
    }

    Ok(candidates.into_values().collect())
}

async fn fetch_served_endpoint(
    client: &reqwest::Client,
    candidate: &ServedEndpointCandidate,
) -> Result<ServedEndpointRecord> {
    let url = format!("{}/models", candidate.base_url.trim_end_matches('/'));
    let mut request = client.get(url);
    if let Some(api_key) = &candidate.api_key {
        request = request.bearer_auth(api_key);
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("Failed to reach {}", candidate.base_url))?
        .error_for_status()
        .with_context(|| {
            format!(
                "Local inference endpoint {} returned an error",
                candidate.base_url
            )
        })?;
    let body = response
        .text()
        .await
        .with_context(|| format!("Failed to read {}", candidate.base_url))?;

    Ok(ServedEndpointRecord {
        base_url: candidate.base_url.clone(),
        source_models: candidate.source_models.iter().cloned().collect(),
        gpu: candidate.gpu.clone(),
        port: candidate
            .port
            .or_else(|| port_from_base_url(&candidate.base_url)),
        models: parse_served_models_response(&body)?,
    })
}

fn parse_served_models_response(body: &str) -> Result<Vec<ServedModelRecord>> {
    let payload: Value =
        serde_json::from_str(body).context("Failed to parse /v1/models response")?;
    let items = payload
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| payload.get("models").and_then(Value::as_array))
        .ok_or_else(|| anyhow!("Response did not contain a model list"))?;

    let mut models = Vec::new();
    for item in items {
        match item {
            Value::String(id) => models.push(ServedModelRecord {
                id: id.clone(),
                object: None,
                owned_by: None,
                created: None,
            }),
            Value::Object(map) => {
                let id = map
                    .get("id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| anyhow!("Model entry missing string id"))?;
                models.push(ServedModelRecord {
                    id: id.to_string(),
                    object: map
                        .get("object")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    owned_by: map
                        .get("owned_by")
                        .and_then(Value::as_str)
                        .map(ToString::to_string),
                    created: map.get("created").and_then(Value::as_i64),
                });
            }
            _ => return Err(anyhow!("Unsupported model entry in /v1/models response")),
        }
    }

    models.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(models)
}

fn local_base_urls_for_entry(entry: &AiModelDatumEntry) -> Vec<String> {
    let mut urls = BTreeSet::new();

    if let Some(api_base) = &entry.model.api_base {
        if let Some(normalized) = normalize_local_base_url(api_base) {
            urls.insert(normalized);
        }
    }

    if let Some(env_map) = &entry.datum.env {
        for (key, value) in env_map {
            if is_base_url_env_key(key) {
                if let Some(normalized) = normalize_local_base_url(value) {
                    urls.insert(normalized);
                }
            }
        }
    }

    urls.into_iter().collect()
}

fn local_env_base_urls() -> Vec<(String, String)> {
    let mut urls = Vec::new();
    for key in [
        "B00T_AI_FRONTIER_BASE",
        "B00T_AI_CH0NKY_BASE",
        "B00T_AI_SM0L_BASE",
        "OPENAI_BASE_URL",
        "MISTRALRS_API_BASE",
        "GEMMA4_API_BASE",
        "PI_BASE_URL",
        "LLAMA_CPP_BASE_URL",
    ] {
        if let Ok(value) = env::var(key) {
            if let Some(normalized) = normalize_local_base_url(&value) {
                urls.push((key.to_string(), normalized));
            }
        }
    }
    urls.sort();
    urls.dedup();
    urls
}

fn api_key_for_entry(entry: &AiModelDatumEntry) -> Option<String> {
    let env_map = entry.datum.env.as_ref();

    if let Some(key_name) = entry.model.api_key_env.as_deref() {
        if let Some(value) = env_map.and_then(|map| map.get(key_name)).cloned() {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
        if let Ok(value) = env::var(key_name) {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }

    for key in ["OPENAI_API_KEY", "VLLM_API_KEY", "MISTRALRS_API_KEY"] {
        if let Some(value) = env_map.and_then(|map| map.get(key)).cloned() {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
        if let Ok(value) = env::var(key) {
            if !value.trim().is_empty() {
                return Some(value);
            }
        }
    }

    None
}

fn is_base_url_env_key(key: &str) -> bool {
    key == "OPENAI_BASE_URL" || key.ends_with("_API_BASE") || key.ends_with("_BASE_URL")
}

fn normalize_local_base_url(raw: &str) -> Option<String> {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    let trimmed = trimmed.strip_suffix("/models").unwrap_or(trimmed);
    let parsed = Url::parse(trimmed).ok()?;
    if !is_local_host(&parsed) {
        return None;
    }

    Some(parsed.to_string().trim_end_matches('/').to_string())
}

fn is_local_host(url: &Url) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };

    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }

    host.parse::<IpAddr>()
        .map(|ip| ip.is_loopback() || ip.is_unspecified())
        .unwrap_or(false)
}

fn port_from_base_url(base_url: &str) -> Option<u16> {
    Url::parse(base_url)
        .ok()
        .and_then(|url| url.port_or_known_default())
}

pub fn export_model_env(path: &str, name: Option<&str>) -> Result<Vec<(String, String)>> {
    let entry = select_model(path, name)?;
    let mut envs: BTreeMap<String, String> = BTreeMap::new();

    if let Some(map) = &entry.datum.env {
        for (key, value) in map {
            if key == "VLLM_MODEL_DIR" {
                envs.insert(key.clone(), shellexpand::tilde(value).to_string());
            } else {
                envs.insert(key.clone(), value.clone());
            }
        }
    }

    envs.insert("B00T_MODEL_ID".to_string(), entry.datum.name.clone());

    if let Some(repo) = entry.huggingface_repo() {
        envs.insert("B00T_MODEL_REPO".to_string(), repo.clone());
        envs.entry("VLLM_MODEL_REPO".to_string()).or_insert(repo);
    }

    if let Some(dtype) = entry.dtype() {
        envs.entry("VLLM_DTYPE".to_string()).or_insert(dtype);
    }

    envs.insert(
        "B00T_MODEL_PROVIDER".to_string(),
        format!("{:?}", entry.model.provider).to_lowercase(),
    );

    let caps = entry
        .model
        .capabilities
        .iter()
        .map(|c| format!("{:?}", c).to_lowercase())
        .collect::<Vec<_>>();
    envs.insert("B00T_MODEL_CAPABILITIES".to_string(), caps.join(","));

    Ok(envs.into_iter().collect())
}

pub fn download_model(
    path: &str,
    name: &str,
    force: bool,
    activate: bool,
) -> Result<ModelOperation> {
    let entry = select_model(path, Some(name))?;

    if !force && entry.is_installed() {
        if activate {
            write_active_model(&entry.datum.name)?;
        }
        return Ok(ModelOperation {
            name: entry.datum.name.clone(),
            repo: entry.huggingface_repo(),
            cache_dir: entry.cache_dir().map(|p| p.display().to_string()),
            activated: activate,
            downloaded: false,
        });
    }

    if !check_command_available("huggingface-cli") {
        anyhow::bail!("huggingface-cli not found. Run 'b00t-cli cli install huggingface' first.");
    }

    let repo = entry
        .huggingface_repo()
        .ok_or_else(|| anyhow!("Model '{}' does not declare a Hugging Face repo", name))?;

    let cache_dir = entry.cache_dir().ok_or_else(|| {
        anyhow!(
            "Model '{}' is missing VLLM_MODEL_DIR or metadata cache_dir",
            name
        )
    })?;
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("Failed to create cache directory {}", cache_dir.display()))?;

    let mut args = vec![
        "download".to_string(),
        repo.clone(),
        "--local-dir".to_string(),
        cache_dir.display().to_string(),
        "--local-dir-use-symlinks".to_string(),
        "False".to_string(),
    ];

    if let Some(revision) = entry.model.metadata.get("revision") {
        if !revision.is_empty() {
            args.push("--revision".to_string());
            args.push(revision.clone());
        }
    }

    if let Some(include) = entry.model.metadata.get("allow_patterns") {
        if !include.is_empty() {
            args.push("--allow-patterns".to_string());
            args.push(include.clone());
        }
    }

    if let Some(exclude) = entry.model.metadata.get("ignore_patterns") {
        if !exclude.is_empty() {
            args.push("--ignore-patterns".to_string());
            args.push(exclude.clone());
        }
    }

    cmd("huggingface-cli", &args)
        .run()
        .with_context(|| format!("huggingface-cli download failed for {}", repo))?;

    if activate {
        write_active_model(&entry.datum.name)?;
    }

    Ok(ModelOperation {
        name: entry.datum.name.clone(),
        repo: Some(repo),
        cache_dir: Some(cache_dir.display().to_string()),
        activated: activate,
        downloaded: true,
    })
}

pub fn remove_model(path: &str, name: &str) -> Result<Option<String>> {
    let entry = select_model(path, Some(name))?;
    let cache_dir = match entry.cache_dir() {
        Some(dir) => dir,
        None => return Ok(None),
    };

    if cache_dir.exists() {
        fs::remove_dir_all(&cache_dir)
            .with_context(|| format!("Failed to remove cache directory {}", cache_dir.display()))?;
    }
    clear_active_model_if(&entry.datum.name)?;
    Ok(Some(cache_dir.display().to_string()))
}

pub fn activate_model(path: &str, name: &str) -> Result<()> {
    let entry = select_model(path, Some(name))?;
    write_active_model(&entry.datum.name)
}

pub fn serve_model(
    path: &str,
    name: Option<&str>,
    mut options: ServeOptions,
) -> Result<ModelServeResult> {
    let entry = select_model(path, name)?;

    if !entry.is_installed() {
        anyhow::bail!(
            "Model '{}' is not cached. Run b00t-cli model download {} first.",
            entry.datum.name,
            entry.datum.name
        );
    }

    let env = export_model_env(path, Some(&entry.datum.name))?;
    let env_map: BTreeMap<_, _> = env.into_iter().collect();

    let cache_dir = entry
        .cache_dir()
        .ok_or_else(|| anyhow!("Model '{}' is missing cache dir metadata", entry.datum.name))?;
    let container_path = entry.container_path().ok_or_else(|| {
        anyhow!(
            "Model '{}' is missing container mount metadata",
            entry.datum.name
        )
    })?;

    let dtype = options
        .dtype
        .take()
        .or_else(|| env_map.get("VLLM_DTYPE").cloned())
        .unwrap_or_else(|| DEFAULT_DTYPE.to_string());

    let port = options.port.unwrap_or(DEFAULT_PORT);
    let image = options
        .image
        .clone()
        .unwrap_or_else(|| DEFAULT_IMAGE.to_string());

    let container = options
        .container_name
        .clone()
        .unwrap_or_else(|| format!("vllm-{}", entry.datum.name.replace('/', "-")));

    // 🤓 container_runtime datum metadata drives podman vs docker selection
    let runtime = detect_container_runtime(
        entry
            .model
            .metadata
            .get("container_runtime")
            .map(String::as_str),
    );
    let bin = runtime_bin(&runtime);

    if options.force_replace {
        let _ = cmd(bin, &["rm", "-f", &container]).run();
    }

    // Container isolation controls (defaults preserve existing behavior).
    // Hardened deployments can override via model metadata:
    // - container_ipc_host="false" to avoid --ipc=host
    // - podman_disable_selinux_label="false" to keep SELinux labeling
    let use_host_ipc = entry
        .model
        .metadata
        .get("container_ipc_host")
        .map(|v| v != "false")
        .unwrap_or(true);
    let disable_selinux_label = entry
        .model
        .metadata
        .get("podman_disable_selinux_label")
        .map(|v| v != "false")
        .unwrap_or(true);

    let mut run_args = vec![
        "run".to_string(),
        "--rm".to_string(),
        "-d".to_string(),
        "--name".to_string(),
        container.clone(),
    ];

    if use_host_ipc {
        // Sharing host IPC namespace weakens container isolation.
        // To disable this, set container_ipc_host="false" in model metadata.
        eprintln!(
            "[b00t:model-manager] Warning: using --ipc=host for container `{}` (set container_ipc_host=\"false\" in model metadata to disable).",
            container
        );
        run_args.push("--ipc=host".to_string());
    }

    if options.gpus {
        match runtime {
            ContainerRuntime::Podman => {
                // CDI-based GPU passthrough (nvidia-ctk cdi generate --output /etc/cdi/nvidia.yaml)
                run_args.push("--device".to_string());
                run_args.push("nvidia.com/gpu=all".to_string());
                if disable_selinux_label {
                    // Disabling SELinux labeling weakens container confinement.
                    // To keep SELinux labeling, set podman_disable_selinux_label="false" in model metadata.
                    eprintln!(
                        "[b00t:model-manager] Warning: disabling SELinux labeling for Podman GPU container `{}` (set podman_disable_selinux_label=\"false\" in model metadata to keep labels).",
                        container
                    );
                    run_args.push("--security-opt=label=disable".to_string());
                }
            }
            ContainerRuntime::Docker => {
                run_args.push("--gpus".to_string());
                run_args.push("all".to_string());
            }
        }
    }

    run_args.push("-p".to_string());
    run_args.push(format!("{}:8000", port));
    run_args.push("-v".to_string());
    run_args.push(format!("{}:{}:ro", cache_dir.display(), container_path));

    if let Ok(token) = std::env::var("HF_TOKEN") {
        run_args.push("-e".to_string());
        run_args.push(format!("HF_TOKEN={}", token));
    }

    run_args.push(image);

    // 🤓 vllm_model_arg overrides container_path for GGUF files (full in-container path)
    let model_arg = entry
        .model
        .metadata
        .get("vllm_model_arg")
        .cloned()
        .unwrap_or_else(|| container_path.clone());
    run_args.push("--model".to_string());
    run_args.push(model_arg);

    run_args.push("--dtype".to_string());
    run_args.push(dtype);
    run_args.push("--tensor-parallel-size".to_string());
    run_args.push(options.tensor_parallel_size.unwrap_or(1).to_string());

    // datum-level extra vllm args (e.g. --max-model-len, --enable-chunked-prefill)
    if let Some(extra) = entry.model.metadata.get("vllm_extra_args") {
        // 🤓 shlex::split handles quoted args correctly (e.g. --override-generation-config '{"k":"v"}')
        if let Some(parsed) = shlex::split(extra) {
            run_args.extend(parsed);
        } else {
            return Err(anyhow!(
                "Invalid shell quoting in metadata key 'vllm_extra_args': {}",
                extra
            ));
        }
    }

    run_args.extend(options.extra_args);

    cmd(bin, &run_args)
        .run()
        .with_context(|| format!("Failed to start vLLM {} container {}", bin, container))?;

    Ok(ModelServeResult { container, port })
}

pub fn stop_model(path: &str, container_name: Option<&str>) -> Result<()> {
    if let Some(name) = container_name {
        // When an explicit container name is provided, we don't know which runtime
        // created it. Try Podman first, then Docker, and only error if both fail.
        let target = name.to_string();

        let podman_rt = ContainerRuntime::Podman;
        let podman_bin = runtime_bin(&podman_rt);
        let podman_result = cmd(podman_bin, &["rm", "-f", &target]).run();
        if podman_result.is_ok() {
            return Ok(());
        }

        let docker_rt = ContainerRuntime::Docker;
        let docker_bin = runtime_bin(&docker_rt);
        let docker_result = cmd(docker_bin, &["rm", "-f", &target]).run();
        if docker_result.is_ok() {
            return Ok(());
        }

        let podman_err = podman_result.err();
        let docker_err = docker_result.err();

        return Err(anyhow!(
            "Failed to stop container {} with either podman or docker (podman error: {:?}, docker error: {:?})",
            target,
            podman_err,
            docker_err
        ));
    } else {
        let entry = select_model(path, None)?;
        let runtime = detect_container_runtime(
            entry
                .model
                .metadata
                .get("container_runtime")
                .map(String::as_str),
        );
        let target = format!("vllm-{}", entry.datum.name.replace('/', "-"));

        let bin = runtime_bin(&runtime);
        cmd(bin, &["rm", "-f", &target])
            .run()
            .with_context(|| format!("Failed to stop {} container {}", bin, target))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;
    use std::io::Write;
    use std::net::TcpListener;
    use std::thread;
    use std::time::Instant;
    use tempfile::tempdir;

    #[test]
    fn test_runtime_override_podman() {
        assert_eq!(
            detect_container_runtime(Some("podman")),
            ContainerRuntime::Podman
        ); // output: Podman
    }

    #[test]
    fn test_runtime_override_docker() {
        assert_eq!(
            detect_container_runtime(Some("docker")),
            ContainerRuntime::Docker
        ); // output: Docker
    }

    #[test]
    fn test_runtime_bin_names() {
        assert_eq!(runtime_bin(&ContainerRuntime::Docker), "docker"); // output: "docker"
        assert_eq!(runtime_bin(&ContainerRuntime::Podman), "podman"); // output: "podman"
    }

    #[test]
    fn test_shlex_split_vllm_extra_args() {
        // 🤓 Validates quoted JSON survives shlex split as a single token
        let args = r#"--max-model-len 32768 --override-generation-config '{"temperature": 0.25}'"#;
        let parsed = shlex::split(args).expect("valid shlex input");
        assert_eq!(parsed[0], "--max-model-len"); // output: "--max-model-len"
        assert_eq!(parsed[1], "32768"); // output: "32768"
        assert_eq!(parsed[2], "--override-generation-config"); // output: flag
        assert_eq!(parsed[3], r#"{"temperature": 0.25}"#); // output: unquoted JSON
    }

    #[test]
    fn test_enumerate_model_files_accepts_current_and_legacy_suffixes() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("qwen3-coder-local.model.toml"), "").unwrap();
        fs::write(dir.path().join("legacy.ai_model.toml"), "").unwrap();
        fs::write(dir.path().join("ignore.txt"), "").unwrap();

        let files = enumerate_model_files(dir.path().to_str().unwrap()).unwrap();
        let names = files
            .iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec!["legacy.ai_model.toml", "qwen3-coder-local.model.toml"]
        );
    }

    #[test]
    fn test_parses_openai_models_fixture() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/openai_models_response.json");
        let mut body = String::new();
        fs::File::open(fixture_path)
            .unwrap()
            .read_to_string(&mut body)
            .unwrap();

        let models = parse_served_models_response(&body).unwrap();
        let ids = models.into_iter().map(|model| model.id).collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                "Qwen/Qwen3-Coder-30B-A3B-Instruct".to_string(),
                "qwen3-coder-local-alias".to_string()
            ]
        );
    }

    #[test]
    fn test_filters_non_local_base_urls() {
        assert!(normalize_local_base_url("http://127.0.0.1:8000/v1").is_some());
        assert!(normalize_local_base_url("http://0.0.0.0:8000/v1/").is_some());
        assert!(normalize_local_base_url("https://example.com/v1").is_none());
    }

    #[test]
    fn test_list_served_models_queries_local_endpoint() {
        let fixture_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/openai_models_response.json");
        let response_body = fs::read_to_string(fixture_path).unwrap();
        let base_url = spawn_models_server(response_body);
        let dir = tempdir().unwrap();

        let datum = format!(
            r#"[b00t]
name = "qwen3-coder-local"
type = "model"
hint = "Qwen local"

[b00t.env]
OPENAI_BASE_URL = "{base_url}"
OPENAI_API_KEY = "local-vllm"

[ai_model]
provider = "openai_compatible"
size = "large"
capabilities = ["chat", "code"]
litellm_model = "openai/qwen3-coder"
api_base = "{base_url}"
api_key_env = "OPENAI_API_KEY"

[ai_model.metadata]
gpu = "rtx3090"
"#
        );
        fs::write(dir.path().join("qwen3-coder-local.model.toml"), datum).unwrap();

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let endpoints = runtime
            .block_on(list_served_models(dir.path().to_str().unwrap()))
            .unwrap();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].base_url, base_url);
        assert_eq!(endpoints[0].gpu.as_deref(), Some("rtx3090"));
        assert_eq!(
            endpoints[0]
                .models
                .iter()
                .map(|model| model.id.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Qwen/Qwen3-Coder-30B-A3B-Instruct",
                "qwen3-coder-local-alias"
            ]
        );
    }

    fn spawn_models_server(response_body: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock models server");
        listener
            .set_nonblocking(true)
            .expect("configure mock models server as non-blocking");
        let address = listener.local_addr().expect("mock models server addr");

        thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(5);

            loop {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let mut buffer = [0_u8; 1024];
                        let _ = stream.read(&mut buffer);

                        let response = format!(
                            "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                            response_body.len(),
                            response_body
                        );
                        stream
                            .write_all(response.as_bytes())
                            .expect("write mock models response");
                        stream.flush().expect("flush mock models response");
                        return;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return;
                        }
                        thread::sleep(Duration::from_millis(50));
                    }
                    Err(error) => panic!("accept mock models connection: {error}"),
                }
            }
        });

        format!("http://{}/v1", address)
    }
}
