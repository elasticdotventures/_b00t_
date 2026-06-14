use crate::model_manager::{
    ModelOperation, ModelRecord, ServeOptions, ServedEndpointRecord, activate_model,
    describe_model, download_model, export_model_env, list_models, list_served_models,
    remove_model, serve_model, stop_model,
};
use anyhow::{Result, anyhow};
use clap::Parser;
use serde_json::json;

#[derive(Parser)]
pub enum ModelCommands {
    #[clap(
        about = "List available AI model datums",
        long_about = "Enumerate AI model datums discovered in the _b00t_ directory."
    )]
    List {
        #[clap(long, help = "Emit JSON instead of human-readable output")]
        json: bool,
        #[clap(
            long,
            help = "Also list trained LoRA adapters from .b00t/models/adapters/"
        )]
        adapters: bool,
    },
    #[clap(
        about = "List models currently served by local inference endpoints",
        long_about = "Discover local OpenAI-compatible inference endpoints from model datums and query each endpoint's /v1/models response."
    )]
    Served {
        #[clap(long, help = "Emit JSON instead of human-readable output")]
        json: bool,
    },
    #[clap(
        about = "Show metadata for an AI model datum",
        long_about = "Display provider, capabilities, cache directories, and container mounts for a model."
    )]
    Info {
        #[clap(help = "Model name (defaults to active model if omitted)")]
        name: Option<String>,
        #[clap(long, help = "Emit JSON output")]
        json: bool,
    },
    #[clap(
        about = "Emit environment exports for a model",
        long_about = "Print environment exports suitable for direnv or shell usage."
    )]
    Env {
        #[clap(help = "Model name (defaults to active model if omitted)")]
        name: Option<String>,
        #[clap(long, help = "Emit KEY=VALUE pairs instead of export statements")]
        plain: bool,
        #[clap(long, help = "Emit JSON list of environment variables")]
        json: bool,
    },
    #[clap(
        about = "Download/cache model weights defined by the datum",
        long_about = "Use huggingface-cli to pull weights into the cache directory defined by the datum."
    )]
    Download {
        #[clap(help = "Model name to download")]
        name: String,
        #[clap(long, help = "Re-download even if cache exists")]
        force: bool,
        #[clap(long, help = "Skip activating model after successful download")]
        no_activate: bool,
    },
    #[clap(
        about = "Remove cached weights for a model",
        long_about = "Delete the cache directory associated with the model datum."
    )]
    Remove {
        #[clap(help = "Model name to remove")]
        name: String,
        #[clap(long, help = "Confirm removal without prompting")]
        yes: bool,
    },
    #[clap(
        about = "Mark a datum as the active model for env exports",
        long_about = "Persist the model name under ~/.b00t/models/active-model so env commands and tooling can default to it."
    )]
    Activate {
        #[clap(help = "Model name to mark as active")]
        name: String,
    },
    #[clap(
        about = "Launch a vLLM container for a cached model",
        long_about = "Start a docker container using the cache and metadata defined in the model datum."
    )]
    Serve {
        #[clap(help = "Model name (defaults to active model if omitted)")]
        name: Option<String>,
        #[clap(long, help = "Load LoRA adapter trained via `b00t model train`")]
        adapter: Option<String>,
        #[clap(long, help = "Override container port (default 8000)")]
        port: Option<u16>,
        #[clap(long, help = "Override dtype passed to vLLM")]
        dtype: Option<String>,
        #[clap(long, help = "Override docker image")]
        image: Option<String>,
        #[clap(long, help = "Override container name")]
        container: Option<String>,
        #[clap(long = "tp-size", help = "Tensor parallel size", default_value = "1")]
        tensor_parallel_size: u32,
        #[clap(long = "arg", help = "Additional arguments passed to docker run", num_args = 1..)]
        extra_args: Vec<String>,
        #[clap(long, help = "Do not request GPU devices")]
        no_gpu: bool,
        #[clap(long, help = "Do not replace an existing container of the same name")]
        no_replace: bool,
    },
    #[clap(
        about = "Stop a vLLM container",
        long_about = "Stop and remove the docker container launched for serving a model."
    )]
    Stop {
        #[clap(help = "Container name (defaults to vllm-server if omitted)")]
        container: Option<String>,
    },
    #[clap(
        about = "LoRA fine-tune a model using candle",
        long_about = "Train a LoRA adapter using FSL examples and a .training.tomllmd datum.\n\n\
                      Steps:\n  1. Read training datum from _b00t_/<name>.training.tomllmd\n  \
                      2. Load FSL examples from .b00t/fsl/\n  \
                      3. Run candle LoRA training\n  \
                      4. Save adapter to .b00t/models/adapters/<name>/"
    )]
    Train {
        #[clap(
            help = "Training datum name (e.g. focus-validator for _b00t_/focus-validator.training.tomllmd)"
        )]
        name: String,
        #[clap(long, help = "Override learning rate", default_value = "1e-4")]
        learning_rate: f64,
        #[clap(long, help = "Override number of epochs", default_value_t = 3)]
        epochs: u32,
        #[clap(long, help = "LoRA rank", default_value_t = 8)]
        lora_r: u32,
    },
    #[clap(about = "Remove unused model weights and adapters")]
    Prune {
        #[clap(long, help = "Dry run — show what would be removed without deleting")]
        dry_run: bool,
        #[clap(long, help = "Skip confirmation prompt")]
        yes: bool,
    },
    #[clap(about = "Quick inference smoke test against served endpoint")]
    Test {
        #[clap(
            long,
            help = "Model endpoint URL",
            default_value = "http://localhost:8001"
        )]
        endpoint: String,
        #[clap(long, help = "Prompt to send", default_value = "say hello in 3 words")]
        prompt: String,
        #[clap(long, help = "Max tokens", default_value_t = 50)]
        max_tokens: u32,
    },
}

impl ModelCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            ModelCommands::List { json, adapters } => list_models_cmd(path, *json, *adapters),
            ModelCommands::Served { .. } => {
                unreachable!("use ModelCommands::execute_async for served")
            }
            ModelCommands::Info { name, json } => info_cmd(path, name.as_deref(), *json),
            ModelCommands::Env { name, plain, json } => {
                env_cmd(path, name.as_deref(), *plain, *json)
            }
            ModelCommands::Download {
                name,
                force,
                no_activate,
            } => download_cmd(path, name, *force, !*no_activate),
            ModelCommands::Remove { name, yes } => remove_cmd(path, name, *yes),
            ModelCommands::Activate { name } => activate_model(path, name).map_err(Into::into),
            ModelCommands::Serve {
                name,
                adapter,
                port,
                dtype,
                image,
                container,
                tensor_parallel_size,
                extra_args,
                no_gpu,
                no_replace,
            } => serve_cmd(
                path,
                name.as_deref(),
                adapter,
                *port,
                dtype.clone(),
                image.clone(),
                container.clone(),
                *tensor_parallel_size,
                extra_args.clone(),
                *no_gpu,
                *no_replace,
            ),
            ModelCommands::Stop { container } => stop_cmd(path, container.as_deref()),
            ModelCommands::Train { name, .. } => train_cmd(path, name),
            ModelCommands::Prune { dry_run, yes } => prune_cmd(path, *dry_run, *yes),
            ModelCommands::Test {
                endpoint,
                prompt,
                max_tokens,
            } => test_cmd(endpoint, prompt, *max_tokens),
        }
    }

    pub async fn execute_async(&self, path: &str) -> Result<()> {
        match self {
            ModelCommands::Served { json } => served_cmd(path, *json).await,
            _ => self.execute(path),
        }
    }
}

fn list_models_cmd(path: &str, json_output: bool, show_adapters: bool) -> Result<()> {
    let models = list_models(path)?;
    if json_output {
        let mut payload = serde_json::json!({ "models": models });
        if show_adapters {
            let adapters = list_adapters(path)?;
            payload["adapters"] = serde_json::json!(adapters);
        }
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    if models.is_empty() {
        println!(
            "No AI model datums found. Create *.model.toml or *.ai_model.toml files in _b00t_."
        );
    } else {
        println!("📦 AI Model Datums:\n");
        for record in models {
            print_record_summary(&record);
        }
        println!("\nUse 'b00t-cli model info <name>' for details.");
    }

    if show_adapters {
        let adapters = list_adapters(path)?;
        if adapters.is_empty() {
            println!("\n📦 No LoRA adapters found in .b00t/models/adapters/.");
        } else {
            println!("\n📦 LoRA Adapters:\n");
            for (name, base_model) in &adapters {
                println!("   {:<22} base: {}", name, base_model);
            }
        }
    }

    Ok(())
}

/// Scan `.b00t/models/adapters/` for `.adapter.tomllmd` files and return (name, base_model) pairs.
fn list_adapters(path: &str) -> Result<Vec<(String, String)>> {
    let adapters_dir = std::path::Path::new(".b00t/models/adapters");
    if !adapters_dir.exists() {
        return Ok(Vec::new());
    }

    let mut adapters = Vec::new();
    let entries =
        std::fs::read_dir(adapters_dir).map_err(|e| anyhow!("reading adapters dir: {}", e))?;

    for entry in entries {
        let entry = entry.map_err(|e| anyhow!("entry: {}", e))?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            // Each adapter subdirectory should have an .adapter.tomllmd alongside it
            // in the _b00t_ datums directory
            let name = entry.file_name().to_string_lossy().to_string();
            let datum_path = std::path::Path::new(path).join(format!("{}.adapter.tomllmd", name));
            let base_model = if datum_path.exists() {
                let content = std::fs::read_to_string(&datum_path)?;
                // Try to extract base_model from the TOML
                let parsed: Result<toml::Value, _> = content.parse();
                match parsed {
                    Ok(tbl) => tbl
                        .get("b00t")
                        .and_then(|b| b.get("base_model"))
                        .and_then(|v| v.as_str().map(String::from))
                        .unwrap_or_else(|| "?".to_string()),
                    Err(_) => "?".to_string(),
                }
            } else {
                // Try reading adapter metadata from the directory itself
                let meta_path = entry.path().join("adapter_metadata.toml");
                if meta_path.exists() {
                    let content = std::fs::read_to_string(&meta_path)?;
                    let parsed: Result<toml::Value, _> = content.parse();
                    match parsed {
                        Ok(tbl) => tbl
                            .get("base_model")
                            .and_then(|v| v.as_str().map(String::from))
                            .unwrap_or_else(|| "?".to_string()),
                        Err(_) => "?".to_string(),
                    }
                } else {
                    "?".to_string()
                }
            };
            adapters.push((name, base_model));
        }
    }

    adapters.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(adapters)
}

async fn served_cmd(path: &str, json_output: bool) -> Result<()> {
    let endpoints = list_served_models(path).await?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&endpoints)?);
        return Ok(());
    }

    if endpoints.is_empty() {
        println!("No local inference models are currently reachable.");
        return Ok(());
    }

    println!("📡 Local Inference Models:\n");
    for endpoint in &endpoints {
        print_served_endpoint(endpoint);
    }
    Ok(())
}

fn info_cmd(path: &str, name: Option<&str>, json_output: bool) -> Result<()> {
    let record = describe_model(path, name)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&record)?);
        return Ok(());
    }

    print_record_details(&record);
    Ok(())
}

fn env_cmd(path: &str, name: Option<&str>, plain: bool, json_output: bool) -> Result<()> {
    let env = export_model_env(path, name)?;
    if json_output {
        let payload: Vec<_> = env
            .iter()
            .map(|(k, v)| json!({ "key": k, "value": v }))
            .collect();
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }

    for (key, value) in &env {
        // 🦨 Fix: iterate by reference, env already borrowed above
        if plain {
            println!("{}={}", key, value);
        } else {
            println!("export {}={}", key, shell_quote(value));
        }
    }
    Ok(())
}

fn download_cmd(path: &str, name: &str, force: bool, activate: bool) -> Result<()> {
    let result = download_model(path, name, force, activate)?;
    print_download_result(&result);
    Ok(())
}

fn remove_cmd(path: &str, name: &str, yes: bool) -> Result<()> {
    if !yes {
        return Err(anyhow!(
            "Removal requires confirmation. Re-run with --yes to delete cached weights."
        ));
    }

    match remove_model(path, name)? {
        Some(dir) => println!("🗑️  Removed cache {}", dir),
        None => println!(
            "Cache directory for '{}' not found; nothing to remove.",
            name
        ),
    }
    Ok(())
}

fn serve_cmd(
    path: &str,
    name: Option<&str>,
    adapter: &Option<String>,
    port: Option<u16>,
    dtype: Option<String>,
    image: Option<String>,
    container: Option<String>,
    tensor_parallel_size: u32,
    extra_args: Vec<String>,
    no_gpu: bool,
    no_replace: bool,
) -> Result<()> {
    let mut options = ServeOptions::default();
    options.port = port;
    options.dtype = dtype;
    options.image = image;
    options.container_name = container;
    options.tensor_parallel_size = Some(tensor_parallel_size);
    options.extra_args = extra_args;
    if no_gpu {
        options.gpus = false;
    }
    if no_replace {
        options.force_replace = false;
    }

    // Load adapter datum if specified
    if let Some(adapter_name) = adapter {
        let adapter_datum_path =
            std::path::PathBuf::from(path).join(format!("{adapter_name}.adapter.tomllmd"));
        if adapter_datum_path.exists() {
            eprintln!(
                "   Using LoRA adapter: {adapter_name} ({})",
                adapter_datum_path.display()
            );
        } else {
            eprintln!("   ⚠️  Adapter datum not found: {adapter_name}");
            eprintln!("   Expected at: {}", adapter_datum_path.display());
        }
    }

    let result = serve_model(path, name, options)?;
    println!(
        "🚀 vLLM container '{}' listening on http://localhost:{}",
        result.container, result.port
    );
    Ok(())
}

fn print_record_summary(record: &ModelRecord) {
    let marker = if record.active { "⭐" } else { " " };
    let status = if record.installed {
        "✅ cached"
    } else {
        "⬜ pending"
    };
    println!(
        "{} {:<22} {:<10} {:<10} {}",
        marker, record.name, record.provider, status, record.hint
    );
}

fn print_record_details(record: &ModelRecord) {
    println!("📌 Model: {}", record.name);
    println!("    Hint: {}", record.hint);
    println!("Provider: {}", record.provider);
    println!("Size: {}", record.size);
    println!("Capabilities: {}", record.capabilities.join(", "));
    println!("Installed: {}", if record.installed { "yes" } else { "no" });
    if let Some(repo) = &record.repo {
        println!("HF Repo: {}", repo);
    }
    if let Some(dir) = &record.cache_dir {
        println!("Cache Dir: {}", dir);
    }
    if let Some(path) = &record.container_path {
        println!("Container Mount: {}", path);
    }
    if let Some(dtype) = &record.dtype {
        println!("DType: {}", dtype);
    }
    if let Some(rpm) = record.rpm_limit {
        println!("RPM Limit: {}", rpm);
    }
    if let Some(ctx) = record.context_window {
        println!("Context Window: {}", ctx);
    }
    if !record.aliases.is_empty() {
        println!("Aliases: {}", record.aliases.join(", "));
    }
    println!("Active: {}", if record.active { "yes" } else { "no" });
}

fn print_download_result(result: &ModelOperation) {
    if result.downloaded {
        println!("✅ Cached model {}", result.name);
    } else {
        println!(
            "✅ Model {} already cached at {}",
            result.name,
            result.cache_dir.as_deref().unwrap_or("<unknown>")
        );
    }
    if result.activated {
        println!("⭐ {} marked as active", result.name);
    }
}

fn stop_cmd(path: &str, container: Option<&str>) -> Result<()> {
    stop_model(path, container)?;
    if let Some(name) = container {
        println!("🛑 Stopped container {}", name);
    } else {
        println!("🛑 Stopped active model container");
    }
    Ok(())
}

fn print_served_endpoint(endpoint: &ServedEndpointRecord) {
    let mut details = Vec::new();
    if let Some(gpu) = &endpoint.gpu {
        details.push(format!("gpu={}", gpu));
    }
    if let Some(port) = endpoint.port {
        details.push(format!("port={}", port));
    }
    if !endpoint.source_models.is_empty() {
        details.push(format!("datum={}", endpoint.source_models.join(",")));
    }

    if details.is_empty() {
        println!("{}", endpoint.base_url);
    } else {
        println!("{}  ({})", endpoint.base_url, details.join(", "));
    }

    for model in &endpoint.models {
        println!("  - {}", model.id);
    }
    println!();
}

fn shell_quote(value: &str) -> String {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || "-_./:@".contains(c))
    {
        value.to_string()
    } else {
        let mut quoted = String::from("'");
        for ch in value.chars() {
            if ch == '\'' {
                quoted.push_str("'\"'\"'");
            } else {
                quoted.push(ch);
            }
        }
        quoted.push('\'');
        quoted
    }
}

// ─── Train command ─────────────────────────────────────────────────────────

fn train_cmd(path: &str, name: &str) -> Result<()> {
    let fsl_path = std::path::PathBuf::from(".b00t/fsl").join(format!("{name}-examples.jsonl"));
    let adapter_dir = std::path::PathBuf::from(".b00t/models/adapters").join(name);
    let training_datum_path =
        std::path::PathBuf::from(path).join(format!("{name}.training.tomllmd"));

    // 1. Load or scaffold training datum
    let training_config = if training_datum_path.exists() {
        let raw = std::fs::read_to_string(&training_datum_path)?;
        toml::from_str::<serde_json::Value>(&raw)?
    } else {
        serde_json::json!({
            "b00t": {
                "name": name,
                "type": "training",
                "base_model": "Qwen/Qwen2.5-1.5B",
                "adapter_output": adapter_dir.to_string_lossy()
            },
            "b00t.training": {
                "hyperparameters": {
                    "learning_rate": 1e-4_f64,
                    "num_epochs": 3,
                    "batch_size": 1,
                    "max_seq_length": 512,
                    "lora_r": 8,
                    "lora_alpha": 16,
                    "lora_dropout": 0.05
                }
            }
        })
    };

    // 2. Count FSL examples
    let example_count = if fsl_path.exists() {
        let content = std::fs::read_to_string(&fsl_path)?;
        content.lines().filter(|l| !l.is_empty()).count()
    } else {
        0
    };

    eprintln!("📦 Training datum:  {name}");
    eprintln!(
        "   Base model:      {}",
        training_config["b00t"]["base_model"]
            .as_str()
            .unwrap_or("?")
    );
    eprintln!(
        "   FSL examples:    {example_count} ({})",
        fsl_path.display()
    );
    eprintln!("   Adapter output:  {}", adapter_dir.display());
    eprintln!(
        "   Epochs:          {}",
        training_config["b00t"]["training"]["hyperparameters"]["num_epochs"]
            .as_u64()
            .unwrap_or(3)
    );
    eprintln!(
        "   Learning rate:   {}",
        training_config["b00t"]["training"]["hyperparameters"]["learning_rate"]
            .as_f64()
            .unwrap_or(1e-4)
    );
    eprintln!(
        "   LoRA rank:      {}",
        training_config["b00t"]["training"]["hyperparameters"]["lora_r"]
            .as_u64()
            .unwrap_or(8)
    );

    if example_count == 0 {
        anyhow::bail!(
            "no FSL examples found at {}\nRun `b00t validate --jsonl <records.jsonl>` to generate examples first.",
            fsl_path.display()
        );
    }

    // 3. Create adapter directory
    std::fs::create_dir_all(&adapter_dir)?;

    // 4. Spawn candle LoRA training
    // 🤓 Uses the now-fixed candle feature (candle-core 0.10+)
    // The candle training binary is at b00t-cli/src/bin/candle-train.rs
    // and is built with `cargo build --features candle --bin candle-train`
    #[cfg(feature = "candle")]
    {
        eprintln!("   Starting candle LoRA training...");
        let status = std::process::Command::new("cargo")
            .args([
                "run",
                "--features",
                "candle",
                "--bin",
                "candle-train",
                "--",
                &format!("--training-datum={}", training_datum_path.display()),
                &format!("--fsl={}", fsl_path.display()),
                &format!("--adapter-out={}", adapter_dir.display()),
            ])
            .status()?;
        if !status.success() {
            anyhow::bail!("candle training failed (exit={:?})", status.code());
        }
    }

    #[cfg(not(feature = "candle"))]
    {
        eprintln!("   ⚠️  candle feature not enabled — training stub only");
        eprintln!("   Rebuild: cargo build --features candle");
        eprintln!("   Adapter directory created at: {}", adapter_dir.display());
        eprintln!(
            "   Training datum config: {}",
            training_datum_path.display()
        );
    }

    // 5. Save adapter datum
    let adapter_datum = format!(
        r#"# 🤖 AUTO-GENERATED from `b00t model train {name}`
[b00t]
name = "{name}"
type = "adapter"
base_model = "{base}"
adapter_path = "{out}"
training_datum = "{datum}"
"#,
        name = name,
        base = training_config["b00t"]["base_model"]
            .as_str()
            .unwrap_or("?"),
        out = adapter_dir.display(),
        datum = training_datum_path.display()
    );
    let adapter_datum_path = std::path::PathBuf::from(path).join(format!("{name}.adapter.tomllmd"));
    std::fs::write(&adapter_datum_path, &adapter_datum)?;

    eprintln!("   ✅ Adapter datum: {}", adapter_datum_path.display());
    Ok(())
}

// ─── Prune command ────────────────────────────────────────────────────────

fn prune_cmd(path: &str, dry_run: bool, yes: bool) -> Result<()> {
    let models_dir = std::path::Path::new(".b00t/models");
    let adapters_dir = models_dir.join("adapters");
    let datum_dir = std::path::PathBuf::from(path);

    let mut to_remove: Vec<std::path::PathBuf> = Vec::new();

    // Scan adapter directories not referenced by any .adapter.tomllmd datum
    if adapters_dir.exists() {
        for entry in std::fs::read_dir(&adapters_dir)? {
            let entry = entry?;
            let dir_name = entry.file_name().to_string_lossy().to_string();
            let adapter_datum = datum_dir.join(format!("{}.adapter.tomllmd", dir_name));
            if !adapter_datum.exists() {
                to_remove.push(entry.path());
            }
        }
    }

    // Scan model weight directories not referenced by any .model.toml or .ai_model.toml datum
    if models_dir.exists() {
        for entry in std::fs::read_dir(models_dir)? {
            let entry = entry?;
            let fname = entry.file_name();
            if fname == "adapters" {
                continue;
            }
            let dir_name = fname.to_string_lossy().to_string();
            let model_datum = datum_dir.join(format!("{}.model.toml", dir_name));
            let ai_model_datum = datum_dir.join(format!("{}.ai_model.toml", dir_name));
            if !model_datum.exists() && !ai_model_datum.exists() {
                to_remove.push(entry.path());
            }
        }
    }

    if to_remove.is_empty() {
        println!("Nothing to prune — all model dirs are referenced by datums.");
        return Ok(());
    }

    println!("The following would be removed:");
    for dir in &to_remove {
        println!("  {}", dir.display());
    }

    if !dry_run && yes {
        for dir in &to_remove {
            std::fs::remove_dir_all(dir)?;
            println!("  Removed: {}", dir.display());
        }
    } else if !dry_run {
        println!("Use --yes to confirm removal.");
    }

    Ok(())
}

// ─── Test command ──────────────────────────────────────────────────────────

fn test_cmd(endpoint: &str, prompt: &str, max_tokens: u32) -> Result<()> {
    let url = format!("{}/v1/chat/completions", endpoint.trim_end_matches('/'));

    let body = serde_json::json!({
        "model": "default",
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": max_tokens,
    });

    let start = std::time::Instant::now();

    let output = std::process::Command::new("curl")
        .args([
            "-s",
            "-X",
            "POST",
            &url,
            "-H",
            "Content-Type: application/json",
            "-d",
            &body.to_string(),
        ])
        .output()
        .map_err(|e| anyhow!("failed to execute curl: {}", e))?;

    let elapsed = start.elapsed();

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("curl exited with {:?}: {}", output.status.code(), stderr);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let resp: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| anyhow!("failed to parse response: {}", e))?;

    let model_name = resp["model"].as_str().unwrap_or("unknown");
    let content = resp["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or("");
    let total_tokens = resp["usage"]["total_tokens"].as_u64().unwrap_or(0);
    let prompt_tokens = resp["usage"]["prompt_tokens"].as_u64().unwrap_or(0);
    let completion_tokens = resp["usage"]["completion_tokens"].as_u64().unwrap_or(0);

    println!("🤖 Model:       {}", model_name);
    println!("   Response:    {}", content.trim());
    println!("   Prompt:      {} tokens", prompt_tokens);
    println!("   Completion:  {} tokens", completion_tokens);
    println!("   Total:       {} tokens", total_tokens);
    println!("   Time:        {:.2}s", elapsed.as_secs_f64());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_served_command_parses_json_flag() {
        let command = ModelCommands::try_parse_from(["model", "served", "--json"]).unwrap();
        match command {
            ModelCommands::Served { json } => assert!(json),
            other => panic!("unexpected command: {:?}", std::mem::discriminant(&other)),
        }
    }
}
