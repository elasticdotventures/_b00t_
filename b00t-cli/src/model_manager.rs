use crate::datum_ai_model::AiModelDatumEntry;
use crate::traits::DatumChecker; // 🦨 Fix: trait needed for is_installed() method
use crate::{check_command_available, get_expanded_path};
use anyhow::{Context, Result, anyhow};
use duct::cmd;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

const STATE_DIR: &str = "~/.b00t/models";
const ACTIVE_MODEL_FILE: &str = "active-model";
const DEFAULT_IMAGE: &str = "vllm/vllm-openai:latest";
const DEFAULT_DTYPE: &str = "float16";
const DEFAULT_PORT: u16 = 8000;

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
                .map(|name| name.ends_with(".ai_model.toml"))
                .unwrap_or(false)
            {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
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
            "No AI model datums were found under _b00t_. Add *.ai_model.toml files first."
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

pub fn describe_model(path: &str, name: Option<&str>) -> Result<ModelRecord> {
    let entry = select_model(path, name)?;
    let active = read_active_model()
        .map(|current| matches_model_name(&entry, &current))
        .unwrap_or(false);
    Ok(record_from_entry(&entry, active))
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
        entry.model.metadata.get("container_runtime").map(String::as_str)
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
    let model_arg = entry.model.metadata
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

    #[test]
    fn test_runtime_override_podman() {
        assert_eq!(detect_container_runtime(Some("podman")), ContainerRuntime::Podman); // output: Podman
    }

    #[test]
    fn test_runtime_override_docker() {
        assert_eq!(detect_container_runtime(Some("docker")), ContainerRuntime::Docker); // output: Docker
    }

    #[test]
    fn test_runtime_bin_names() {
        assert_eq!(runtime_bin(&ContainerRuntime::Docker), "docker");    // output: "docker"
        assert_eq!(runtime_bin(&ContainerRuntime::Podman), "podman");    // output: "podman"
    }

    #[test]
    fn test_shlex_split_vllm_extra_args() {
        // 🤓 Validates quoted JSON survives shlex split as a single token
        let args = r#"--max-model-len 32768 --override-generation-config '{"temperature": 0.25}'"#;
        let parsed = shlex::split(args).expect("valid shlex input");
        assert_eq!(parsed[0], "--max-model-len");         // output: "--max-model-len"
        assert_eq!(parsed[1], "32768");                   // output: "32768"
        assert_eq!(parsed[2], "--override-generation-config"); // output: flag
        assert_eq!(parsed[3], r#"{"temperature": 0.25}"#); // output: unquoted JSON
    }
}
