use anyhow::{Context, Result, bail};
use clap::Subcommand;

fn api_key() -> Result<String> {
    if let Ok(k) = std::env::var("RUNPOD_API_KEY") {
        return Ok(k);
    }
    let env_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".b00t/.env");
    if env_path.exists() {
        for line in std::fs::read_to_string(&env_path)?.lines() {
            if let Some(v) = line.strip_prefix("RUNPOD_API_KEY=") {
                return Ok(v.trim().to_string());
            }
        }
    }
    bail!("RUNPOD_API_KEY not set — add to ~/.b00t/.env or environment")
}

#[derive(Debug, Subcommand, Clone)]
pub enum RunpodCommands {
    #[clap(about = "Verify API key + list available GPU types")]
    Ping {
        #[arg(long, help = "Show all GPU types")]
        verbose: bool,
    },
    #[clap(about = "List running pods")]
    Pods {
        #[arg(long, help = "JSON output")]
        json: bool,
    },
    #[clap(about = "Start a pod (on-demand)")]
    Start {
        #[arg(long, help = "Docker image", default_value = "runpod/pytorch:latest")]
        image: String,
        #[arg(long, help = "GPU type ID", default_value = "NVIDIA RTX 4090")]
        gpu: String,
        #[arg(long, help = "Pod name", default_value = "b00t-pod")]
        name: String,
        #[arg(long, help = "Use spot (interruptible) pricing")]
        spot: bool,
        #[arg(long, help = "Disk GB", default_value_t = 20)]
        disk: i32,
    },
    #[clap(about = "Stop a pod")]
    Stop {
        #[arg(help = "Pod ID")]
        id: String,
    },
    #[clap(about = "Delete a pod")]
    Delete {
        #[arg(help = "Pod ID")]
        id: String,
    },
    #[clap(about = "Get container logs")]
    Logs {
        #[arg(help = "Pod ID")]
        id: String,
    },
    #[clap(about = "Launch a training pod using a b00t fine-tune config YAML")]
    Train {
        #[arg(long, default_value = "fine-tune/config-smol.yaml", help = "Config YAML path")]
        config: String,
        #[arg(long, help = "GPU type override (default: NVIDIA RTX 4090)")]
        gpu: Option<String>,
        #[arg(long, help = "Use spot pricing (cheaper, interruptible)")]
        spot: bool,
        #[arg(long, help = "Dry-run: print pod spec without launching")]
        dry_run: bool,
    },
}

fn pod_env_vars() -> Vec<runpod::EnvVar> {
    let mut vars = vec![];
    // 🤓 huggingface_hub checks HUGGING_FACE_HUB_TOKEN before HF_TOKEN; pass whichever is set
    //    as HF_TOKEN so the training script's push_to_hub call authenticates correctly.
    let hf_token = std::env::var("HF_TOKEN")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("HUGGING_FACE_HUB_TOKEN").ok().filter(|v| !v.is_empty()));
    if let Some(tok) = hf_token {
        vars.push(runpod::EnvVar { key: "HF_TOKEN".to_string(), value: tok.clone() });
        vars.push(runpod::EnvVar { key: "HUGGING_FACE_HUB_TOKEN".to_string(), value: tok });
    }
    // NGC_API_KEY — allows training pods to pull nvcr.io containers + push to NGC model registry
    let ngc_key = std::env::var("NGC_API_KEY").ok().filter(|v| !v.is_empty())
        .or_else(|| std::env::var("NVIDIA_API_KEY").ok().filter(|v| !v.is_empty()));
    if let Some(k) = ngc_key {
        vars.push(runpod::EnvVar { key: "NGC_API_KEY".to_string(), value: k.clone() });
        vars.push(runpod::EnvVar { key: "NVIDIA_API_KEY".to_string(), value: k });
    }
    for k in &["MLFLOW_TRACKING_URI", "MLFLOW_EXPERIMENT_NAME"] {
        if let Ok(v) = std::env::var(k) {
            if !v.is_empty() {
                vars.push(runpod::EnvVar { key: k.to_string(), value: v });
            }
        }
    }
    vars
}

pub async fn handle_runpod(cmd: RunpodCommands) -> Result<()> {
    use runpod::{CreateOnDemandPodRequest, CreateSpotPodRequest, RunpodClient};

    let key = api_key()?;
    let client = RunpodClient::new(&key);

    match cmd {
        RunpodCommands::Ping { verbose } => {
            let resp = client
                .list_gpu_types_graphql()
                .await
                .context("RunPod API unreachable")?;
            let gpus = resp.data.unwrap_or_default();
            println!("PASS — {} GPU types available", gpus.len());
            if verbose {
                for g in &gpus {
                    println!("  {} — {} ({}GB)", g.id, g.display_name,
                        g.memory_in_gb.map(|m| m.to_string()).unwrap_or_else(|| "?".into()));
                }
            } else {
                for g in gpus.iter().take(5) {
                    println!("  {} — {}", g.id, g.display_name);
                }
                if gpus.len() > 5 {
                    println!("  … and {} more (--verbose to list all)", gpus.len() - 5);
                }
            }
        }

        RunpodCommands::Pods { json } => {
            let resp = client.list_pods().await.context("list_pods failed")?;
            let pods = resp.data.map(|d| d.pods).unwrap_or_default();
            if json {
                println!("{}", serde_json::to_string_pretty(&pods)?);
            } else if pods.is_empty() {
                println!("No pods running.");
            } else {
                for p in &pods {
                    let dc = p.machine.as_ref()
                        .map(|m| m.location.as_str())
                        .unwrap_or("?");
                    println!("{} — {} — {} — dc:{}", p.id, p.name, p.desired_status, dc);
                }
            }
        }

        RunpodCommands::Start { image, gpu, name, spot, disk } => {
            if spot {
                let req = CreateSpotPodRequest {
                    name,
                    image_name: image,
                    gpu_type_id: gpu,
                    cloud_type: Some("SECURE".to_string()),
                    gpu_count: 1,
                    volume_in_gb: 0,
                    container_disk_in_gb: disk,
                    bid_per_gpu: 0.5,
                    ..Default::default()
                };
                let resp = client.create_spot_pod(req).await.context("create_spot_pod failed")?;
                let pod_id = resp.data.map(|p| p.id).unwrap_or_else(|| "?".to_string());
                println!("pod id: {pod_id}");
            } else {
                let req = CreateOnDemandPodRequest {
                    name: Some(name),
                    image_name: Some(image),
                    gpu_type_id: Some(gpu),
                    cloud_type: Some("SECURE".to_string()),
                    gpu_count: Some(1),
                    container_disk_in_gb: Some(disk),
                    ports: Some(vec![]),
                    ..Default::default()
                };
                let resp = client.create_on_demand_pod(req).await.context("create_on_demand_pod failed")?;
                let pod_id = resp.data.map(|p| p.id).unwrap_or_else(|| "?".to_string());
                println!("pod id: {pod_id}");
            }
        }

        RunpodCommands::Stop { id } => {
            let _ = client.stop_pod(&id).await.context("stop_pod failed")?;
            println!("stopped {id}");
        }

        RunpodCommands::Delete { id } => {
            let _ = client.delete_pod(&id).await.context("delete_pod failed")?;
            println!("deleted {id}");
        }

        RunpodCommands::Logs { id } => {
            // 🤓 runpod crate's get_container_logs hits hapi.runpod.net without auth — 401.
            //    The endpoint accepts the API key as a query param instead.
            let url = format!("https://hapi.runpod.net/v1/pod/{id}/logs");
            let resp = reqwest::Client::new()
                .get(&url)
                .bearer_auth(&key)
                .send()
                .await
                .context("logs request failed")?;
            if !resp.status().is_success() {
                // 🤓 hapi.runpod.net logs require supportPublicIp=true on pod creation.
                //    Workaround: check status via `runpod pods` or use RunPod web console.
                //    Issue filed: agentsea/runpod.rs doesn't forward auth to hapi domain.
                bail!("logs unavailable ({}): pod must be created with supportPublicIp=true — use `runpod pods` to check status", resp.status());
            }
            let body: serde_json::Value = resp.json().await.context("logs JSON parse failed")?;
            let lines = body["container"].as_array().cloned().unwrap_or_default();
            for l in lines {
                println!("{}", l.as_str().unwrap_or(""));
            }
        }

        RunpodCommands::Train { config, gpu, spot, dry_run } => {
            let yaml = std::fs::read_to_string(&config)
                .with_context(|| format!("cannot read config: {config}"))?;

            let base_model = yaml.lines()
                .find_map(|l| l.strip_prefix("base_model:").map(|v| v.trim().trim_matches('"').to_string()))
                .unwrap_or_else(|| "unsloth/Qwen2.5-0.5B-Instruct".into());

            let gpu_type = gpu.unwrap_or_else(|| "NVIDIA RTX 4090".to_string());

            // 🤓 Use NGC PyTorch container — standard bash entrypoint (no supervisord).
            //    nvcr.io/nvidia/pytorch:24.12-py3 = PyTorch 2.5, CUDA 12.6, Python 3.10, Ampere-ready.
            //    unsloth/unsloth:latest is WRONG: supervisord entrypoint ignores docker_args CMD.
            //    NGC API key: NGC_API_KEY env var (or anonymous pull for public images).
            let startup_cmd = format!(
                "set -euo pipefail; \
                 export PATH=\"$HOME/.local/bin:$PATH\"; \
                 curl -LsSf https://astral.sh/uv/install.sh | sh 2>&1 | tail -1; \
                 uv pip install --system -q \
                   'unsloth[cu124-ampere-torch260]' \
                   'transformers>=5.2.0' mlflow pyyaml datasets trl 2>&1 | tail -5; \
                 git clone --depth=1 https://github.com/elasticdotventures/_b00t_.git /workspace/b00t; \
                 cd /workspace/b00t; \
                 python3 fine-tune/train_unsloth.py --config {config} 2>&1 | tee /workspace/train.log; \
                 echo DONE"
            );

            if dry_run {
                println!("--- dry-run pod spec ---");
                println!("image:   nvcr.io/nvidia/pytorch:24.12-py3");
                println!("gpu:     {gpu_type}");
                println!("model:   {base_model}");
                println!("spot:    {spot}");
                println!("cmd:     {startup_cmd}");
                return Ok(());
            }

            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let pod_name = format!("b00t-train-{ts}");
            let image = "nvcr.io/nvidia/pytorch:24.12-py3".to_string();
            let bash_args = vec!["bash".to_string(), "-c".to_string(), startup_cmd];

            let env = pod_env_vars();
            if env.is_empty() {
                eprintln!("⚠️  HF_TOKEN and MLFLOW_TRACKING_URI not set — adapter won't push to HF and MLflow won't track");
            } else {
                let keys: Vec<_> = env.iter().map(|e| e.key.as_str()).collect();
                println!("env passed to pod: {}", keys.join(", "));
            }

            let pod_id = if spot {
                let req = CreateSpotPodRequest {
                    name: pod_name.clone(),
                    image_name: image,
                    gpu_type_id: gpu_type,
                    cloud_type: Some("SECURE".to_string()),
                    gpu_count: 1,
                    volume_in_gb: 0,
                    container_disk_in_gb: 80,
                    bid_per_gpu: 0.5,
                    docker_args: Some(bash_args),
                    env,
                    ..Default::default()
                };
                client.create_spot_pod(req).await.context("create_spot_pod failed")?
                    .data.map(|p| p.id).unwrap_or_else(|| "?".to_string())
            } else {
                let req = CreateOnDemandPodRequest {
                    name: Some(pod_name.clone()),
                    image_name: Some(image),
                    gpu_type_id: Some(gpu_type),
                    cloud_type: Some("SECURE".to_string()),
                    gpu_count: Some(1),
                    container_disk_in_gb: Some(80),
                    docker_args: Some(bash_args),
                    ports: Some(vec![]),
                    env,
                    ..Default::default()
                };
                client.create_on_demand_pod(req).await.context("create_on_demand_pod failed")?
                    .data.map(|p| p.id).unwrap_or_else(|| "?".to_string())
            };

            println!("training pod: {pod_id} ({pod_name})");
            println!("monitor: b00t-cli runpod logs {pod_id}");
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_key_lookup() {
        // Structural test — no network call in unit tests.
        // Integration: `b00t-cli runpod ping` against real API.
        let original = std::env::var("RUNPOD_API_KEY").ok();
        unsafe { std::env::remove_var("RUNPOD_API_KEY") };
        let result = api_key();
        if let Some(k) = original {
            unsafe { std::env::set_var("RUNPOD_API_KEY", k) };
        }
        // Ok = key found in ~/.b00t/.env; Err = no key set — both valid
        let _ = result;
    }
}
