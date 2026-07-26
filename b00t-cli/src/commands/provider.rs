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
    #[serde(default)]
    pub volumes: Vec<VolumeMount>,
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
    pub fn ensure_fleet(&self, name: &str) -> Result<()> {
        let yaml = dstack_fleet_yaml(name);
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
/// exists before scheduling a task — one pool, reused across jobs, rather
/// than a fleet per submission (matches the warm-reuse philosophy behind
/// Task 12's persistent volumes).
const SHARED_FLEET_NAME: &str = "b00t-dstack-fleet";

/// Generates a `type: fleet` autoscaling config: `nodes: 0..1` costs nothing
/// while idle (dstack only provisions compute once a task actually needs
/// scheduling capacity), `resources: gpu: 1` requests any single GPU rather
/// than a specific model. Deliberately omits `backends:`/`regions:` so it
/// matches whatever backend(s) the operator's dstack server config.yml has
/// configured (runpod today, potentially gcp/azure later) instead of
/// hardcoding one.
fn dstack_fleet_yaml(name: &str) -> String {
    format!("type: fleet\nname: {name}\nnodes: 0..1\nresources:\n  gpu: 1\n")
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
            volumes: vec![],
        };
        let yaml = dstack_task_yaml(&name, &batch_spec);
        submit_dstack_yaml(self, &name, &yaml)
    }

    async fn submit_batch_job(&self, spec: &BatchJobSpec) -> Result<JobHandle> {
        let name = format!("b00t-job-{}", dstack_short_id());
        let yaml = dstack_task_yaml(&name, spec);
        submit_dstack_yaml(self, &name, &yaml)
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

/// Write `yaml` to a temp file and `dstack apply -f <file> -y -d` it.
/// Split out so submit_batch_job/submit_training_job share one path.
/// Ensures the shared autoscaling fleet exists first — see
/// `DstackProvider::ensure_fleet` for why this is required against real
/// dstack 0.20.x, not optional.
fn submit_dstack_yaml(provider: &DstackProvider, name: &str, yaml: &str) -> Result<JobHandle> {
    provider
        .ensure_fleet(SHARED_FLEET_NAME)
        .context("ensuring shared dstack fleet exists before task submission")?;
    let tmp = dstack_scratch_config_path(name, "task")?;
    std::fs::write(&tmp, yaml).context("writing dstack task config")?;
    let tmp_path = tmp
        .to_str()
        .context("temp file path is not valid UTF-8")?;
    provider.run_dstack(&["apply", "-f", tmp_path, "-y", "-d"])?;
    // Cleanup is best-effort — a failure to remove the temp file must not
    // fail the job submission itself.
    if let Err(err) = std::fs::remove_file(&tmp) {
        tracing::warn!("failed to remove temp dstack task config {tmp:?}: {err}");
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
                volumes: vec![],
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
            volumes: vec![],
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
            volumes: vec![],
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
        let yaml = dstack_fleet_yaml(SHARED_FLEET_NAME);
        assert!(yaml.contains("type: fleet"));
        assert!(yaml.contains("name: b00t-dstack-fleet"));
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
            volumes: vec![VolumeMount { name: "b00t-mesh-cache".into(), path: "/cache".into() }],
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
            volumes: vec![],
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
}
