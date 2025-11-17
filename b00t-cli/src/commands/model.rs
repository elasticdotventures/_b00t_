use anyhow::{Result, anyhow};
use b00t_cli::model_manager::{
    // 🦨 Fix: use b00t_cli:: from binary, model_manager is in lib
    ModelOperation,
    ModelRecord,
    ServeOptions,
    activate_model,
    describe_model,
    download_model,
    export_model_env,
    list_models,
    remove_model,
    serve_model,
    stop_model,
};
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
        alias = "install",
        visible_alias = "install",
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
}

impl ModelCommands {
    pub fn execute(&self, path: &str) -> Result<()> {
        match self {
            ModelCommands::List { json } => list_models_cmd(path, *json),
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
        }
    }
}

fn list_models_cmd(path: &str, json_output: bool) -> Result<()> {
    let models = list_models(path)?;
    if json_output {
        println!("{}", serde_json::to_string_pretty(&models)?);
        return Ok(());
    }

    if models.is_empty() {
        println!("No AI model datums found. Create *.ai_model.toml files in _b00t_.");
        return Ok(());
    }

    println!("📦 AI Model Datums:\n");
    for record in models {
        print_record_summary(&record);
    }
    println!("\nUse 'b00t-cli model info <name>' for details.");
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
