//! Multi-provider compute abstraction — inference endpoints + training jobs.
//!
//! Providers: runpod (native crate), hf (CLI wrapper for `hf jobs`)
//! Single source of truth: PROVIDER-*.provider.tomllmd datums
//!
//! b00t provider endpoint deploy|status|teardown|list --provider runpod|hf
//! b00t provider job submit|status|cancel|list      --provider runpod|hf

use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::process::Command;

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
    async fn job_status(&self, handle: &JobHandle) -> Result<String>;
    async fn cancel_job(&self, handle: &JobHandle) -> Result<()>;
    async fn list_jobs(&self) -> Result<Vec<JobHandle>>;
}

pub fn get_provider(name: &str) -> Result<Box<dyn ComputeProvider>> {
    match name {
        "runpod" => Ok(Box::new(RunpodProvider::new()?)),
        "hf" => Ok(Box::new(HfProvider::new())),
        other => bail!("unknown provider '{}'; supported: runpod, hf", other),
    }
}

// ── RunPod provider ──────────────────────────────────────────────────────────

pub struct RunpodProvider {
    client: runpod::RunpodClient,
}

impl RunpodProvider {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("RUNPOD_API_KEY")
            .context("RUNPOD_API_KEY not set — see PROVIDER-RUNPOD.provider.tomllmd")?;
        Ok(Self {
            client: runpod::RunpodClient::new(api_key),
        })
    }
}

#[async_trait]
impl ComputeProvider for RunpodProvider {
    fn name(&self) -> &str {
        "runpod"
    }

    async fn deploy_inference_endpoint(&self, cfg: &EndpointConfig) -> Result<EndpointHandle> {
        use runpod::EndpointCreateInput;
        // 🤓 EndpointCreateInput requires template_id; env is baked into the template
        let template_id = cfg
            .env
            .get("RUNPOD_TEMPLATE_ID")
            .cloned()
            .unwrap_or_default();
        let input = EndpointCreateInput {
            template_id,
            name: Some(cfg.name.clone()),
            gpu_type_ids: Some(cfg.gpu_type_ids.clone()),
            workers_min: Some(cfg.workers_min),
            workers_max: Some(cfg.workers_max),
            idle_timeout: Some(cfg.idle_timeout_s),
            execution_timeout_ms: Some(cfg.execution_timeout_ms),
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
        let endpoint = self
            .client
            .get_endpoint(id)
            .await
            .context("RunPod get_endpoint failed")?;
        let worker_count = endpoint.workers.as_ref().map(|w| w.len()).unwrap_or(0);
        Ok(EndpointHandle {
            id: endpoint.id,
            provider: "runpod".into(),
            name: endpoint.name,
            status: Some(format!("workers={}", worker_count)),
        })
    }

    async fn teardown_endpoint(&self, id: &str) -> Result<()> {
        self.client
            .delete_endpoint(id)
            .await
            .context("RunPod delete_endpoint failed")
    }

    async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
        // list_endpoints returns Vec<Endpoint> (type alias Endpoints)
        let endpoints = self
            .client
            .list_endpoints()
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
        use runpod::{CreateOnDemandPodRequest, EnvVar};
        let gpu_type = hf_flavor_to_runpod_gpu(&spec.flavor).to_string();
        let env = vec![
            EnvVar {
                key: "TRAINING_CONFIG".into(),
                value: spec.config_path.clone(),
            },
            EnvVar {
                key: "UNSLOTH_CACHE_DIR".into(),
                value: "/opt/unsloth_compiled_cache".into(),
            },
        ];
        let req = CreateOnDemandPodRequest {
            name: Some("b00t-training".into()),
            image_name: Some(spec.image.clone()),
            gpu_type_id: Some(gpu_type),
            cloud_type: Some("SECURE".into()),
            gpu_count: Some(1),
            volume_in_gb: Some(50),
            container_disk_in_gb: Some(20),
            env,
            ..Default::default()
        };
        let pod = self
            .client
            .create_on_demand_pod(req)
            .await
            .context("RunPod create_on_demand_pod failed")?;
        // PodCreateResponseData.data: Option<Pod>; Pod.id: String
        let id = pod
            .data
            .context("RunPod returned no pod data")?
            .id;
        Ok(JobHandle {
            id,
            provider: "runpod".into(),
        })
    }

    async fn job_status(&self, handle: &JobHandle) -> Result<String> {
        let info = self
            .client
            .get_pod(&handle.id)
            .await
            .context("RunPod get_pod failed")?;
        // PodInfoResponseData.data: Option<PodInfoFull>; PodInfoFull.desired_status: String
        let status = info
            .data
            .map(|p| p.desired_status)
            .unwrap_or_else(|| "unknown".into());
        Ok(format!("pod={} status={}", handle.id, status))
    }

    async fn cancel_job(&self, handle: &JobHandle) -> Result<()> {
        self.client
            .delete_pod(&handle.id)
            .await
            .context("RunPod delete_pod failed")
    }

    async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
        let resp = self
            .client
            .list_pods()
            .await
            .context("RunPod list_pods failed")?;
        // PodsListResponseData.data: Option<MyselfPods>; MyselfPods.pods: Vec<PodInfoFull>
        let pods = resp.data.map(|m| m.pods).unwrap_or_default();
        Ok(pods
            .into_iter()
            .map(|p| JobHandle {
                id: p.id,
                provider: "runpod".into(),
            })
            .collect())
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
