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
use std::ffi::OsStr;
use std::process::Command;

use runpod_sdk::model::{
    CloudType, Endpoint, EndpointCreateInput, GetEndpointQuery, GetPodQuery, GpuTypeId,
    ListEndpointsQuery, ListPodsQuery, Pod, PodCreateInput, PodStatus,
};

/// Minimum free VRAM (MB) required before a local batch job is allowed to start.
/// Matches the gate style already used by `[b00t.hive.resources.gate]` profiles.
const LOCAL_GPU_FREE_MB_GATE: u32 = 4000;

/// Default `--memory`/`--memory-swap` cap for local batch containers, overridable
/// via `B00T_LOCAL_MEMORY_LIMIT`. Required by sm3lly's b00t-limits-hook (shared-node
/// protocol, added after a 2026-07-17 uncapped-container crash) — podman run without
/// a memory cap is hard-rejected at the OCI prestart hook.
const LOCAL_MEMORY_LIMIT_DEFAULT: &str = "16g";

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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VolumeMount {
    pub name: String,
    pub path: String,
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
    #[serde(default = "default_gpu_count")]
    pub gpu_count: u32,
    #[serde(default)]
    pub volumes: Vec<VolumeMount>,
    /// Local files to copy alongside the scratch config before submission.
    /// dstack syncs the directory containing the config file, so files
    /// placed there become available in the remote container. Each entry
    /// is a path relative to CWD.
    #[serde(default)]
    pub inputs: Vec<String>,
}

fn default_gpu_count() -> u32 { 1 }

fn fmt_cost(cost: Option<f64>) -> String {
    cost.map(|c| format!("${c:.2}")).unwrap_or_else(|| "-".to_string())
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
        "dstack" => Ok(Box::new(DstackProvider::new())),
        other => bail!(
            "unknown provider '{}'; supported: runpod, hf, local, dstack",
            other
        ),
    }
}

// ── RunPod provider ──────────────────────────────────────────────────────────

/// Thin transport abstraction over the runpod SDK client so pod/endpoint
/// lifecycle can be unit-tested with a mock. Mirrors exactly the methods
/// `ComputeProvider for RunpodProvider` calls; implemented for the real
/// `runpod_sdk::RunpodClient` via its `PodsService`/`EndpointsService` traits.
#[async_trait]
pub trait RunpodApi: Send + Sync {
    async fn create_endpoint(&self, input: EndpointCreateInput) -> Result<Endpoint>;
    async fn get_endpoint(&self, id: &str, query: GetEndpointQuery) -> Result<Endpoint>;
    async fn delete_endpoint(&self, id: &str) -> Result<()>;
    async fn list_endpoints(&self, query: ListEndpointsQuery) -> Result<Vec<Endpoint>>;
    async fn create_pod(&self, input: PodCreateInput) -> Result<Pod>;
    async fn get_pod(&self, id: &str, query: GetPodQuery) -> Result<Pod>;
    async fn delete_pod(&self, id: &str) -> Result<()>;
    async fn list_pods(&self, query: ListPodsQuery) -> Result<Vec<Pod>>;
}

#[async_trait]
impl RunpodApi for runpod_sdk::RunpodClient {
    // UFCS with an explicit service trait keeps these unambiguous even though
    // the same method names exist on both `RunpodApi` and the SDK traits.
    async fn create_endpoint(&self, input: EndpointCreateInput) -> Result<Endpoint> {
        Ok(<Self as runpod_sdk::service::EndpointsService>::create_endpoint(self, input).await?)
    }

    async fn get_endpoint(&self, id: &str, query: GetEndpointQuery) -> Result<Endpoint> {
        Ok(<Self as runpod_sdk::service::EndpointsService>::get_endpoint(self, id, query).await?)
    }

    async fn delete_endpoint(&self, id: &str) -> Result<()> {
        <Self as runpod_sdk::service::EndpointsService>::delete_endpoint(self, id).await?;
        Ok(())
    }

    async fn list_endpoints(&self, query: ListEndpointsQuery) -> Result<Vec<Endpoint>> {
        Ok(<Self as runpod_sdk::service::EndpointsService>::list_endpoints(self, query).await?)
    }

    async fn create_pod(&self, input: PodCreateInput) -> Result<Pod> {
        Ok(<Self as runpod_sdk::service::PodsService>::create_pod(self, input).await?)
    }

    async fn get_pod(&self, id: &str, query: GetPodQuery) -> Result<Pod> {
        Ok(<Self as runpod_sdk::service::PodsService>::get_pod(self, id, query).await?)
    }

    async fn delete_pod(&self, id: &str) -> Result<()> {
        <Self as runpod_sdk::service::PodsService>::delete_pod(self, id).await?;
        Ok(())
    }

    async fn list_pods(&self, query: ListPodsQuery) -> Result<Vec<Pod>> {
        Ok(<Self as runpod_sdk::service::PodsService>::list_pods(self, query).await?)
    }
}

pub struct RunpodProvider<C: RunpodApi = runpod_sdk::RunpodClient> {
    client: C,
}

impl RunpodProvider<runpod_sdk::RunpodClient> {
    pub fn new() -> Result<Self> {
        let config = runpod_sdk::RunpodConfig::from_env()
            .context("RUNPOD_API_KEY not set — see PROVIDER-RUNPOD.provider.tomllmd")?;
        Ok(Self {
            client: runpod_sdk::RunpodClient::new(config)
                .context("RunpodClient::new failed")?,
        })
    }
}

#[cfg(test)]
impl<C: RunpodApi> RunpodProvider<C> {
    /// Test-only constructor: inject a mock transport.
    fn with_client(client: C) -> Self {
        Self { client }
    }
}

#[async_trait]
impl<C: RunpodApi> ComputeProvider for RunpodProvider<C> {
    fn name(&self) -> &str {
        "runpod"
    }

    async fn deploy_inference_endpoint(&self, cfg: &EndpointConfig) -> Result<EndpointHandle> {
        let input = endpoint_create_request(cfg);
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
        self.client
            .delete_endpoint(id)
            .await
            .context("RunPod delete_endpoint failed")
    }

    async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
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
        let req = training_pod_request(spec)?;
        let pod = self.client.create_pod(req).await.context("RunPod create_pod failed")?;
        Ok(JobHandle { id: pod.id, provider: "runpod".into() })
    }

    async fn submit_batch_job(&self, spec: &BatchJobSpec) -> Result<JobHandle> {
        let req = batch_pod_request(spec)?;
        let pod = self.client.create_pod(req).await.context("RunPod create_pod failed")?;
        Ok(JobHandle { id: pod.id, provider: "runpod".into() })
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<String> {
        let pod = self.client.get_pod(&handle.id, GetPodQuery::default())
            .await.context("RunPod get_pod failed")?;
        Ok(format!("pod={} status={:?}", handle.id, pod.desired_status))
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<()> {
        self.client.delete_pod(&handle.id).await.context("RunPod delete_pod failed")
    }

    async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
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

/// Parses a RunPod GPU type string into the SDK enum, with an error that names
/// the offending string. Pure — split out for unit-testing the error path.
fn parse_gpu_type_id(gpu_str: &str) -> Result<GpuTypeId> {
    serde_json::from_value(serde_json::Value::String(gpu_str.to_string()))
        .with_context(|| format!("unknown GPU type '{gpu_str}'"))
}

/// Env vars injected into training pods: the config path the runner reads and
/// the unsloth compiled-cache mount.
fn training_pod_env(config_path: &str) -> std::collections::HashMap<String, String> {
    [
        ("TRAINING_CONFIG".to_string(), config_path.to_string()),
        ("UNSLOTH_CACHE_DIR".to_string(), "/opt/unsloth_compiled_cache".to_string()),
    ]
    .into()
}

/// Pure decision helper for batch pods: empty or `/dev/null` config paths mean
/// the image carries its own entrypoint and must not be overridden.
fn docker_start_cmd_for(config_path: &str) -> Option<Vec<String>> {
    let cp = config_path.trim();
    if cp.is_empty() || cp == "/dev/null" {
        None
    } else {
        Some(vec!["bash".into(), "-c".into(), config_path.to_string()])
    }
}

/// Builds the PodCreateInput for a fine-tuning pod. Split out so request
/// construction is unit-testable without a live RunPod connection.
fn training_pod_request(spec: &TrainingJobSpec) -> Result<PodCreateInput> {
    let gpu_id = parse_gpu_type_id(hf_flavor_to_runpod_gpu(&spec.flavor))?;
    Ok(PodCreateInput {
        name: Some("b00t-training".into()),
        image_name: Some(spec.image.clone()),
        gpu_type_ids: Some(vec![gpu_id]),
        cloud_type: Some(CloudType::Secure),
        gpu_count: Some(1),
        volume_in_gb: Some(50),
        container_disk_in_gb: Some(20),
        env: Some(training_pod_env(&spec.config_path)),
        ..Default::default()
    })
}

/// Builds the PodCreateInput for a generic containerized batch job.
fn batch_pod_request(spec: &BatchJobSpec) -> Result<PodCreateInput> {
    let gpu_id = parse_gpu_type_id(hf_flavor_to_runpod_gpu(&spec.flavor))?;
    Ok(PodCreateInput {
        name: Some("b00t-batch".into()),
        image_name: Some(spec.image.clone()),
        gpu_type_ids: Some(vec![gpu_id]),
        cloud_type: Some(CloudType::Secure),
        gpu_count: Some(1),
        volume_in_gb: Some(50),
        container_disk_in_gb: Some(20),
        docker_start_cmd: docker_start_cmd_for(&spec.config_path),
        env: Some(spec.env.clone()),
        ..Default::default()
    })
}

/// Builds the EndpointCreateInput for a serverless inference endpoint.
/// 🤓 EndpointCreateInput requires template_id; env is baked into the template
fn endpoint_create_request(cfg: &EndpointConfig) -> EndpointCreateInput {
    let template_id = cfg
        .env
        .get("RUNPOD_TEMPLATE_ID")
        .cloned()
        .unwrap_or_default();
    EndpointCreateInput {
        template_id,
        name: Some(cfg.name.clone()),
        workers_min: Some(cfg.workers_min),
        workers_max: Some(cfg.workers_max),
        idle_timeout: Some(cfg.idle_timeout_s as i32),
        execution_timeout_ms: Some(cfg.execution_timeout_ms as i32),
        network_volume_id: cfg.network_volume_id.clone(),
        ..Default::default()
    }
}

/// Formats the one-line pod status used by `b00t provider runpod status`.
fn fmt_pod_status_line(id: &str, status: Option<PodStatus>, cost_per_hr: Option<f64>) -> String {
    let st = status.map(|s| format!("{s:?}")).unwrap_or_default();
    format!("pod={id}  status={st}  cost_per_hr={}", fmt_cost(cost_per_hr))
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

// ── dstack provider ────────────────────────────────────────────────────────

pub struct DstackProvider;

impl DstackProvider {
    pub fn new() -> Self {
        Self
    }

    fn run_dstack(&self, args: &[&str]) -> Result<String> {
        let out = Command::new("dstack")
            .args(args)
            .output()
            .context("dstack CLI not found — run: uv tool install 'dstack[all]'")?;
        if !out.status.success() {
            bail!(
                "dstack {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    /// Applies a `type: volume` config — idempotent per dstack's own
    /// `apply` semantics. Call once before submitting jobs that reference
    /// this volume by name.
    pub fn ensure_volume(&self, name: &str, size_gb: u32, region: &str) -> Result<()> {
        let yaml = dstack_volume_yaml(name, size_gb, region);
        let tmp = dstack_scratch_config_path(name, "volume")?;
        std::fs::write(&tmp, yaml).context("writing dstack volume config")?;
        let path = tmp.to_str().context("temp file path is not valid UTF-8")?;
        let result = self.run_dstack(&["apply", "-f", path, "-y", "-d"]);
        // Cleanup is best-effort — a failure to remove the temp file must not
        // fail the volume application itself.
        if let Err(err) = std::fs::remove_file(&tmp) {
            tracing::warn!("failed to remove temp dstack volume config {tmp:?}: {err}");
        }
        result.map(|_| ())
    }

    /// Stops a named dev-environment/service run — the lifecycle/cost-control
    /// counterpart to a persistent (non-auto-terminating) resource. Distinct
    /// from `cancel_job` (which targets task/batch runs via `JobHandle`) —
    /// dev-environments are addressed by name, not a `JobHandle`, since they
    /// aren't created through `submit_batch_job`.
    pub fn stop_dev_environment(&self, name: &str) -> Result<()> {
        self.run_dstack(&["stop", name, "-y"])?;
        Ok(())
    }

    /// Applies a `type: fleet` autoscaling (0..1) config so a task submission
    /// has capacity to schedule against. dstack 0.20.x requires a matching
    /// fleet to already exist before any task can be scheduled — verified
    /// via a live RunPod test (Task 1, see tests/fixtures/dstack_ps_json.
    /// NOTES.md): submitting a bare task with no fleet fails immediately
    /// with "No matching fleet found" / FAILED_TO_START_DUE_TO_NO_CAPACITY,
    /// before the request ever reaches the backend — the older
    /// per-task-dynamic-pod assumption this provider was originally
    /// designed against no longer holds. `nodes: 0..1` costs nothing while
    /// idle. Applying an existing fleet name is idempotent per dstack's own
    /// `apply` semantics (same as `ensure_volume`) — safe to call before
    /// every submission rather than tracking whether it already ran.
    pub fn ensure_fleet(&self, name: &str, gpu_count: u32) -> Result<()> {
        let yaml = dstack_fleet_yaml(name, gpu_count);
        let tmp = dstack_scratch_config_path(name, "fleet")?;
        std::fs::write(&tmp, yaml).context("writing dstack fleet config")?;
        let path = tmp.to_str().context("temp file path is not valid UTF-8")?;
        let result = self.run_dstack(&["apply", "-f", path, "-y", "-d"]);
        // Cleanup is best-effort — a failure to remove the temp file must not
        // fail the fleet application itself.
        if let Err(err) = std::fs::remove_file(&tmp) {
            tracing::warn!("failed to remove temp dstack fleet config {tmp:?}: {err}");
        }
        result.map(|_| ())
    }
}

/// Generates a `type: volume` dstack config — a persistent volume that
/// survives across separate `dstack apply` runs (verified against dstack's
/// docs: "Volumes enable data persistence between runs of dev environments,
/// tasks, and services"). Call once per volume name; re-applying an
/// existing volume name is idempotent per dstack's own `apply` semantics
/// (not re-verified here — Task 1's fixture capture should confirm this
/// once real dstack access exists).
fn dstack_volume_yaml(name: &str, size_gb: u32, region: &str) -> String {
    format!(
        "type: volume\nname: {name}\nsize: {size_gb}GB\nregion: {region}\n"
    )
}

/// The shared autoscaling fleet all `DstackProvider` submissions ensure
/// exists before scheduling a task — one pool per host, reused across
/// jobs, rather than a fleet per submission (matches the warm-reuse
/// philosophy behind Task 12's persistent volumes). The hostname suffix
/// isolates instances so two b00t processes sharing the same dstack
/// server don't compete for the same fleet's capacity.
fn shared_fleet_name() -> String {
    let host = hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "unknown".into());
    format!("b00t-dstack-fleet-{}", sanitize_fleet_host_part(&host))
}

/// Sanitizes an arbitrary hostname down to the `host_part` portion of
/// `shared_fleet_name`'s dstack fleet name. Only ASCII alphanumerics survive
/// unchanged (`char::is_ascii_alphanumeric`, not the Unicode-aware
/// `is_alphanumeric` — non-ASCII hostnames like accented Latin or CJK
/// characters must not pass through untouched, since dstack's fleet-name
/// regex `^[a-z][a-z0-9-]{1,40}$` is ASCII-only); everything else becomes a
/// hyphen, consecutive hyphens collapse, and the result is capped at 23
/// chars so the full `b00t-dstack-fleet-<host_part>` name stays within
/// dstack's 41-char limit.
fn sanitize_fleet_host_part(host: &str) -> String {
    let sanitized: String = host
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    // Collapse consecutive hyphens and trim leading/trailing hyphens.
    let collapsed = sanitized
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if collapsed.is_empty() {
        "unknown".into()
    } else {
        collapsed.chars().take(23).collect::<String>()
    }
}

/// Generates a `type: fleet` autoscaling config: `nodes: 0..1` costs nothing
/// while idle (dstack only provisions compute once a task actually needs
/// scheduling capacity), `resources: gpu: 1` requests any single GPU rather
/// than a specific model. Deliberately omits `backends:`/`regions:` so it
/// matches whatever backend(s) the operator's dstack server config.yml has
/// configured (runpod today, potentially gcp/azure later) instead of
/// hardcoding one.
fn dstack_fleet_yaml(name: &str, gpu_count: u32) -> String {
    format!(
        "type: fleet\nname: {name}\nnodes: 0..1\nresources:\n  gpu: {gpu_count}\n"
    )
}

/// Pure YAML builder, split out so tests can assert the exact task config
/// without the `dstack` CLI installed — same rationale as `hf_batch_args`.
///
/// `config_path`, `flavor`, and `timeout_hours` are passed through as plain
/// environment variables (`B00T_JOB_CONFIG_PATH`, `B00T_JOB_FLAVOR`,
/// `B00T_JOB_TIMEOUT_HOURS`) — env vars are a construct we've confirmed
/// dstack supports.
///
/// **Resolved via Task 10's live e2e test** (was an open question until real
/// dstack access existed): dstack's own `TaskConfiguration` model requires
/// "either `commands` or `image` must be set", not both — `commands:` is
/// optional whenever `image:` is present, in which case dstack runs the
/// image's own ENTRYPOINT/CMD. `BatchJobSpec` carries no command-override
/// field (the intended architecture is that job images — e.g.
/// `mesh-runner:v6` — bake in their own entrypoint that reads
/// `B00T_JOB_CONFIG_PATH` and emits the PASS/FAIL evidence line), so
/// `commands:` is deliberately omitted here entirely. An earlier version of
/// this function emitted a hardcoded `commands: [echo starting]` placeholder
/// — that was a real bug, not a harmless stopgap: it silently replaced every
/// real image's actual entrypoint with a no-op on every single submission,
/// discovered only by running a real submission against a real image.
fn dstack_task_yaml(name: &str, spec: &BatchJobSpec) -> String {
    let mut env_lines = String::new();
    env_lines.push_str(&format!(
        "  B00T_JOB_CONFIG_PATH: \"{}\"\n",
        spec.config_path
    ));
    env_lines.push_str(&format!("  B00T_JOB_FLAVOR: \"{}\"\n", spec.flavor));
    env_lines.push_str(&format!(
        "  B00T_JOB_TIMEOUT_HOURS: \"{}\"\n",
        spec.timeout_hours
    ));
    for (key, value) in &spec.env {
        env_lines.push_str(&format!("  {key}: \"{value}\"\n"));
    }

    let mut volumes_block = String::new();
    if !spec.volumes.is_empty() {
        volumes_block.push_str("volumes:\n");
        for v in &spec.volumes {
            volumes_block.push_str(&format!("  - name: {}\n    path: {}\n", v.name, v.path));
        }
    }

    format!(
        "type: task\nname: {name}\nimage: {image}\nenv:\n{env_lines}{volumes_block}",
        image = spec.image,
    )
}

#[async_trait]
impl ComputeProvider for DstackProvider {
    fn name(&self) -> &str {
        "dstack"
    }

    async fn deploy_inference_endpoint(&self, _cfg: &EndpointConfig) -> Result<EndpointHandle> {
        bail!("dstack provider does not yet support inference endpoints in b00t (batch/training jobs only) — use provider=runpod")
    }

    async fn endpoint_status(&self, _id: &str) -> Result<EndpointHandle> {
        bail!("dstack provider has no endpoint management yet; use provider=runpod")
    }

    async fn teardown_endpoint(&self, _id: &str) -> Result<()> {
        bail!("dstack provider has no endpoint management yet; use provider=runpod")
    }

    async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
        Ok(vec![])
    }

    async fn submit_training_job(&self, spec: &TrainingJobSpec) -> Result<JobHandle> {
        let name = format!("b00t-train-{}", dstack_short_id());
        let batch_spec = BatchJobSpec {
            image: spec.image.clone(),
            config_path: spec.config_path.clone(),
            env: Default::default(),
            flavor: spec.flavor.clone(),
            timeout_hours: spec.timeout_hours,
            gpu_count: 1,
            volumes: vec![],
            inputs: vec![],
        };
        let yaml = dstack_task_yaml(&name, &batch_spec);
        submit_dstack_yaml(self, &name, &yaml, batch_spec.gpu_count, &batch_spec.inputs)
    }

    async fn submit_batch_job(&self, spec: &BatchJobSpec) -> Result<JobHandle> {
        let name = format!("b00t-job-{}", dstack_short_id());
        let yaml = dstack_task_yaml(&name, spec);
        submit_dstack_yaml(self, &name, &yaml, spec.gpu_count, &spec.inputs)
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<String> {
        let out = self.run_dstack(&["ps", "--json", "-a"])?;
        let matches = parse_dstack_ps_json(&out, Some(&handle.id))?;
        // `dstack ps -a` returns full run history, so a re-used run name can
        // have multiple entries (verified against the real fixture: 4
        // historical attempts under one name, ordered most-recent-first by
        // submitted_at) — `.next()` takes the first, i.e. the latest attempt.
        let (_, status) = matches.into_iter().next().ok_or_else(|| {
            anyhow::anyhow!("dstack run '{}' not found in `dstack ps`", handle.id)
        })?;
        Ok(format!("run={} status={}", handle.id, status))
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<()> {
        self.run_dstack(&["stop", &handle.id, "-y"])?;
        Ok(())
    }

    async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
        let out = self.run_dstack(&["ps", "--json", "-a"])?;
        let matches = parse_dstack_ps_json(&out, None)?;
        Ok(matches.into_iter().map(|(h, _)| h).collect())
    }
}

/// Generates a short, lowercase-hex ID safe to embed in a dstack resource
/// name. Verified via a real, live `dstack apply` invocation (Task 10 e2e
/// smoke test): dstack rejects any `name:` not matching
/// `^[a-z][a-z0-9-]{1,40}$` (max 41 characters total) with "Resource name
/// should match regex ...". A full UUID (36 chars) pushed
/// `b00t-job-<uuid>` to 45 characters — always over the limit, so every
/// `submit_batch_job`/`submit_training_job` call failed unconditionally
/// before this fix. 12 hex characters (48 bits) keeps collision risk
/// negligible for this provider's job volumes while leaving headroom under
/// the 41-char cap for any prefix used here (`b00t-job-`, `b00t-train-`).
fn dstack_short_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()[..12].to_string()
}

/// Resolves the scratch-file path for a generated dstack config, rooted at
/// the current working directory rather than the system temp dir.
///
/// Verified via a real, live `dstack apply` invocation (Task 10 e2e smoke
/// test): dstack's own `apply` command computes
/// `configuration_path.absolute().relative_to(Path.cwd())` and errors —
/// before ever making a network call — if the config file isn't inside the
/// CWD's subtree ("... is not in the subpath of ..."). System temp dirs
/// (`/tmp` on Linux) are essentially never a subpath of an arbitrary
/// invocation's CWD, so every dstack config write in this provider
/// (`ensure_volume`, `ensure_fleet`, `submit_dstack_yaml`) must live under
/// CWD. Uses a dotfile-prefixed name to avoid cluttering directory
/// listings; all three call sites already best-effort `remove_file` it
/// after `apply` runs. Rejects names containing path separators or `..`
/// to prevent directory traversal outside CWD.
fn dstack_scratch_config_path(name: &str, suffix: &str) -> Result<std::path::PathBuf> {
    if name.contains('/') || name.contains('\\') || name.contains("..") {
        anyhow::bail!(
            "dstack scratch config name {:?} contains path separator or '..' — rejecting",
            name
        );
    }
    let cwd = std::env::current_dir()
        .context("resolving current directory for dstack config scratch file")?;
    Ok(cwd.join(format!(".{name}.{suffix}.dstack.yml")))
}

/// Copies each local file in `inputs` into `dest_dir` so dstack syncs them
/// to the remote container alongside the task config. A missing input file
/// or a failed copy fails fast with an `Err` — silently skipping (the prior
/// behavior: `tracing::warn!` + `continue`) let jobs submit to dstack
/// without a file they needed, failing opaquely inside the remote container
/// instead of locally where it's actionable.
fn copy_dstack_inputs(inputs: &[String], dest_dir: &std::path::Path) -> Result<()> {
    for input in inputs {
        let src = std::path::Path::new(input);
        if !src.exists() {
            anyhow::bail!("input file {input:?} not found for dstack job submission");
        }
        let dest = dest_dir.join(src.file_name().unwrap_or_else(|| OsStr::new(input)));
        std::fs::copy(src, &dest)
            .with_context(|| format!("failed to copy dstack input {input:?} to {dest:?}"))?;
    }
    Ok(())
}

/// Write `yaml` to a temp file and `dstack apply -f <file> -y -d` it.
/// Split out so submit_batch_job/submit_training_job share one path.
/// Ensures the shared autoscaling fleet exists first — see
/// `DstackProvider::ensure_fleet` for why this is required against real
/// dstack 0.20.x, not optional. Copies local files listed in `inputs` into
/// the scratch directory so dstack syncs them to the remote container.
fn submit_dstack_yaml(
    provider: &DstackProvider,
    name: &str,
    yaml: &str,
    gpu_count: u32,
    inputs: &[String],
) -> Result<JobHandle> {
    provider
        .ensure_fleet(&shared_fleet_name(), gpu_count)
        .context("ensuring shared dstack fleet exists before task submission")?;
    let scratch_dir = dstack_scratch_config_path(name, "task")?;
    std::fs::write(&scratch_dir, yaml).context("writing dstack task config")?;
    let dest_dir = scratch_dir.parent().unwrap_or(&scratch_dir);
    copy_dstack_inputs(inputs, dest_dir)?;
    let tmp_path = scratch_dir
        .to_str()
        .context("temp file path is not valid UTF-8")?;
    provider.run_dstack(&["apply", "-f", tmp_path, "-y", "-d"])?;
    // Cleanup is best-effort — a failure to remove the temp file must not
    // fail the job submission itself.
    if let Err(err) = std::fs::remove_file(&scratch_dir) {
        tracing::warn!("failed to remove temp dstack task config {scratch_dir:?}: {err}");
    }
    Ok(JobHandle {
        id: name.to_string(),
        provider: "dstack".into(),
    })
}

/// Parse `dstack ps --json -a` output. Field names below are taken from the
/// real fixture captured in Task 1 against a live dstack 0.20.28 server
/// (`tests/fixtures/dstack_ps_json.txt`) — the top-level shape is
/// `{"project": ..., "runs": [...]}`, and each run's display name lives at
/// `run_spec.run_name`, NOT a top-level `name`/`run_name` key (the original
/// plan draft guessed the latter before real dstack access was available;
/// corrected here against the actual captured shape). `status` is a
/// top-level string on each run object (verified: `"done"` in the fixture).
fn parse_dstack_ps_json(json: &str, run_name: Option<&str>) -> Result<Vec<(JobHandle, String)>> {
    let value: serde_json::Value =
        serde_json::from_str(json).context("parsing dstack ps --json output")?;
    let runs = value
        .get("runs")
        .and_then(|r| r.as_array())
        .cloned()
        .or_else(|| value.as_array().cloned())
        .ok_or_else(|| anyhow::anyhow!("unexpected dstack ps --json shape: {json}"))?;

    let mut out = Vec::new();
    for run in runs {
        let name = run
            .get("run_spec")
            .and_then(|rs| rs.get("run_name"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("run entry missing run_spec.run_name field: {run}"))?
            .to_string();
        if let Some(filter) = run_name {
            if name != filter {
                continue;
            }
        }
        let status = run
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();
        out.push((
            JobHandle {
                id: name,
                provider: "dstack".into(),
            },
            status,
        ));
    }
    Ok(out)
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

    let memory_limit = std::env::var("B00T_LOCAL_MEMORY_LIMIT")
        .unwrap_or_else(|_| LOCAL_MEMORY_LIMIT_DEFAULT.to_string());
    args.push("--memory".into());
    args.push(memory_limit.clone());
    args.push("--memory-swap".into());
    args.push(memory_limit);

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
    #[clap(about = "dstack — persistent volumes, dev-environment lifecycle")]
    Dstack {
        #[clap(subcommand)]
        cmd: DstackSubCommands,
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

#[derive(Parser, Clone, Debug)]
pub enum DstackSubCommands {
    #[clap(about = "Ensure a persistent volume exists (idempotent)")]
    EnsureVolume {
        name: String,
        #[clap(long, help = "Volume size in GB")]
        size_gb: u32,
        #[clap(long, help = "dstack region")]
        region: String,
    },
    #[clap(about = "Stop a named dev-environment/service run")]
    StopDevEnvironment {
        name: String,
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
        ProviderCommands::Dstack { cmd } => handle_dstack(cmd).await,
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
                gpu_count: 1,
                volumes: vec![],
                inputs: vec![],
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
            println!("{}", fmt_pod_status_line(&id, pod.desired_status, pod.cost_per_hr));
        }
        RunpodSubCommands::List => {
            for p in client.list_pods(Default::default()).await? {
                let st = p.desired_status.map(|s| format!("{s:?}")).unwrap_or_default();
                println!("{}  {st}  {} /hr", p.id, fmt_cost(p.cost_per_hr));
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

async fn handle_dstack(cmd: DstackSubCommands) -> Result<()> {
    let provider = DstackProvider::new();
    match cmd {
        DstackSubCommands::EnsureVolume { name, size_gb, region } => {
            provider.ensure_volume(&name, size_gb, &region)?;
            println!("volume {name} ready ({size_gb}GB, {region})");
        }
        DstackSubCommands::StopDevEnvironment { name } => {
            provider.stop_dev_environment(&name)?;
            println!("stopped dev-environment {name}");
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
            gpu_count: 1,
            volumes: vec![],
            inputs: vec![],
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
    fn local_batch_args_includes_memory_cap_by_default() {
        // b00t-limits-hook (shared-node protocol) hard-rejects podman run
        // without --memory/--memory-swap — regression coverage for that.
        // Not hermetic against a caller-set $B00T_LOCAL_MEMORY_LIMIT (process-global,
        // tests run in-process/parallel) — skip the default-value assertion then,
        // same rationale as test_effective_kubeconfig_path.
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_str().unwrap().to_string();
        let spec = sample_spec("app4dog/sam1-runner:local", &path);
        let args = local_batch_args("b00t-batch-test", &ContainerRuntime::Podman, &spec).unwrap();
        let mem_idx = args.iter().position(|a| a == "--memory").expect("--memory flag present");
        let swap_idx = args
            .iter()
            .position(|a| a == "--memory-swap")
            .expect("--memory-swap flag present");
        if std::env::var("B00T_LOCAL_MEMORY_LIMIT").is_ok() {
            return;
        }
        assert_eq!(args[mem_idx + 1], LOCAL_MEMORY_LIMIT_DEFAULT);
        assert_eq!(args[swap_idx + 1], LOCAL_MEMORY_LIMIT_DEFAULT);
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

    #[test]
    fn dstack_task_yaml_includes_image_env_and_command() {
        let mut env = std::collections::HashMap::new();
        env.insert("MESH_GPU".to_string(), "auto".to_string());
        let spec = BatchJobSpec {
            image: "docker.io/elasticdotventures/mesh-runner:v6".into(),
            config_path: "/workspace/request.json".into(),
            env,
            flavor: "RTX_4090".into(),
            timeout_hours: 2.0,
            gpu_count: 1,
            volumes: vec![],
            inputs: vec![],
        };
        let yaml = dstack_task_yaml("b00t-job-abc123", &spec);
        assert!(yaml.contains("type: task"));
        assert!(yaml.contains("name: b00t-job-abc123"));
        assert!(yaml.contains("image: docker.io/elasticdotventures/mesh-runner:v6"));
        assert!(yaml.contains("MESH_GPU: \"auto\""));
        assert!(yaml.contains("B00T_JOB_CONFIG_PATH: \"/workspace/request.json\""));
        assert!(yaml.contains("B00T_JOB_FLAVOR: \"RTX_4090\""));
        assert!(yaml.contains("B00T_JOB_TIMEOUT_HOURS: \"2\""));
        // Task 10 regression guard: `commands:` must be omitted so dstack
        // runs the image's own ENTRYPOINT/CMD — an earlier version hardcoded
        // `commands: [echo starting]` here, which silently discarded every
        // real image's actual behavior on every submission (found via a
        // live e2e run, not by inspection).
        assert!(!yaml.contains("commands:"));
    }

    #[test]
    fn dstack_volume_yaml_includes_size_and_region() {
        let yaml = dstack_volume_yaml("b00t-mesh-cache", 100, "eu-central-1");
        assert!(yaml.contains("type: volume"));
        assert!(yaml.contains("name: b00t-mesh-cache"));
        assert!(yaml.contains("size: 100GB"));
        assert!(yaml.contains("region: eu-central-1"));
    }

    #[test]
    fn parses_real_dstack_ps_json_fixture() {
        let json = include_str!("../../tests/fixtures/dstack_ps_json.txt");
        let parsed = parse_dstack_ps_json(json, None).expect("fixture should parse");
        assert!(!parsed.is_empty(), "fixture should contain at least one run");
        let (handle, status) = &parsed[0];
        assert_eq!(handle.provider, "dstack");
        assert_eq!(handle.id, "b00t-fixture-capture");
        assert_eq!(status, "done");
    }

    #[test]
    fn parses_real_dstack_ps_json_fixture_filtered_by_run_name() {
        let json = include_str!("../../tests/fixtures/dstack_ps_json.txt");
        // The real fixture contains 4 historical runs, all named
        // "b00t-fixture-capture" (repeated attempts from live troubleshooting
        // during Task 1's capture — 3 failed before the fleet-ensure fix,
        // 1 succeeded) — dstack's `ps -a` returns full run history, not just
        // the latest attempt per name, so filtering by name legitimately
        // returns multiple entries here.
        let parsed = parse_dstack_ps_json(json, Some("b00t-fixture-capture"))
            .expect("fixture should parse");
        assert_eq!(parsed.len(), 4);
        let parsed_missing = parse_dstack_ps_json(json, Some("no-such-run"))
            .expect("fixture should still parse with a non-matching filter");
        assert!(parsed_missing.is_empty());
    }

    #[test]
    fn dstack_short_id_keeps_job_and_train_names_within_dstack_name_limit() {
        // dstack's real name regex, confirmed live: ^[a-z][a-z0-9-]{1,40}$
        // (max 41 chars total, lowercase alphanumeric + hyphen only).
        let re = regex::Regex::new("^[a-z][a-z0-9-]{1,40}$").unwrap();
        for _ in 0..20 {
            let job_name = format!("b00t-job-{}", dstack_short_id());
            let train_name = format!("b00t-train-{}", dstack_short_id());
            assert!(re.is_match(&job_name), "job name violates dstack regex: {job_name}");
            assert!(re.is_match(&train_name), "train name violates dstack regex: {train_name}");
        }
    }

    #[test]
    fn dstack_fleet_yaml_is_autoscaling_zero_to_one_with_gpu() {
        let yaml = dstack_fleet_yaml("test-fleet", 1);
        assert!(yaml.contains("type: fleet"));
        assert!(yaml.contains("name: test-fleet"));
        assert!(yaml.contains("nodes: 0..1"));
        assert!(yaml.contains("gpu: 1"));
        // Deliberately no `backends:`/`regions:` — must match whatever
        // backend(s) the operator's dstack server config.yml has actually
        // configured, not hardcode runpod.
        assert!(!yaml.contains("backends:"));
        assert!(!yaml.contains("regions:"));
    }

    #[test]
    fn dstack_task_yaml_attaches_volumes_when_present() {
        let env = std::collections::HashMap::new();
        let spec = BatchJobSpec {
            image: "docker.io/elasticdotventures/mesh-runner:v6".into(),
            config_path: "/workspace/request.json".into(),
            env,
            flavor: "RTX_4090".into(),
            timeout_hours: 2.0,
            gpu_count: 1,
            volumes: vec![VolumeMount { name: "b00t-mesh-cache".into(), path: "/cache".into() }],
            inputs: vec![],
        };
        let yaml = dstack_task_yaml("b00t-job-abc", &spec);
        assert!(yaml.contains("volumes:"));
        assert!(yaml.contains("- name: b00t-mesh-cache"));
        assert!(yaml.contains("path: /cache"));
    }

    #[test]
    fn dstack_task_yaml_omits_volumes_block_when_empty() {
        let spec = BatchJobSpec {
            image: "ubuntu:24.04".into(),
            config_path: "/dev/null".into(),
            env: Default::default(),
            flavor: "cpu".into(),
            timeout_hours: 1.0,
            gpu_count: 1,
            volumes: vec![],
            inputs: vec![],
        };
        let yaml = dstack_task_yaml("b00t-job-def", &spec);
        assert!(!yaml.contains("volumes:"));
    }

    #[test]
    fn dstack_subcommands_parses_ensure_volume() {
        let cmd = DstackSubCommands::try_parse_from([
            "dstack",
            "ensure-volume",
            "b00t-mesh-cache",
            "--size-gb",
            "100",
            "--region",
            "eu-central-1",
        ])
        .expect("ensure-volume should parse: positional name, --size-gb, --region");
        match cmd {
            DstackSubCommands::EnsureVolume { name, size_gb, region } => {
                assert_eq!(name, "b00t-mesh-cache");
                assert_eq!(size_gb, 100);
                assert_eq!(region, "eu-central-1");
            }
            _ => panic!("expected EnsureVolume variant"),
        }
    }

    #[test]
    fn dstack_subcommands_parses_stop_dev_environment() {
        let cmd = DstackSubCommands::try_parse_from([
            "dstack",
            "stop-dev-environment",
            "my-env",
        ])
        .expect("stop-dev-environment should parse: positional name");
        match cmd {
            DstackSubCommands::StopDevEnvironment { name } => {
                assert_eq!(name, "my-env");
            }
            _ => panic!("expected StopDevEnvironment variant"),
        }
    }

    #[test]
    fn dstack_subcommands_ensure_volume_requires_size_gb_and_region() {
        let err = DstackSubCommands::try_parse_from([
            "dstack",
            "ensure-volume",
            "b00t-mesh-cache",
        ])
        .unwrap_err();
        assert!(err.to_string().contains("size-gb") || err.to_string().contains("required"));
    }

    #[test]
    fn sanitize_fleet_host_part_strips_non_ascii_to_valid_dstack_name() {
        // Regression guard: char::is_alphanumeric() is Unicode-aware and let
        // accented/CJK characters through untouched, producing a fleet name
        // that violated dstack's ^[a-z][a-z0-9-]{1,40}$ regex. Non-ASCII
        // chars must be replaced with '-' like any other punctuation.
        let host_part = sanitize_fleet_host_part("café-serveur");
        let name = format!("b00t-dstack-fleet-{host_part}");
        let re = regex::Regex::new("^[a-z][a-z0-9-]{1,40}$").unwrap();
        assert!(re.is_match(&name), "fleet name violates dstack regex: {name}");
        assert_eq!(host_part, "caf-serveur");
    }

    #[test]
    fn sanitize_fleet_host_part_strips_cjk_characters() {
        let host_part = sanitize_fleet_host_part("東京-server");
        let re = regex::Regex::new("^[a-z0-9-]{1,40}$").unwrap();
        assert!(re.is_match(&host_part), "host part violates dstack regex: {host_part}");
        assert!(host_part.is_ascii());
    }

    #[test]
    fn dstack_scratch_config_path_rejects_dotdot_traversal() {
        assert!(dstack_scratch_config_path("../evil", "task").is_err());
    }

    #[test]
    fn dstack_scratch_config_path_rejects_path_separator() {
        assert!(dstack_scratch_config_path("a/b", "task").is_err());
        assert!(dstack_scratch_config_path("a\\b", "task").is_err());
    }

    #[test]
    fn dstack_scratch_config_path_accepts_normal_name() {
        let path = dstack_scratch_config_path("my-job", "task")
            .expect("plain name should be accepted");
        assert!(path.to_string_lossy().contains("my-job"));
    }

    #[test]
    fn copy_dstack_inputs_errors_on_missing_input_file() {
        // Regression guard: a missing input file previously only logged
        // tracing::warn! and continued, letting the job submit to dstack
        // without a file it needed. It must now fail fast locally instead.
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("does-not-exist.json");
        let err = copy_dstack_inputs(&[missing.to_str().unwrap().to_string()], dir.path())
            .unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn copy_dstack_inputs_copies_existing_file_into_dest_dir() {
        let src_dir = tempfile::tempdir().unwrap();
        let dest_dir = tempfile::tempdir().unwrap();
        let src_file = src_dir.path().join("photo.png");
        std::fs::write(&src_file, b"fake-image-bytes").unwrap();
        copy_dstack_inputs(&[src_file.to_str().unwrap().to_string()], dest_dir.path())
            .expect("existing input file should copy successfully");
        let copied = dest_dir.path().join("photo.png");
        assert!(copied.exists());
        assert_eq!(std::fs::read(&copied).unwrap(), b"fake-image-bytes");
    }
}

#[cfg(test)]
mod runpod_tests {
    use super::*;
    use std::sync::Mutex;

    // ── Mock transport ────────────────────────────────────────────────────────

    /// In-memory `RunpodApi`: canned responses, a call log, captured request
    /// inputs, and a `fail` switch. When `fail` is set every call returns Err,
    /// exercising the provider's error-context propagation with no network.
    struct MockRunpod {
        calls: Mutex<Vec<String>>,
        last_pod_input: Mutex<Option<PodCreateInput>>,
        last_endpoint_input: Mutex<Option<EndpointCreateInput>>,
        pod: Pod,
        pods: Vec<Pod>,
        endpoint: Endpoint,
        endpoints: Vec<Endpoint>,
        fail: bool,
    }

    impl Default for MockRunpod {
        fn default() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                last_pod_input: Mutex::new(None),
                last_endpoint_input: Mutex::new(None),
                pod: test_pod("mock-pod", None),
                pods: Vec::new(),
                endpoint: test_endpoint("mock-endpoint", "mock-endpoint"),
                endpoints: Vec::new(),
                fail: false,
            }
        }
    }

    impl MockRunpod {
        fn call_log(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }

        fn last_pod_input(&self) -> PodCreateInput {
            self.last_pod_input
                .lock()
                .unwrap()
                .clone()
                .expect("expected a create_pod call")
        }

        fn last_endpoint_input(&self) -> EndpointCreateInput {
            self.last_endpoint_input
                .lock()
                .unwrap()
                .clone()
                .expect("expected a create_endpoint call")
        }
    }

    #[async_trait]
    impl RunpodApi for MockRunpod {
        async fn create_pod(&self, input: PodCreateInput) -> Result<Pod> {
            self.calls.lock().unwrap().push("create_pod".into());
            *self.last_pod_input.lock().unwrap() = Some(input);
            if self.fail {
                bail!("mock create_pod failure");
            }
            Ok(self.pod.clone())
        }

        async fn get_pod(&self, id: &str, _query: GetPodQuery) -> Result<Pod> {
            self.calls.lock().unwrap().push(format!("get_pod:{id}"));
            if self.fail {
                bail!("mock get_pod failure");
            }
            Ok(self.pod.clone())
        }

        async fn delete_pod(&self, id: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("delete_pod:{id}"));
            if self.fail {
                bail!("mock delete_pod failure");
            }
            Ok(())
        }

        async fn list_pods(&self, _query: ListPodsQuery) -> Result<Vec<Pod>> {
            self.calls.lock().unwrap().push("list_pods".into());
            if self.fail {
                bail!("mock list_pods failure");
            }
            Ok(self.pods.clone())
        }

        async fn create_endpoint(&self, input: EndpointCreateInput) -> Result<Endpoint> {
            self.calls.lock().unwrap().push("create_endpoint".into());
            *self.last_endpoint_input.lock().unwrap() = Some(input);
            if self.fail {
                bail!("mock create_endpoint failure");
            }
            Ok(self.endpoint.clone())
        }

        async fn get_endpoint(&self, id: &str, _query: GetEndpointQuery) -> Result<Endpoint> {
            self.calls.lock().unwrap().push(format!("get_endpoint:{id}"));
            if self.fail {
                bail!("mock get_endpoint failure");
            }
            Ok(self.endpoint.clone())
        }

        async fn delete_endpoint(&self, id: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("delete_endpoint:{id}"));
            if self.fail {
                bail!("mock delete_endpoint failure");
            }
            Ok(())
        }

        async fn list_endpoints(&self, _query: ListEndpointsQuery) -> Result<Vec<Endpoint>> {
            self.calls.lock().unwrap().push("list_endpoints".into());
            if self.fail {
                bail!("mock list_endpoints failure");
            }
            Ok(self.endpoints.clone())
        }
    }

    // ── Test fixtures ─────────────────────────────────────────────────────────

    /// Builds a minimal `Pod` via JSON: every field except `id` is `Option`,
    /// so omitted keys deserialize to `None`. `PodStatus` serializes as
    /// UPPERCASE ("RUNNING"/"EXITED"/"TERMINATED").
    fn test_pod(id: &str, status: Option<PodStatus>) -> Pod {
        let mut v = serde_json::json!({
            "id": id,
            "name": format!("name-{id}"),
            "costPerHr": 0.39,
        });
        if let Some(s) = status {
            v["desiredStatus"] = serde_json::to_value(s).expect("PodStatus serializes");
        }
        serde_json::from_value(v).expect("test pod json should deserialize")
    }

    /// Builds a minimal `Endpoint` via JSON — supplies every non-Option field
    /// required by the SDK struct.
    fn test_endpoint(id: &str, name: &str) -> Endpoint {
        serde_json::from_value(serde_json::json!({
            "id": id,
            "name": name,
            "userId": "user-1",
            "templateId": "tpl-1",
            "version": 1,
            "computeType": "GPU",
            "createdAt": "2026-01-01T00:00:00Z",
            "dataCenterIds": [],
            "executionTimeoutMs": 30000,
            "idleTimeout": 5,
            "scalerType": "QUEUE_DELAY",
            "scalerValue": 4,
            "workersMax": 3,
            "workersMin": 0,
        }))
        .expect("test endpoint json should deserialize")
    }

    fn training_spec() -> TrainingJobSpec {
        TrainingJobSpec {
            config_path: "fine-tune/config.yaml".into(),
            image: "ghcr.io/elasticdotventures/b00t-training-image:latest".into(),
            flavor: "a100-large".into(),
            timeout_hours: 10.0,
        }
    }

    fn batch_spec(config_path: &str) -> BatchJobSpec {
        BatchJobSpec {
            image: "app4dog/sam3-runner:cloud".into(),
            config_path: config_path.into(),
            env: [("SAM_RUNNER_MODE".to_string(), "real".to_string())].into(),
            flavor: "a10g-small".into(),
            timeout_hours: 1.0,
            gpu_count: 1,
            volumes: vec![],
            inputs: vec![],
        }
    }

    fn failing_provider() -> RunpodProvider<MockRunpod> {
        let mut mock = MockRunpod::default();
        mock.fail = true;
        RunpodProvider::with_client(mock)
    }

    // ── Pure helper tests ─────────────────────────────────────────────────────

    #[test]
    fn hf_flavor_to_runpod_gpu_maps_known_flavors_and_defaults() {
        assert_eq!(hf_flavor_to_runpod_gpu("a100-large"), "NVIDIA A100 80GB PCIe");
        assert_eq!(hf_flavor_to_runpod_gpu("a100"), "NVIDIA A100 80GB PCIe");
        assert_eq!(hf_flavor_to_runpod_gpu("h100"), "NVIDIA H100 PCIe");
        assert_eq!(hf_flavor_to_runpod_gpu("a10g-large"), "NVIDIA A40");
        assert_eq!(hf_flavor_to_runpod_gpu("a10g-small"), "NVIDIA A40");
        // Unknown flavors fall back to the default GPU
        assert_eq!(hf_flavor_to_runpod_gpu("rtx-4090"), "NVIDIA A40");
    }

    #[test]
    fn default_gpu_count_is_one() {
        assert_eq!(default_gpu_count(), 1);
    }

    #[test]
    fn fmt_cost_formats_dollars_or_dash() {
        assert_eq!(fmt_cost(Some(1.234)), "$1.23");
        assert_eq!(fmt_cost(Some(0.0)), "$0.00");
        assert_eq!(fmt_cost(None), "-");
    }

    #[test]
    fn fmt_pod_status_line_includes_status_and_cost() {
        let line = fmt_pod_status_line("pod-1", Some(PodStatus::Running), Some(0.39));
        assert!(line.contains("pod=pod-1"));
        assert!(line.contains("status=Running"));
        assert!(line.contains("cost_per_hr=$0.39"));
        let blank = fmt_pod_status_line("pod-2", None, None);
        assert!(blank.contains("status="));
        assert!(blank.contains("cost_per_hr=-"));
    }

    #[test]
    fn parse_gpu_type_id_rejects_unknown_string_with_context() {
        let err = parse_gpu_type_id("BOGUS GPU").unwrap_err();
        assert!(err.to_string().contains("unknown GPU type 'BOGUS GPU'"));
    }

    #[test]
    fn parse_gpu_type_id_accepts_known_gpu() {
        assert_eq!(parse_gpu_type_id("NVIDIA A40").unwrap(), GpuTypeId::NvidiaA40);
    }

    #[test]
    fn training_pod_env_injects_config_and_cache_dir() {
        let env = training_pod_env("/cfg.yaml");
        assert_eq!(env.get("TRAINING_CONFIG").map(String::as_str), Some("/cfg.yaml"));
        assert_eq!(
            env.get("UNSLOTH_CACHE_DIR").map(String::as_str),
            Some("/opt/unsloth_compiled_cache")
        );
    }

    #[test]
    fn docker_start_cmd_for_omits_override_for_dev_null_or_empty() {
        assert_eq!(docker_start_cmd_for("/dev/null"), None);
        assert_eq!(docker_start_cmd_for(""), None);
        assert_eq!(docker_start_cmd_for("   /dev/null   "), None);
    }

    #[test]
    fn docker_start_cmd_for_overrides_with_bash_for_real_path() {
        assert_eq!(
            docker_start_cmd_for("/workspace/request.json"),
            Some(vec!["bash".to_string(), "-c".to_string(), "/workspace/request.json".to_string()])
        );
    }

    #[test]
    fn training_pod_request_builds_expected_input() {
        let req = training_pod_request(&training_spec()).expect("known flavor should parse");
        assert_eq!(req.name.as_deref(), Some("b00t-training"));
        assert_eq!(
            req.image_name.as_deref(),
            Some("ghcr.io/elasticdotventures/b00t-training-image:latest")
        );
        assert_eq!(req.gpu_type_ids, Some(vec![GpuTypeId::NvidiaA100_80GbPcie]));
        assert_eq!(req.cloud_type, Some(CloudType::Secure));
        assert_eq!(req.gpu_count, Some(1));
        assert_eq!(req.volume_in_gb, Some(50));
        assert_eq!(req.container_disk_in_gb, Some(20));
        assert_eq!(req.docker_start_cmd, None);
        let env = req.env.as_ref().expect("training env should be set");
        assert_eq!(env.get("TRAINING_CONFIG").map(String::as_str), Some("fine-tune/config.yaml"));
    }

    #[test]
    fn batch_pod_request_overrides_start_cmd_for_real_config_path() {
        let req = batch_pod_request(&batch_spec("/workspace/request.json")).expect("known flavor");
        assert_eq!(req.name.as_deref(), Some("b00t-batch"));
        assert_eq!(
            req.docker_start_cmd,
            Some(vec!["bash".to_string(), "-c".to_string(), "/workspace/request.json".to_string()])
        );
        let env = req.env.as_ref().expect("batch env should be preserved");
        assert_eq!(env.get("SAM_RUNNER_MODE").map(String::as_str), Some("real"));
    }

    #[test]
    fn batch_pod_request_leaves_start_cmd_unset_for_dev_null() {
        let req = batch_pod_request(&batch_spec("/dev/null")).expect("known flavor");
        assert_eq!(req.docker_start_cmd, None);
    }

    #[test]
    fn endpoint_create_request_uses_template_id_from_env() {
        let mut cfg = EndpointConfig {
            name: "b00t-ch0nky".into(),
            workers_min: 0,
            workers_max: 3,
            idle_timeout_s: 5,
            execution_timeout_ms: 30_000,
            image: "vllm/vllm-openai:latest".into(),
            network_volume_id: Some("vol-9".into()),
            ..Default::default()
        };
        cfg.env.insert("RUNPOD_TEMPLATE_ID".into(), "tpl-7".into());
        let req = endpoint_create_request(&cfg);
        assert_eq!(req.template_id, "tpl-7");
        assert_eq!(req.name.as_deref(), Some("b00t-ch0nky"));
        assert_eq!(req.workers_min, Some(0));
        assert_eq!(req.workers_max, Some(3));
        assert_eq!(req.idle_timeout, Some(5));
        assert_eq!(req.execution_timeout_ms, Some(30_000));
        assert_eq!(req.network_volume_id.as_deref(), Some("vol-9"));
    }

    #[test]
    fn endpoint_create_request_defaults_template_id_when_missing() {
        let req = endpoint_create_request(&EndpointConfig::default());
        assert_eq!(req.template_id, "");
    }

    // ── Error-path context propagation ─────────────────────────────────────────

    #[tokio::test]
    async fn submit_training_job_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let err = provider.submit_training_job(&training_spec()).await.unwrap_err();
        assert!(err.to_string().contains("RunPod create_pod failed"), "{err}");
        let chain = format!("{err:?}");
        assert!(chain.contains("mock create_pod failure"), "{chain}");
    }

    #[tokio::test]
    async fn submit_batch_job_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let err = provider
            .submit_batch_job(&batch_spec("/workspace/request.json"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("RunPod create_pod failed"), "{err}");
    }

    #[tokio::test]
    async fn job_status_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let handle = JobHandle { id: "pod-1".into(), provider: "runpod".into() };
        let err = provider.job_status(&handle).await.unwrap_err();
        assert!(err.to_string().contains("RunPod get_pod failed"), "{err}");
    }

    #[tokio::test]
    async fn cancel_job_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let handle = JobHandle { id: "pod-1".into(), provider: "runpod".into() };
        let err = provider.cancel_job(&handle).await.unwrap_err();
        assert!(err.to_string().contains("RunPod delete_pod failed"), "{err}");
    }

    #[tokio::test]
    async fn list_jobs_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let err = provider.list_jobs().await.unwrap_err();
        assert!(err.to_string().contains("RunPod list_pods failed"), "{err}");
    }

    #[tokio::test]
    async fn deploy_inference_endpoint_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let err = provider
            .deploy_inference_endpoint(&EndpointConfig::default())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("RunPod create_endpoint failed"), "{err}");
    }

    #[tokio::test]
    async fn endpoint_status_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let err = provider.endpoint_status("ep-1").await.unwrap_err();
        assert!(err.to_string().contains("RunPod get_endpoint failed"), "{err}");
    }

    #[tokio::test]
    async fn teardown_endpoint_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let err = provider.teardown_endpoint("ep-1").await.unwrap_err();
        assert!(err.to_string().contains("RunPod delete_endpoint failed"), "{err}");
    }

    #[tokio::test]
    async fn list_endpoints_propagates_mock_error_with_context() {
        let provider = failing_provider();
        let err = provider.list_endpoints().await.unwrap_err();
        assert!(err.to_string().contains("RunPod list_endpoints failed"), "{err}");
    }

    // ── Mock-driven lifecycle ─────────────────────────────────────────────────

    #[tokio::test]
    async fn submit_training_job_returns_handle_from_mock_pod() {
        let mut mock = MockRunpod::default();
        mock.pod = test_pod("pod-42", Some(PodStatus::Running));
        let provider = RunpodProvider::with_client(mock);
        let handle = provider.submit_training_job(&training_spec()).await.unwrap();
        assert_eq!(handle.id, "pod-42");
        assert_eq!(handle.provider, "runpod");
        assert_eq!(provider.client.call_log(), ["create_pod"]);
    }

    #[tokio::test]
    async fn submit_batch_job_passes_env_and_start_cmd_to_transport() {
        let mock = MockRunpod::default();
        let provider = RunpodProvider::with_client(mock);
        let handle = provider
            .submit_batch_job(&batch_spec("/workspace/request.json"))
            .await
            .unwrap();
        assert_eq!(handle.id, "mock-pod");
        let input = provider.client.last_pod_input();
        assert_eq!(
            input.docker_start_cmd.as_deref(),
            Some(&["bash".to_string(), "-c".to_string(), "/workspace/request.json".to_string()][..])
        );
        let env = input.env.as_ref().expect("env forwarded to transport");
        assert_eq!(env.get("SAM_RUNNER_MODE").map(String::as_str), Some("real"));
    }

    #[tokio::test]
    async fn job_status_formats_desired_status() {
        let mut mock = MockRunpod::default();
        mock.pod = test_pod("pod-7", Some(PodStatus::Running));
        let provider = RunpodProvider::with_client(mock);
        let handle = JobHandle { id: "pod-7".into(), provider: "runpod".into() };
        let status = provider.job_status(&handle).await.unwrap();
        // `{:?}` on Option<PodStatus> yields "Some(Running)" — matches the
        // pre-existing provider formatting exactly.
        assert_eq!(status, "pod=pod-7 status=Some(Running)");
        assert_eq!(provider.client.call_log(), ["get_pod:pod-7"]);
    }

    #[tokio::test]
    async fn job_status_handles_missing_desired_status() {
        let mock = MockRunpod::default(); // default pod has no desired_status
        let provider = RunpodProvider::with_client(mock);
        let handle = JobHandle { id: "pod-x".into(), provider: "runpod".into() };
        let status = provider.job_status(&handle).await.unwrap();
        assert_eq!(status, "pod=pod-x status=None");
    }

    #[tokio::test]
    async fn cancel_job_deletes_pod_by_id() {
        let mock = MockRunpod::default();
        let provider = RunpodProvider::with_client(mock);
        let handle = JobHandle { id: "pod-9".into(), provider: "runpod".into() };
        provider.cancel_job(&handle).await.unwrap();
        assert_eq!(provider.client.call_log(), ["delete_pod:pod-9"]);
    }

    #[tokio::test]
    async fn list_jobs_maps_mock_pods_to_handles() {
        let mut mock = MockRunpod::default();
        mock.pods = vec![test_pod("pod-a", None), test_pod("pod-b", None)];
        let provider = RunpodProvider::with_client(mock);
        let jobs = provider.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 2);
        assert_eq!(jobs[0].id, "pod-a");
        assert_eq!(jobs[1].id, "pod-b");
        assert!(jobs.iter().all(|j| j.provider == "runpod"));
        assert_eq!(provider.client.call_log(), ["list_pods"]);
    }

    #[tokio::test]
    async fn deploy_inference_endpoint_returns_handle() {
        let mut mock = MockRunpod::default();
        mock.endpoint = test_endpoint("ep-1", "b00t-ch0nky");
        let provider = RunpodProvider::with_client(mock);
        let mut cfg = EndpointConfig::default();
        cfg.name = "b00t-ch0nky".into();
        let handle = provider.deploy_inference_endpoint(&cfg).await.unwrap();
        assert_eq!(handle.id, "ep-1");
        assert_eq!(handle.provider, "runpod");
        assert_eq!(handle.name.as_deref(), Some("b00t-ch0nky"));
        assert_eq!(provider.client.call_log(), ["create_endpoint"]);
        assert_eq!(
            provider.client.last_endpoint_input().name.as_deref(),
            Some("b00t-ch0nky")
        );
    }

    #[tokio::test]
    async fn endpoint_status_returns_handle_from_mock_endpoint() {
        let mut mock = MockRunpod::default();
        mock.endpoint = test_endpoint("ep-2", "b00t-ch0nky");
        let provider = RunpodProvider::with_client(mock);
        let handle = provider.endpoint_status("ep-2").await.unwrap();
        assert_eq!(handle.id, "ep-2");
        assert_eq!(handle.provider, "runpod");
        assert_eq!(provider.client.call_log(), ["get_endpoint:ep-2"]);
    }

    #[tokio::test]
    async fn teardown_endpoint_deletes_by_id() {
        let mock = MockRunpod::default();
        let provider = RunpodProvider::with_client(mock);
        provider.teardown_endpoint("ep-3").await.unwrap();
        assert_eq!(provider.client.call_log(), ["delete_endpoint:ep-3"]);
    }

    #[tokio::test]
    async fn list_endpoints_maps_to_handles() {
        let mut mock = MockRunpod::default();
        mock.endpoints = vec![test_endpoint("ep-1", "a"), test_endpoint("ep-2", "b")];
        let provider = RunpodProvider::with_client(mock);
        let endpoints = provider.list_endpoints().await.unwrap();
        assert_eq!(endpoints.len(), 2);
        assert_eq!(endpoints[0].id, "ep-1");
        assert_eq!(endpoints[1].id, "ep-2");
        assert_eq!(provider.client.call_log(), ["list_endpoints"]);
    }

    #[tokio::test]
    async fn lifecycle_records_calls_in_order() {
        let mock = MockRunpod::default();
        let provider = RunpodProvider::with_client(mock);
        let handle = provider
            .submit_batch_job(&batch_spec("/workspace/request.json"))
            .await
            .unwrap();
        provider.job_status(&handle).await.unwrap();
        provider.cancel_job(&handle).await.unwrap();
        assert_eq!(
            provider.client.call_log(),
            ["create_pod", "get_pod:mock-pod", "delete_pod:mock-pod"]
        );
    }
}
