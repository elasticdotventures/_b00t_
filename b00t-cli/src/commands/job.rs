// Job command - orchestrator-agnostic job deployment
// Translates abstract Job datums to orchestrator-specific formats

use anyhow::{Context, Result};
use clap::Subcommand;
use std::path::Path;

use crate::orchestrator::{create_adapter, detect_orchestrator, Orchestrator};
use crate::JobDatum;

#[derive(Debug, Subcommand)]
pub enum JobCommands {
    #[clap(about = "Deploy a job using auto-detected or specified orchestrator")]
    Deploy {
        /// Job name (from .job.toml file)
        name: String,

        /// Specific orchestrator to use (kubernetes, docker-compose, nomad, direct)
        #[clap(short, long)]
        orchestrator: Option<String>,

        /// Path to _b00t_ directory
        #[clap(short, long, default_value = "_b00t_")]
        path: String,

        /// Dry run - generate manifests without deploying
        #[clap(long)]
        dry_run: bool,
    },

    #[clap(about = "Generate manifests for a job without deploying")]
    ToManifest {
        /// Job name
        name: String,

        /// Orchestrator (kubernetes, docker-compose, nomad, direct)
        #[clap(short, long)]
        orchestrator: Option<String>,

        /// Path to _b00t_ directory
        #[clap(short, long, default_value = "_b00t_")]
        path: String,

        /// Output file (default: stdout)
        #[clap(short, long)]
        output: Option<String>,
    },

    #[clap(about = "Show job datum information")]
    Show {
        /// Job name
        name: String,

        /// Path to _b00t_ directory
        #[clap(short, long, default_value = "_b00t_")]
        path: String,
    },
}

pub fn handle_job_command(cmd: JobCommands) -> Result<()> {
    match cmd {
        JobCommands::Deploy {
            name,
            orchestrator,
            path,
            dry_run,
        } => job_deploy(&name, orchestrator, &path, dry_run),

        JobCommands::ToManifest {
            name,
            orchestrator,
            path,
            output,
        } => job_to_manifest(&name, orchestrator, &path, output.as_deref()),

        JobCommands::Show { name, path } => job_show(&name, &path),
    }
}

fn job_deploy(
    name: &str,
    orchestrator: Option<String>,
    path: &str,
    dry_run: bool,
) -> Result<()> {
    println!("🚀 Deploying job: {}", name);

    // Load job datum
    let job = load_job_datum(name, path)?;

    // Determine orchestrator
    let orch = if let Some(o) = orchestrator {
        o.parse::<Orchestrator>()
            .context(format!("Invalid orchestrator: {}", o))?
    } else {
        let detected = detect_orchestrator()?;
        println!("🔍 Auto-detected orchestrator: {}", detected);
        detected
    };

    // Create adapter
    let adapter = create_adapter(orch)?;

    // Check availability
    if !adapter.is_available() {
        anyhow::bail!(
            "Orchestrator {} is not available. Check installation and configuration.",
            adapter.name()
        );
    }

    println!("📋 Using orchestrator: {}", adapter.name());

    // Translate job to orchestrator-specific format
    let output = adapter
        .translate_job(&job)
        .context("Failed to translate job")?;

    // Print warnings
    if !output.metadata.warnings.is_empty() {
        println!("\n⚠️  Warnings:");
        for warning in &output.metadata.warnings {
            println!("  - {}", warning);
        }
    }

    // Print manifests
    println!("\n📄 Generated Manifests:");
    for (idx, manifest) in output.manifests.iter().enumerate() {
        println!("--- Manifest {} ---", idx + 1);
        println!("{}", manifest);
    }

    if dry_run {
        println!("\n🔍 Dry run - not deploying");
        return Ok(());
    }

    // Print MCP commands
    if !output.mcp_commands.is_empty() {
        println!("\n🔧 MCP Commands to execute:");
        for cmd in &output.mcp_commands {
            println!("  Server: {}", cmd.server);
            println!("  Tool: {}", cmd.tool);
            println!("  Args: {}", serde_json::to_string_pretty(&cmd.arguments)?);
        }
    }

    println!("\n✅ Job manifests generated successfully!");
    println!("💡 To deploy, integrate with {}-mcp server or use orchestrator CLI", orch);

    Ok(())
}

fn job_to_manifest(
    name: &str,
    orchestrator: Option<String>,
    path: &str,
    output_file: Option<&str>,
) -> Result<()> {
    // Load job datum
    let job = load_job_datum(name, path)?;

    // Determine orchestrator
    let orch = if let Some(o) = orchestrator {
        o.parse::<Orchestrator>()?
    } else {
        detect_orchestrator()?
    };

    // Create adapter and translate
    let adapter = create_adapter(orch)?;
    let output = adapter.translate_job(&job)?;

    // Combine manifests
    let combined = output.manifests.join("\n---\n");

    // Write to file or stdout
    if let Some(file) = output_file {
        std::fs::write(file, &combined)?;
        println!("✅ Wrote manifest to: {}", file);
    } else {
        println!("{}", combined);
    }

    Ok(())
}

fn job_show(name: &str, path: &str) -> Result<()> {
    let job = load_job_datum(name, path)?;

    println!("📋 Job: {}", job.datum.name);
    if let Some(hint) = &job.datum.hint {
        println!("💡 Hint: {}", hint);
    }

    if let Some(image) = &job.datum.image {
        println!("\n🐳 Container:");
        println!("  Image: {}", image);
        if let Some(cmd) = &job.datum.command {
            println!("  Command: {:?}", cmd);
        }
        if let Some(args) = &job.datum.args {
            println!("  Args: {:?}", args);
        }
    }

    if let Some(env) = &job.datum.env {
        println!("\n🔧 Environment Variables:");
        for (key, value) in env {
            println!("  {} = {}", key, value);
        }
    }

    if let Some(orch) = &job.datum.orchestration {
        println!("\n🎯 Orchestration:");
        if let Some(stacks) = &orch.requires_stacks {
            println!("  Requires Stacks: {:?}", stacks);
        }
        if let Some(queue) = &orch.queue_name {
            println!("  Queue: {}", queue);
        }
        if let Some(sched) = &orch.schedule_type {
            println!("  Schedule Type: {}", sched);
        }

        if let Some(budget) = &orch.budget_constraint {
            println!("\n💰 Budget:");
            println!("  Daily Limit: ${:.2}", budget.daily_limit);
            println!("  Cost Per Job: ${:.2}", budget.cost_per_job);
            println!("  On Exceeded: {}", budget.on_budget_exceeded);
        }

        if let Some(gpu) = &orch.gpu_requirements {
            println!("\n🎮 GPU Requirements:");
            if let Some(count) = gpu.count {
                println!("  Count: {}", count);
            }
            if let Some(gpu_type) = &gpu.gpu_type {
                println!("  Type: {}", gpu_type);
            }
            if let Some(shared) = gpu.shared {
                println!("  Shared: {}", shared);
            }
        }
    }

    Ok(())
}

fn load_job_datum(name: &str, path: &str) -> Result<JobDatum> {
    let job_path = Path::new(path).join(format!("{}.job.toml", name));

    if !job_path.exists() {
        anyhow::bail!(
            "Job datum not found: {}. Expected file: {}",
            name,
            job_path.display()
        );
    }

    JobDatum::from_config(name, path)
        .context(format!("Failed to load job datum: {}", name))
}
