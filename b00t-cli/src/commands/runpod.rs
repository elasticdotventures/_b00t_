use anyhow::{Context, Result, bail};
use clap::Subcommand;
use runpod_sdk::model::{CloudType, GpuTypeId, ListPodsQuery, PodCreateInput};
use runpod_sdk::service::PodsService;
use runpod_sdk::{RunpodClient, RunpodConfig};
use std::collections::HashMap;

#[derive(Subcommand, Debug, Clone)]
pub enum RunpodCommands {
    #[clap(about = "Verify API key + list available GPU types")]
    Ping,
    #[clap(about = "List running pods")]
    Pods,
    #[clap(about = "Stop a pod")]
    Stop { id: String },
    #[clap(about = "Delete a pod")]
    Delete { id: String },
    #[clap(about = "Get container logs (requires supportPublicIp=true on pod creation)")]
    Logs { id: String },
    #[clap(about = "Launch a training pod using a b00t fine-tune config YAML")]
    Train {
        #[clap(long, default_value = "fine-tune/config-smol.yaml")]
        config: String,
        #[clap(long, default_value = "NVIDIA RTX 4090", help = "GPU type override")]
        gpu: String,
        #[clap(long, action, help = "Use spot pricing (cheaper, interruptible)")]
        spot: bool,
        #[clap(long, action, help = "Dry-run: print pod spec without launching")]
        dry_run: bool,
    },
}

fn load_key() -> Result<String> {
    if let Ok(k) = std::env::var("RUNPOD_API_KEY") {
        if !k.is_empty() {
            return Ok(k);
        }
    }
    let env_path = dirs::home_dir().unwrap_or_default().join(".b00t/.env");
    if env_path.exists() {
        for line in std::fs::read_to_string(&env_path)?.lines() {
            if let Some(rest) = line.strip_prefix("RUNPOD_API_KEY=") {
                let key = rest.trim().trim_matches('"').trim_matches('\'').to_string();
                if !key.is_empty() {
                    return Ok(key);
                }
            }
        }
    }
    bail!("RUNPOD_API_KEY not set — add it to ~/.b00t/.env or set the env var");
}

fn build_client(key: &str) -> Result<RunpodClient> {
    let config = RunpodConfig::builder()
        .with_api_key(key)
        .build()
        .context("RunpodConfig build failed")?;
    RunpodClient::new(config).context("RunpodClient::new failed")
}

pub async fn handle_runpod(command: RunpodCommands) -> Result<()> {
    match command {
        RunpodCommands::Ping => {
            let key = load_key()?;
            let client = build_client(&key)?;
            let pods = client
                .list_pods(ListPodsQuery::default())
                .await
                .context("list_pods failed")?;
            println!("PASS — API key valid; {} pod(s) visible", pods.len());
        }

        RunpodCommands::Pods => {
            let key = load_key()?;
            let client = build_client(&key)?;
            let pods = client
                .list_pods(ListPodsQuery::default())
                .await
                .context("list_pods failed")?;
            if pods.is_empty() {
                println!("No pods running.");
            } else {
                for p in &pods {
                    println!(
                        "{} — {} — {:?}",
                        p.id,
                        p.name.as_deref().unwrap_or("?"),
                        p.desired_status
                    );
                }
            }
        }

        RunpodCommands::Stop { id } => {
            let key = load_key()?;
            let client = build_client(&key)?;
            client.stop_pod(&id).await.context("stop_pod failed")?;
            println!("stopped {id}");
        }

        RunpodCommands::Delete { id } => {
            let key = load_key()?;
            let client = build_client(&key)?;
            client.delete_pod(&id).await.context("delete_pod failed")?;
            println!("deleted {id}");
        }

        RunpodCommands::Logs { id } => {
            let key = load_key()?;
            // 🤓 hapi.runpod.net logs require supportPublicIp=true on pod creation.
            let url = format!("https://hapi.runpod.net/v1/pod/{id}/logs");
            let resp = reqwest::Client::new()
                .get(&url)
                .bearer_auth(&key)
                .send()
                .await
                .context("logs request failed")?;
            if !resp.status().is_success() {
                bail!(
                    "logs unavailable ({}): pod must be created with support_public_ip=true — use `runpod pods` to check status",
                    resp.status()
                );
            }
            let body: serde_json::Value = resp.json().await.context("logs JSON parse failed")?;
            let lines = body["container"].as_array().cloned().unwrap_or_default();
            for l in lines {
                println!("{}", l.as_str().unwrap_or(""));
            }
        }

        RunpodCommands::Train {
            config,
            gpu,
            spot,
            dry_run,
        } => {
            // Load fine-tune config YAML for base model name
            let yaml_src = std::fs::read_to_string(&config)
                .with_context(|| format!("read config {config}"))?;
            let yaml: serde_yaml::Value =
                serde_yaml::from_str(&yaml_src).context("parse config YAML")?;
            let base_model = yaml["base_model"].as_str().unwrap_or("unknown").to_string();

            // runpod/pytorch has bash + cuda + python pre-installed; simpler than nvcr.io NGC image
            let image = "runpod/pytorch:2.4.0-py3.11-cuda12.4.1-devel-ubuntu22.04".to_string();
            let startup_cmd = format!(
                "set -euo pipefail; \
                 export PATH=\"$HOME/.local/bin:$PATH\"; \
                 curl -LsSf https://astral.sh/uv/install.sh | sh 2>&1 | tail -1; \
                 uv pip install --system -q 'torch==2.6.0' --index-url https://download.pytorch.org/whl/cu124 2>&1 | tail -2; \
                 uv pip install --system -q --no-build-isolation \
                   'unsloth[cu124-ampere-torch260]' \
                   mlflow pyyaml datasets trl 2>&1 | tail -5; \
                 git clone --depth=1 https://github.com/elasticdotventures/_b00t_.git /workspace/b00t; \
                 cd /workspace/b00t; \
                 python3 fine-tune/train_unsloth.py --config {config} 2>&1 | tee /workspace/train.log; \
                 echo DONE"
            );

            if dry_run {
                println!("--- dry-run pod spec ---");
                println!("image:   {image}");
                println!("gpu:     {gpu}");
                println!("model:   {base_model}");
                println!("spot:    {spot}");
                println!("cmd:     {startup_cmd}");
                return Ok(());
            }

            // Parse GPU type string → typed enum (rejects unknown GPU types at call site)
            let gpu_id: GpuTypeId = serde_json::from_value(serde_json::Value::String(gpu.clone()))
                .with_context(|| {
                    format!(
                        "unknown GPU type '{gpu}' — run `b00t-cli runpod ping` to list valid types"
                    )
                })?;

            // Collect env vars (log keys only — never log values)
            let mut env: HashMap<String, String> = HashMap::new();
            for var in &[
                "HF_TOKEN",
                "HUGGING_FACE_HUB_TOKEN",
                "NGC_API_KEY",
                "NVIDIA_API_KEY",
            ] {
                if let Ok(v) = std::env::var(var) {
                    env.insert(var.to_string(), v);
                }
            }
            // Also load from ~/.b00t/.env
            let env_path = dirs::home_dir().unwrap_or_default().join(".b00t/.env");
            if env_path.exists() {
                for line in std::fs::read_to_string(&env_path)?.lines() {
                    if line.starts_with('#') || !line.contains('=') {
                        continue;
                    }
                    let (k, v) = line.split_once('=').unwrap();
                    let k = k.trim().to_string();
                    if [
                        "HF_TOKEN",
                        "HUGGING_FACE_HUB_TOKEN",
                        "MLFLOW_TRACKING_URI",
                        "MLFLOW_EXPERIMENT_NAME",
                        "NGC_API_KEY",
                        "NVIDIA_API_KEY",
                    ]
                    .contains(&k.as_str())
                    {
                        env.entry(k).or_insert_with(|| {
                            v.trim().trim_matches('"').trim_matches('\'').to_string()
                        });
                    }
                }
            }
            let env_keys: Vec<&str> = env.keys().map(String::as_str).collect();
            eprintln!("env passed to pod: {}", env_keys.join(", "));

            if let Some(uri) = env.get("MLFLOW_TRACKING_URI") {
                if uri.is_empty() {
                    eprintln!("⚠️  MLFLOW_TRACKING_URI not set — training won't log to MLflow");
                }
            }

            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let pod_name = format!("b00t-train-{ts}");

            let key = load_key()?;
            let client = build_client(&key)?;

            let req = PodCreateInput {
                name: Some(pod_name.clone()),
                image_name: Some(image),
                gpu_type_ids: Some(vec![gpu_id]),
                cloud_type: Some(CloudType::Secure),
                gpu_count: Some(1),
                container_disk_in_gb: Some(80),
                // docker_entrypoint replaces ENTRYPOINT — the only REST-API-honored field for custom startup
                docker_entrypoint: Some(vec!["bash".to_string(), "-c".to_string(), startup_cmd]),
                support_public_ip: Some(true),
                interruptible: Some(spot),
                env: Some(env),
                ..Default::default()
            };

            let pod = client.create_pod(req).await.context("create_pod failed")?;
            let pod_id = pod.id;
            println!("training pod: {pod_id} ({pod_name})");
            println!("monitor: b00t-cli runpod logs {pod_id}");
        }
    }

    Ok(())
}
