//! Multi-provider compute abstraction — inference endpoints + training jobs.
//!
//! Providers: runpod (native crate), hf (CLI wrapper for `hf jobs`), local (podman)
//! Single source of truth: PROVIDER-*.provider.tomllmd datums
//!
//! b00t provider endpoint deploy|status|teardown|list --provider runpod|hf
//! b00t provider job submit|status|cancel|list      --provider runpod|hf
//! b00t provider job submit-batch|status|cancel|list --provider runpod|hf|local

use crate::hive::SystemSnapshot;
use crate::model_manager::{detect_container_runtime, runtime_bin, ContainerRuntime};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::process::Command;

/// Minimum free VRAM (MB) required before a local batch job is allowed to start.
/// Matches the gate style already used by `[b00t.hive.resources.gate]` profiles.
const LOCAL_GPU_FREE_MB_GATE: u32 = 4000;

// ── Public types ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointConfig {
    pub name: String,
    /// Ordered preference list of RunPod gpu_type_id strings
    pub gpu_type_ids: Vec<String>,
    pub workers_min: i32,
    pub workers_max: i32,
    pub idle_timeout_s: i32,
    pub execution_timeout_ms: i32,
    /// Docker image for the inference worker
    pub image: String,
    /// Optional RunPod network volume holding adapter weights
    pub network_volume_id: Option<String>,
    pub env: std::collections::HashMap<String, String>,
}

impl Default for EndpointConfig {
    fn default() -> Self {
        Self {
            name: "b00t-ch0nky".into(),
            gpu_type_ids: vec!["NVIDIA A40".into(), "NVIDIA RTX A6000".into()],
            workers_min: 0,
            workers_max: 3,
            idle_timeout_s: 5,
            execution_timeout_ms: 30_000,
            image: "vllm/vllm-openai:latest".into(),
            network_volume_id: None,
            env: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHandle {
    pub id: String,
    pub provider: String,
    pub name: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrainingJobSpec {
    pub config_path: String,
    pub image: String,
    pub flavor: String,
    pub timeout_hours: f32,
}

/// A generic containerized batch job — the image carries its own entrypoint,
/// unlike `TrainingJobSpec` which always drives a fine-tuning script.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatchJobSpec {
    pub image: String,
    /// Local file path or provider-specific URI (e.g. `hf://...`); each
    /// provider decides how to make this reachable inside the job.
    pub config_path: String,
    pub env: std::collections::HashMap<String, String>,
    pub flavor: String,
    pub timeout_hours: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobHandle {
    pub id: String,
    pub provider: String,
}

// ── Trait ────────────────────────────────────────────────────────────────────

#[async_trait]
pub trait ComputeProvider: Send + Sync {
    fn name(&self) -> &str;

    async fn deploy_inference_endpoint(&self, cfg: &EndpointConfig) -> Result<EndpointHandle>;
    async fn endpoint_status(&self, id: &str) -> Result<EndpointHandle>;
    async fn teardown_endpoint(&self, id: &str) -> Result<()>;
    async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>>;

    async fn submit_training_job(&self, spec: &TrainingJobSpec) -> Result<JobHandle>;
    async fn submit_batch_job(&self, spec: &BatchJobSpec) -> Result<JobHandle>;
    async fn job_status(&self, handle: &JobHandle) -> Result<String>;
    async fn cancel_job(&self, handle: &JobHandle) -> Result<()>;
    async fn list_jobs(&self) -> Result<Vec<JobHandle>>;
}

pub fn get_provider(name: &str) -> Result<Box<dyn ComputeProvider>> {
    match name {
        "runpod" => Ok(Box::new(RunpodProvider::new()?)),
        "hf" => Ok(Box::new(HfProvider::new())),
        "local" => Ok(Box::new(LocalProvider::new())),
        other => bail!("unknown provider '{}'; supported: runpod, hf, local", other),
    }
}

// ── RunPod provider ──────────────────────────────────────────────────────────

pub struct RunpodProvider {
    client: runpod_sdk::RunpodClient,
}

impl RunpodProvider {
    pub fn new() -> Result<Self> {
        let config = runpod_sdk::RunpodConfig::from_env()
            .context("RUNPOD_API_KEY not set — see PROVIDER-RUNPOD.provider.tomllmd")?;
        Ok(Self {
            client: runpod_sdk::RunpodClient::new(config)
                .context("RunpodClient::new failed")?,
        })
    }
}

#[async_trait]
impl ComputeProvider for RunpodProvider {
    fn name(&self) -> &str {
        "runpod"
    }

    async fn deploy_inference_endpoint(&self, cfg: &EndpointConfig) -> Result<EndpointHandle> {
        use runpod_sdk::model::EndpointCreateInput;
        use runpod_sdk::service::EndpointsService;
        // 🤓 EndpointCreateInput requires template_id; env is baked into the template
        let template_id = cfg
            .env
            .get("RUNPOD_TEMPLATE_ID")
            .cloned()
            .unwrap_or_default();
        let input = EndpointCreateInput {
            template_id,
            name: Some(cfg.name.clone()),
            workers_min: Some(cfg.workers_min),
            workers_max: Some(cfg.workers_max),
            idle_timeout: Some(cfg.idle_timeout_s as i32),
            execution_timeout_ms: Some(cfg.execution_timeout_ms as i32),
            network_volume_id: cfg.network_volume_id.clone(),
            ..Default::default()
        };
        let endpoint = self
            .client
            .create_endpoint(input)
            .await
            .context("RunPod create_endpoint failed")?;
        Ok(EndpointHandle {
            id: endpoint.id,
            provider: "runpod".into(),
            name: endpoint.name,
            status: None,
        })
    }

    async fn endpoint_status(&self, id: &str) -> Result<EndpointHandle> {
        use runpod_sdk::service::EndpointsService;
        use runpod_sdk::model::GetEndpointQuery;
        let endpoint = self
            .client
            .get_endpoint(id, GetEndpointQuery::default())
            .await
            .context("RunPod get_endpoint failed")?;
        Ok(EndpointHandle {
            id: endpoint.id,
            provider: "runpod".into(),
            name: endpoint.name,
            status: None,
        })
    }

    async fn teardown_endpoint(&self, id: &str) -> Result<()> {
        use runpod_sdk::service::EndpointsService;
        self.client
            .delete_endpoint(id)
            .await
            .context("RunPod delete_endpoint failed")
    }

    async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
        use runpod_sdk::service::EndpointsService;
        use runpod_sdk::model::ListEndpointsQuery;
        let endpoints = self
            .client
            .list_endpoints(ListEndpointsQuery::default())
            .await
            .context("RunPod list_endpoints failed")?;
        Ok(endpoints
            .into_iter()
            .map(|e| EndpointHandle {
                id: e.id,
                provider: "runpod".into(),
                name: e.name,
                status: None,
            })
            .collect())
    }

    async fn submit_training_job(&self, spec: &TrainingJobSpec) -> Result<JobHandle> {
        use runpod_sdk::model::{CloudType, GpuTypeId, PodCreateInput};
        use runpod_sdk::service::PodsService;
        let gpu_str = hf_flavor_to_runpod_gpu(&spec.flavor);
        let gpu_id: GpuTypeId = serde_json::from_value(serde_json::Value::String(gpu_str.to_string()))
            .with_context(|| format!("unknown GPU type '{gpu_str}'"))?;
        let env: std::collections::HashMap<String, String> = [
            ("TRAINING_CONFIG".to_string(), spec.config_path.clone()),
            ("UNSLOTH_CACHE_DIR".to_string(), "/opt/unsloth_compiled_cache".to_string()),
        ].into();
        let req = PodCreateInput {
            name: Some("b00t-training".into()),
            image_name: Some(spec.image.clone()),
            gpu_type_ids: Some(vec![gpu_id]),
            cloud_type: Some(CloudType::Secure),
            gpu_count: Some(1),
            volume_in_gb: Some(50),
            container_disk_in_gb: Some(20),
            env: Some(env),
            ..Default::default()
        };
        let pod = self.client.create_pod(req).await.context("RunPod create_pod failed")?;
        Ok(JobHandle { id: pod.id, provider: "runpod".into() })
    }

    async fn submit_batch_job(&self, spec: &BatchJobSpec) -> Result<JobHandle> {
        use runpod_sdk::model::{CloudType, GpuTypeId, PodCreateInput};
        use runpod_sdk::service::PodsService;
        let gpu_str = hf_flavor_to_runpod_gpu(&spec.flavor);
        let gpu_id: GpuTypeId = serde_json::from_value(serde_json::Value::String(gpu_str.to_string()))
            .with_context(|| format!("unknown GPU type '{gpu_str}'"))?;
        let req = PodCreateInput {
            name: Some("b00t-batch".into()),
            image_name: Some(spec.image.clone()),
            gpu_type_ids: Some(vec![gpu_id]),
            cloud_type: Some(CloudType::Secure),
            gpu_count: Some(1),
            volume_in_gb: Some(50),
            container_disk_in_gb: Some(20),
            docker_start_cmd: {
                let cp = spec.config_path.trim();
                if cp.is_empty() || cp == "/dev/null" {
                    None  // Image has its own entrypoint — don't override
                } else {
                    Some(vec!["bash".into(), "-c".into(), spec.config_path.clone()])
                }
            },
            env: Some(spec.env.clone()),
            ..Default::default()
        };
        let pod = self.client.create_pod(req).await.context("RunPod create_pod failed")?;
        Ok(JobHandle { id: pod.id, provider: "runpod".into() })
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<String> {
        use runpod_sdk::service::PodsService;
        use runpod_sdk::model::GetPodQuery;
        let pod = self.client.get_pod(&handle.id, GetPodQuery::default())
            .await.context("RunPod get_pod failed")?;
        Ok(format!("pod={} status={:?}", handle.id, pod.desired_status))
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<()> {
        use runpod_sdk::service::PodsService;
        self.client.delete_pod(&handle.id).await.context("RunPod delete_pod failed")
    }

    async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
        use runpod_sdk::service::PodsService;
        use runpod_sdk::model::ListPodsQuery;
        let pods = self.client.list_pods(ListPodsQuery::default())
            .await.context("RunPod list_pods failed")?;
        Ok(pods.into_iter().map(|p| JobHandle { id: p.id, provider: "runpod".into() }).collect())
    }
}

fn hf_flavor_to_runpod_gpu(flavor: &str) -> &str {
    match flavor {
        "a100-large" | "a100" => "NVIDIA A100 80GB PCIe",
        "h100" => "NVIDIA H100 PCIe",
        "a10g-large" | "a10g-small" => "NVIDIA A40",
        _ => "NVIDIA A40",
    }
}

// ── HF provider (CLI wrapper) ─────────────────────────────────────────────────

pub struct HfProvider;

impl HfProvider {
    pub fn new() -> Self {
        Self
    }

    fn run_hf(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("hf")
            .args(args)
            .output()
            .context("hf CLI not found — run: uv tool install huggingface_hub[cli]")?;
        if !out.status.success() {
            bail!(
                "hf {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }
}

/// Pure argv builder, split out so tests can assert the exact `hf jobs run`
/// invocation without the `hf` CLI installed.
fn hf_batch_args(spec: &BatchJobSpec) -> Result<Vec<String>> {
    // 🤓 unlike submit_training_job, no hardcoded script/`--` command here —
    //    the image's own ENTRYPOINT runs. `hf jobs run` can't upload an
    //    arbitrary local file, so config_path must already be an hf:// URI
    //    the job can mount (e.g. via --volume) or reference directly.
    if !spec.config_path.starts_with("hf://") {
        bail!(
            "hf batch jobs require config_path to be an hf:// URI reachable inside the job, got '{}'",
            spec.config_path
        );
    }
    let timeout = format!("{}h", spec.timeout_hours.ceil() as u32);
    let mut args: Vec<String> = vec![
        "jobs".into(),
        "run".into(),
        spec.image.clone(),
        "--flavor".into(),
        spec.flavor.clone(),
        "--timeout".into(),
        timeout,
    ];
    for (key, value) in &spec.env {
        args.push("--env".into());
        args.push(format!("{key}={value}"));
    }
    // trailing entrypoint arg — same convention as the local/runpod providers.
    args.push("--".into());
    args.push(spec.config_path.clone());
    Ok(args)
}

#[async_trait]
impl ComputeProvider for HfProvider {
    fn name(&self) -> &str {
        "hf"
    }

    async fn deploy_inference_endpoint(&self, _cfg: &EndpointConfig) -> Result<EndpointHandle> {
        bail!("HF Jobs does not support serverless inference endpoints; use provider=runpod or k8s")
    }

    async fn endpoint_status(&self, _id: &str) -> Result<EndpointHandle> {
        bail!("HF provider has no endpoint management; use provider=runpod")
    }

    async fn teardown_endpoint(&self, _id: &str) -> Result<()> {
        bail!("HF provider has no endpoint management; use provider=runpod")
    }

    async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
        Ok(vec![])
    }

    async fn submit_training_job(&self, spec: &TrainingJobSpec) -> Result<JobHandle> {
        let timeout = format!("{}h", spec.timeout_hours.ceil() as u32);
        let output = self.run_hf(&[
            "jobs",
            "run",
            &spec.image,
            "--flavor",
            &spec.flavor,
            "--timeout",
            &timeout,
            "--env",
            "UNSLOTH_CACHE_DIR=/opt/unsloth_compiled_cache",
            "--volume",
            "hf://datasets/elasticdotventures/b00t-training:/data:ro",
            "--",
            "python3",
            "/data/train_unsloth.py",
            "--config",
            &spec.config_path,
        ])?;
        // hf jobs run prints the job ID on stdout
        let id = output
            .lines()
            .find(|l| l.len() == 24 && l.chars().all(|c| c.is_ascii_hexdigit()))
            .unwrap_or(output.trim())
            .to_string();
        Ok(JobHandle {
            id,
            provider: "hf".into(),
        })
    }

    async fn submit_batch_job(&self, spec: &BatchJobSpec) -> Result<JobHandle> {
        let args = hf_batch_args(spec)?;
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        let output = self.run_hf(&arg_refs)?;
        let id = output
            .lines()
            .find(|l| l.len() == 24 && l.chars().all(|c| c.is_ascii_hexdigit()))
            .unwrap_or(output.trim())
            .to_string();
        Ok(JobHandle {
            id,
            provider: "hf".into(),
        })
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<String> {
        self.run_hf(&["jobs", "inspect", &handle.id])
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<()> {
        self.run_hf(&["jobs", "cancel", &handle.id])?;
        Ok(())
    }

    async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
        let out = self.run_hf(&["jobs", "ps"])?;
        // parse first hex-looking token per line as job ID
        let handles = out
            .lines()
            .filter_map(|l| {
                l.split_whitespace()
                    .find(|t| t.len() == 24 && t.chars().all(|c| c.is_ascii_hexdigit()))
                    .map(|id| JobHandle {
                        id: id.to_string(),
                        provider: "hf".into(),
                    })
            })
            .collect();
        Ok(handles)
    }
}

// ── Local (podman/docker) provider ────────────────────────────────────────────

/// Label attached to every container this provider starts, so `list_jobs`/
/// `cancel_job` only ever touch b00t-managed batch containers.
const LOCAL_PROVIDER_LABEL: &str = "b00t.provider=local";

pub struct LocalProvider {
    runtime: ContainerRuntime,
}

impl LocalProvider {
    pub fn new() -> Self {
        Self { runtime: detect_container_runtime(Some("podman")) }
    }

    fn run(bin: &str, args: &[String]) -> Result<String> {
        let output = Command::new(bin)
            .args(args)
            .output()
            .with_context(|| format!("failed to run `{bin} {}`", args.join(" ")))?;
        if !output.status.success() {
            bail!(
                "{} {} failed: {}",
                bin,
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

/// Pure argv builder, split out so tests can assert the exact `podman`/
/// `docker run` invocation without touching the GPU gate or a real runtime.
/// Returns `Err` if `config_path` is not a clean absolute path — colons in
/// the path would silently corrupt the `-v src:dst:opts` volume spec.
fn local_batch_args(name: &str, runtime: &ContainerRuntime, spec: &BatchJobSpec) -> Result<Vec<String>> {
    // Validate config_path before interpolating into the bind-mount spec.
    // A colon in the path breaks `src:dst:opts` parsing; non-absolute paths
    // are unreachable inside the container after chroot.
    let p = std::path::Path::new(&spec.config_path);
    if !p.is_absolute() {
        bail!("config_path must be absolute, got: {:?}", spec.config_path);
    }
    if spec.config_path.contains(':') {
        bail!("config_path must not contain ':', got: {:?}", spec.config_path);
    }
    if !p.is_file() {
        bail!("config_path does not exist or is not a file: {:?}", spec.config_path);
    }

    let mut args: Vec<String> = vec![
        "run".into(),
        "-d".into(),
        "--name".into(),
        name.to_string(),
        "--label".into(),
        LOCAL_PROVIDER_LABEL.into(),
    ];

    match runtime {
        ContainerRuntime::Podman => {
            args.push("--device".into());
            args.push("nvidia.com/gpu=all".into());
        }
        ContainerRuntime::Docker => {
            args.push("--gpus".into());
            args.push("all".into());
        }
    }

    for (key, value) in &spec.env {
        args.push("-e".into());
        args.push(format!("{key}={value}"));
    }

    // 🤓 Job-dir convention: config_path's parent directory is the job's
    //    staging dir, bind-mounted rw at /workspace so the runner can read
    //    adjacent inputs (e.g. the photo referenced by the request) and write
    //    outputs (output.json, masks/) back to the host — the read-back half
    //    of app4dog's submit→poll→read pipeline. The request file itself is
    //    passed as the trailing entrypoint arg at its in-container path.
    let config_dir = p
        .parent()
        .ok_or_else(|| anyhow::anyhow!("config_path has no parent dir: {:?}", spec.config_path))?;
    let config_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| anyhow::anyhow!("config_path has no valid filename: {:?}", spec.config_path))?;
    args.push("-v".into());
    args.push(format!("{}:/workspace:rw", config_dir.display()));
    args.push(spec.image.clone());
    args.push(format!("/workspace/{config_name}"));

    Ok(args)
}

#[async_trait]
impl ComputeProvider for LocalProvider {
    fn name(&self) -> &str {
        "local"
    }

    async fn deploy_inference_endpoint(&self, _cfg: &EndpointConfig) -> Result<EndpointHandle> {
        bail!("local provider has no persistent endpoints; use provider=runpod")
    }

    async fn endpoint_status(&self, _id: &str) -> Result<EndpointHandle> {
        bail!("local provider has no persistent endpoints; use provider=runpod")
    }

    async fn teardown_endpoint(&self, _id: &str) -> Result<()> {
        bail!("local provider has no persistent endpoints; use provider=runpod")
    }

    async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
        Ok(vec![])
    }

    async fn submit_training_job(&self, _spec: &TrainingJobSpec) -> Result<JobHandle> {
        bail!("local provider does not run fine-tuning jobs; use provider=hf or provider=runpod")
    }

    async fn submit_batch_job(&self, spec: &BatchJobSpec) -> Result<JobHandle> {
        // 🤓 mirrors the GPU resource-gate pattern from [b00t.hive.resources.gate]
        //    (see finetune.hive.toml) — refuse to start rather than OOM-kill later.
        let snapshot = SystemSnapshot::capture().context("capturing local GPU/RAM state")?;
        let free_mb = match snapshot.gpu_free_mb {
            None => bail!("no GPU detected by SystemSnapshot; LocalProvider requires CUDA-capable hardware"),
            Some(mb) => mb,
        };
        if free_mb < LOCAL_GPU_FREE_MB_GATE {
            bail!(
                "GPU: need {}MB free, have {}MB — stop inference/fine-tuning first (see `b00t hive status`)",
                LOCAL_GPU_FREE_MB_GATE,
                free_mb
            );
        }

        let bin = runtime_bin(&self.runtime);
        let name = format!("b00t-batch-{}", uuid::Uuid::new_v4());
        let args = local_batch_args(&name, &self.runtime, spec)?;

        let id = Self::run(bin, &args)?;
        Ok(JobHandle {
            id,
            provider: "local".into(),
        })
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<String> {
        let bin = runtime_bin(&self.runtime);
        Self::run(
            bin,
            &[
                "inspect".into(),
                "--format".into(),
                "{{.State.Status}}".into(),
                handle.id.clone(),
            ],
        )
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<()> {
        let bin = runtime_bin(&self.runtime);
        // Guard: only remove containers b00t itself created — reject crafted handles.
        let label = Self::run(
            bin,
            &[
                "inspect".into(),
                "--format".into(),
                "{{index .Config.Labels \"b00t.provider\"}}".into(),
                handle.id.clone(),
            ],
        )
        .unwrap_or_default();
        if label.trim() != "local" {
            bail!("container {} is not a b00t-managed local batch job", handle.id);
        }
        Self::run(bin, &["rm".into(), "-f".into(), handle.id.clone()])?;
        Ok(())
    }

    async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
        let bin = runtime_bin(&self.runtime);
        let out = Self::run(
            bin,
            &[
                "ps".into(),
                "-a".into(),
                "--filter".into(),
                format!("label={LOCAL_PROVIDER_LABEL}"),
                "--format".into(),
                "{{.ID}}".into(),
            ],
        )?;
        Ok(out
            .lines()
            .map(|id| JobHandle {
                id: id.trim().to_string(),
                provider: "local".into(),
            })
            .collect())
    }
}

// ── CLI commands ──────────────────────────────────────────────────────────────

#[derive(Parser, Clone)]
pub enum ProviderCommands {
    #[clap(about = "Manage inference endpoints")]
    Endpoint {
        #[clap(subcommand)]
        cmd: EndpointCommands,
    },
    #[clap(about = "Manage training jobs")]
    Job {
        #[clap(subcommand)]
        cmd: ProviderJobCommands,
    },
    #[clap(about = "RunPod GPU cloud — submit, status, list, stop, wait")]
    Runpod {
        #[clap(subcommand)]
        cmd: RunpodSubCommands,
    },
}

#[derive(Parser, Clone)]
pub enum RunpodSubCommands {
    #[clap(about = "Submit a GPU pod from an image")]
    Submit {
        image: String,
        #[clap(long, short = 'g', default_value = "NVIDIA RTX 4090", help = "GPU type")]
        gpu: String,
        #[clap(long, short = 't', default_value = "2", help = "Timeout in hours")]
        timeout: f32,
        #[clap(long, short = 'e', help = "Env vars KEY=VAL (repeatable)")]
        env: Vec<String>,
        #[clap(long, short = 'n', default_value = "b00t-pod", help = "Pod name")]
        name: String,
        #[clap(long, help = "Wait for completion")]
        wait: bool,
    },
    #[clap(about = "Check pod status and GPU utilization")]
    Status {
        id: String,
    },
    #[clap(about = "Block until pod exits")]
    Wait {
        id: String,
        #[clap(long, short = 't', default_value = "3600", help = "Max wait seconds")]
        timeout: u64,
    },
    #[clap(about = "List all pods with cost")]
    List,
    #[clap(about = "Stop a pod (or all with --all)")]
    Stop {
        id: Option<String>,
        #[clap(long, help = "Stop ALL running pods")]
        all: bool,
    },
}

#[derive(Parser, Clone)]
pub enum EndpointCommands {
    #[clap(about = "Deploy serverless inference endpoint")]
    Deploy {
        #[clap(long, default_value = "runpod")]
        provider: String,
        #[clap(long, default_value = "b00t-ch0nky")]
        name: String,
        #[clap(long, help = "Comma-separated GPU type IDs")]
        gpu_types: Option<String>,
        #[clap(long, default_value = "vllm/vllm-openai:latest")]
        image: String,
        #[clap(long)]
        network_volume_id: Option<String>,
    },
    #[clap(about = "Show endpoint status")]
    Status {
        #[clap(long, default_value = "runpod")]
        provider: String,
        id: String,
    },
    #[clap(about = "Tear down endpoint")]
    Teardown {
        #[clap(long, default_value = "runpod")]
        provider: String,
        id: String,
    },
    #[clap(about = "List all endpoints")]
    List {
        #[clap(long, default_value = "runpod")]
        provider: String,
    },
}

#[derive(Parser, Clone)]
pub enum ProviderJobCommands {
    #[clap(about = "Submit training job")]
    Submit {
        #[clap(long, default_value = "hf")]
        provider: String,
        #[clap(long, default_value = "fine-tune/config-cloud-coder.yaml")]
        config: String,
        #[clap(long, default_value = "ghcr.io/elasticdotventures/b00t-training-image:latest")]
        image: String,
        #[clap(long, default_value = "a100-large")]
        flavor: String,
        #[clap(long, default_value_t = 10.0)]
        timeout_hours: f32,
    },
    #[clap(about = "Submit a generic containerized batch job (image runs its own entrypoint)")]
    SubmitBatch {
        #[clap(long, default_value = "local")]
        provider: String,
        #[clap(long, help = "Path/URI the provider resolves into the job's config")]
        config: String,
        #[clap(long)]
        image: String,
        #[clap(long, default_value = "local-gpu")]
        flavor: String,
        #[clap(long, default_value_t = 1.0)]
        timeout_hours: f32,
        #[clap(long = "env", help = "KEY=VALUE, may be repeated")]
        env: Vec<String>,
    },
    #[clap(about = "Show job status")]
    Status {
        #[clap(long, default_value = "hf")]
        provider: String,
        id: String,
    },
    #[clap(about = "Cancel a job")]
    Cancel {
        #[clap(long, default_value = "hf")]
        provider: String,
        id: String,
    },
    #[clap(about = "List all jobs")]
    List {
        #[clap(long, default_value = "hf")]
        provider: String,
    },
}

pub async fn handle_provider_command(cmd: ProviderCommands) -> Result<()> {
    match cmd {
        ProviderCommands::Endpoint { cmd } => handle_endpoint(cmd).await,
        ProviderCommands::Job { cmd } => handle_job(cmd).await,
        ProviderCommands::Runpod { cmd } => handle_runpod(cmd).await,
    }
}

async fn handle_endpoint(cmd: EndpointCommands) -> Result<()> {
    match cmd {
        EndpointCommands::Deploy {
            provider,
            name,
            gpu_types,
            image,
            network_volume_id,
        } => {
            let p = get_provider(&provider)?;
            let gpu_type_ids = gpu_types
                .as_deref()
                .unwrap_or("NVIDIA A40,NVIDIA RTX A6000")
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            let cfg = EndpointConfig {
                name,
                gpu_type_ids,
                image,
                network_volume_id,
                ..Default::default()
            };
            let handle = p.deploy_inference_endpoint(&cfg).await?;
            println!("{}", serde_json::to_string_pretty(&handle)?);
        }
        EndpointCommands::Status { provider, id } => {
            let p = get_provider(&provider)?;
            let handle = p.endpoint_status(&id).await?;
            println!("{}", serde_json::to_string_pretty(&handle)?);
        }
        EndpointCommands::Teardown { provider, id } => {
            let p = get_provider(&provider)?;
            p.teardown_endpoint(&id).await?;
            println!("endpoint {} torn down", id);
        }
        EndpointCommands::List { provider } => {
            let p = get_provider(&provider)?;
            let endpoints = p.list_endpoints().await?;
            println!("{}", serde_json::to_string_pretty(&endpoints)?);
        }
    }
    Ok(())
}

async fn handle_job(cmd: ProviderJobCommands) -> Result<()> {
    match cmd {
        ProviderJobCommands::Submit {
            provider,
            config,
            image,
            flavor,
            timeout_hours,
        } => {
            let p = get_provider(&provider)?;
            let spec = TrainingJobSpec {
                config_path: config,
                image,
                flavor,
                timeout_hours,
            };
            let handle = p.submit_training_job(&spec).await?;
            println!("{}", serde_json::to_string_pretty(&handle)?);
        }
        ProviderJobCommands::SubmitBatch {
            provider,
            config,
            image,
            flavor,
            timeout_hours,
            env,
        } => {
            let p = get_provider(&provider)?;
            let mut env_map = std::collections::HashMap::new();
            for pair in env {
                let (key, value) = pair
                    .split_once('=')
                    .with_context(|| format!("--env expects KEY=VALUE, got '{pair}'"))?;
                env_map.insert(key.to_string(), value.to_string());
            }
            let spec = BatchJobSpec {
                image,
                config_path: config,
                env: env_map,
                flavor,
                timeout_hours,
            };
            let handle = p.submit_batch_job(&spec).await?;
            println!("{}", serde_json::to_string_pretty(&handle)?);
        }
        ProviderJobCommands::Status { provider, id } => {
            let p = get_provider(&provider)?;
            let handle = JobHandle {
                id,
                provider: provider.clone(),
            };
            let status = p.job_status(&handle).await?;
            println!("{}", status);
        }
        ProviderJobCommands::Cancel { provider, id } => {
            let p = get_provider(&provider)?;
            let handle = JobHandle {
                id,
                provider: provider.clone(),
            };
            p.cancel_job(&handle).await?;
            println!("job cancelled");
        }
        ProviderJobCommands::List { provider } => {
            let p = get_provider(&provider)?;
            let jobs = p.list_jobs().await?;
            println!("{}", serde_json::to_string_pretty(&jobs)?);
        }
    }
    Ok(())
}

async fn handle_runpod(cmd: RunpodSubCommands) -> Result<()> {
    use runpod_sdk::model::{CloudType, GpuTypeId, PodCreateInput, PodStatus};
    use runpod_sdk::service::PodsService;
    use runpod_sdk::RunpodConfig;
    let config = RunpodConfig::from_env().context("RUNPOD_API_KEY not set")?;
    let client = runpod_sdk::RunpodClient::new(config).context("RunpodClient::new failed")?;

    match cmd {
        RunpodSubCommands::Submit { image, gpu, timeout: _, env, name, wait: _ } => {
            let gpu_id: GpuTypeId = serde_json::from_value(
                serde_json::Value::String(gpu)
            ).context("unknown GPU type")?;
            let env_map: std::collections::HashMap<String, String> = env.into_iter()
                .filter_map(|e| e.split_once('=').map(|(k,v)| (k.to_string(), v.to_string())))
                .collect();
            let req = PodCreateInput {
                name: Some(name), image_name: Some(image),
                gpu_type_ids: Some(vec![gpu_id]),
                cloud_type: Some(CloudType::Secure),
                gpu_count: Some(1), volume_in_gb: Some(50),
                container_disk_in_gb: Some(20),
                docker_start_cmd: None,
                env: Some(env_map), ..Default::default()
            };
            let pod = client.create_pod(req).await.context("create_pod failed")?;
            println!("{}", pod.id);
        }
        RunpodSubCommands::Status { id } => {
            let pod = client.get_pod(&id, Default::default()).await?;
            let st = pod.desired_status.map(|s| format!("{s:?}")).unwrap_or_default();
            let cost = pod.cost_per_hr;
            println!("pod={id}  status={st}  cost_per_hr=${cost:.2}");
        }
        RunpodSubCommands::List => {
            for p in client.list_pods(Default::default()).await? {
                let st = p.desired_status.map(|s| format!("{s:?}")).unwrap_or_default();
                let cost = p.cost_per_hr;
                println!("{}  {st}  ${cost:.2}/hr", p.id);
            }
        }
        RunpodSubCommands::Stop { id, all } => {
            if all {
                for p in client.list_pods(Default::default()).await? {
                    client.delete_pod(&p.id).await.ok();
                    println!("stopped {}", p.id);
                }
            } else if let Some(pid) = id {
                client.delete_pod(&pid).await?;
                println!("stopped {pid}");
            }
        }
        RunpodSubCommands::Wait { .. } => {
            eprintln!("wait not yet implemented — check status manually");
        }
    }
    Ok(())
}

#[cfg(test)]
mod batch_job_tests {
    use super::*;

    fn sample_spec(image: &str, config_path: &str) -> BatchJobSpec {
        let mut env = std::collections::HashMap::new();
        env.insert("SAM_RUNNER_MODE".to_string(), "real".to_string());
        BatchJobSpec {
            image: image.to_string(),
            config_path: config_path.to_string(),
            env,
            flavor: "a10g-small".to_string(),
            timeout_hours: 1.0,
        }
    }

    #[test]
    fn hf_batch_args_requires_hf_uri_config_path() {
        let spec = sample_spec("app4dog/sam3-runner:cloud", "/local/request.json");
        let err = hf_batch_args(&spec).unwrap_err();
        assert!(err.to_string().contains("hf://"));
    }

    #[test]
    fn hf_batch_args_builds_expected_argv_for_hf_uri() {
        let spec = sample_spec(
            "app4dog/sam3-runner:cloud",
            "hf://datasets/x/request.json",
        );
        let args = hf_batch_args(&spec).expect("hf:// config_path should be accepted");
        assert_eq!(args[0], "jobs");
        assert_eq!(args[1], "run");
        assert_eq!(args[2], "app4dog/sam3-runner:cloud");
        assert!(args.contains(&"--flavor".to_string()));
        assert!(args.contains(&"a10g-small".to_string()));
        assert!(args.contains(&"--env".to_string()));
        assert!(args.contains(&"SAM_RUNNER_MODE=real".to_string()));
        // trailing entrypoint arg after `--`
        assert_eq!(args.last(), Some(&"hf://datasets/x/request.json".to_string()));
    }

    #[test]
    fn local_batch_args_uses_podman_cdi_gpu_flags() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let spec = sample_spec("app4dog/sam1-runner:local", &path);
        let args = local_batch_args("b00t-batch-test", &ContainerRuntime::Podman, &spec).unwrap();
        assert!(args.contains(&"--device".to_string()));
        assert!(args.contains(&"nvidia.com/gpu=all".to_string()));
        assert!(!args.contains(&"--gpus".to_string()));
        // NamedTempFile has a random basename; the job-dir convention passes
        // it through at /workspace/<basename>.
        let basename = std::path::Path::new(&path).file_name().unwrap().to_str().unwrap();
        assert_eq!(args.last().unwrap(), &format!("/workspace/{basename}"));
        assert!(args.contains(&"app4dog/sam1-runner:local".to_string()));
        assert!(args.iter().any(|a| a.ends_with(":/workspace:rw")));
    }

    #[test]
    fn local_batch_args_mounts_job_dir_rw() {
        // Job-dir convention: the config file's parent directory is the job's
        // staging dir — mounted rw at /workspace so the runner can read
        // adjacent inputs (photo) and write outputs (output.json, masks/)
        // back to the host.
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("request.json");
        std::fs::write(&config, b"{}").unwrap();
        let spec = sample_spec("app4dog/sam1-runner:local", config.to_str().unwrap());
        let args = local_batch_args("b00t-batch-test", &ContainerRuntime::Podman, &spec).unwrap();
        let dir_str = dir.path().to_str().unwrap();
        assert!(args.contains(&format!("{dir_str}:/workspace:rw")));
        assert_eq!(args.last().unwrap(), "/workspace/request.json");
        assert!(!args.iter().any(|a| a.ends_with(":ro")));
    }

    #[test]
    fn local_batch_args_uses_docker_gpus_flag() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let spec = sample_spec("app4dog/sam1-runner:local", &path);
        let args = local_batch_args("b00t-batch-test", &ContainerRuntime::Docker, &spec).unwrap();
        assert!(args.contains(&"--gpus".to_string()));
        assert!(args.contains(&"all".to_string()));
        assert!(!args.contains(&"--device".to_string()));
    }

    #[test]
    fn local_batch_args_rejects_colon_in_path() {
        let spec = sample_spec("img:latest", "/some/path:with:colons/req.json");
        assert!(local_batch_args("b00t-batch-test", &ContainerRuntime::Podman, &spec).is_err());
    }

    #[test]
    fn local_batch_args_rejects_relative_path() {
        let spec = sample_spec("img:latest", "relative/path/req.json");
        assert!(local_batch_args("b00t-batch-test", &ContainerRuntime::Podman, &spec).is_err());
    }
}
