//! Job orchestration commands - Workflow execution with checkpoints
//!
//! Provides `b00t job` command for:
//! - Creating job definitions
//! - Planning and reviewing workflows
//! - Executing with git-based checkpoints
//! - Sub-agent spawning for complex tasks
//! - Integration with k0mmander/Dagu/go tasker

use anyhow::{Context, Result};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
pub enum JobCommands {
    #[clap(
        about = "List available jobs",
        long_about = "List all job definitions found in _b00t_/ directory.\n\nExamples:\n  b00t job list\n  b00t job list --json"
    )]
    List {
        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },

    #[clap(
        about = "Show job details and execution plan",
        long_about = "Display job configuration, steps, dependencies, and execution plan.\n\nExamples:\n  b00t job plan build-release\n  b00t job plan example-workflow --dag"
    )]
    Plan {
        #[clap(help = "Job name (without .job.toml extension)")]
        name: String,

        #[clap(long, help = "Show DAG visualization")]
        dag: bool,

        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },

    #[clap(
        about = "Execute job workflow",
        long_about = "Execute job workflow with checkpoint support and sub-agent spawning.\n\nExamples:\n  b00t job run build-release\n  b00t job run example-workflow --from-step lint\n  b00t job run build-release --dry-run\n  b00t job run example-workflow --no-checkpoint"
    )]
    Run {
        #[clap(help = "Job name (without .job.toml extension)")]
        name: String,

        #[clap(long, help = "Start from specific step")]
        from_step: Option<String>,

        #[clap(long, help = "Stop after specific step")]
        to_step: Option<String>,

        #[clap(long, help = "Dry run - show what would be executed")]
        dry_run: bool,

        #[clap(long, help = "Skip checkpoint creation")]
        no_checkpoint: bool,

        #[clap(long, help = "Continue from last checkpoint")]
        resume: bool,

        #[clap(short, long, help = "Environment variables (KEY=VALUE)")]
        env: Vec<String>,
    },

    #[clap(
        about = "Show job execution status",
        long_about = "Show status of currently running or recent jobs.\n\nExamples:\n  b00t job status\n  b00t job status build-release\n  b00t job status --all"
    )]
    Status {
        #[clap(help = "Job name (optional - shows all if omitted)")]
        name: Option<String>,

        #[clap(long, help = "Show all historical runs")]
        all: bool,

        #[clap(long, help = "Output in JSON format")]
        json: bool,
    },

    #[clap(
        about = "Stop running job",
        long_about = "Stop a currently running job execution.\n\nExamples:\n  b00t job stop build-release\n  b00t job stop --all"
    )]
    Stop {
        #[clap(help = "Job name to stop")]
        name: Option<String>,

        #[clap(long, help = "Stop all running jobs")]
        all: bool,
    },

    #[clap(
        about = "List job checkpoints",
        long_about = "List git checkpoints created during job execution.\n\nExamples:\n  b00t job checkpoints\n  b00t job checkpoints build-release"
    )]
    Checkpoints {
        #[clap(help = "Job name (optional - shows all if omitted)")]
        name: Option<String>,
    },

    #[clap(
        about = "Create new job definition",
        long_about = "Create a new job definition from template or interactive wizard.\n\nExamples:\n  b00t job create my-workflow\n  b00t job create my-workflow --template sequential\n  b00t job create my-workflow --from-dagu quick-check.yaml"
    )]
    Create {
        #[clap(help = "Job name")]
        name: String,

        #[clap(long, help = "Template to use (sequential, parallel, dag)")]
        template: Option<String>,

        #[clap(long, help = "Import from Dagu YAML file")]
        from_dagu: Option<String>,
    },
}

impl JobCommands {
    pub async fn execute_async(&self, path: &str) -> Result<()> {
        match self {
            JobCommands::List { json } => list_jobs(path, *json).await,
            JobCommands::Plan { name, dag, json } => plan_job(path, name, *dag, *json).await,
            JobCommands::Run {
                name,
                from_step,
                to_step,
                dry_run,
                no_checkpoint,
                resume,
                env,
            } => {
                run_job(
                    path,
                    name,
                    from_step.as_deref(),
                    to_step.as_deref(),
                    *dry_run,
                    *no_checkpoint,
                    *resume,
                    env,
                )
                .await
            }
            JobCommands::Status { name, all, json } => {
                status_job(path, name.as_deref(), *all, *json).await
            }
            JobCommands::Stop { name, all } => stop_job(path, name.as_deref(), *all).await,
            JobCommands::Checkpoints { name } => checkpoints_job(path, name.as_deref()).await,
            JobCommands::Create {
                name,
                template,
                from_dagu,
            } => create_job(path, name, template.as_deref(), from_dagu.as_deref()).await,
        }
    }
}

/// List all available jobs
async fn list_jobs(path: &str, json: bool) -> Result<()> {
    use crate::datum_job::JobDatum;

    let jobs = find_job_datums(path)?;

    if json {
        let job_list: Vec<_> = jobs
            .iter()
            .map(|name| {
                serde_json::json!({
                    "name": name,
                    "path": format!("{}/_b00t_/{}.job.toml", path, name)
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&job_list)?);
    } else {
        println!("📋 Available jobs:\n");
        for job_name in jobs {
            let datum_path = format!("{}.job.toml", job_name);
            match JobDatum::from_config(&datum_path, path) {
                Ok(datum) => {
                    if let Ok(config) = datum.job_config() {
                        println!("  • {} - {}", job_name, config.description);
                        println!("    Steps: {}", config.steps.len());
                        println!("    Mode: {}", config.config.mode);
                    } else {
                        println!("  • {} (invalid config)", job_name);
                    }
                }
                Err(_) => {
                    println!("  • {} (not found)", job_name);
                }
            }
            println!();
        }
    }

    Ok(())
}

/// Show job execution plan
async fn plan_job(path: &str, name: &str, show_dag: bool, json: bool) -> Result<()> {
    use crate::datum_job::JobDatum;

    let datum_path = format!("{}.job.toml", name);
    let datum =
        JobDatum::from_config(&datum_path, path).context(format!("Job '{}' not found", name))?;

    datum.validate()?;

    let config = datum.job_config()?;
    let execution_order = datum.execution_order()?;

    if json {
        let plan = serde_json::json!({
            "name": name,
            "description": config.description,
            "mode": config.config.mode,
            "execution_order": execution_order,
            "steps": config.steps,
            "checkpoints_enabled": config.config.checkpoint_mode != "off",
        });
        println!("{}", serde_json::to_string_pretty(&plan)?);
        return Ok(());
    }

    // Text output
    println!("🎯 Job: {}", name);
    println!("📝 Description: {}", config.description);
    println!("⚙️  Mode: {}", config.config.mode);
    println!("📍 Checkpoints: {}", config.config.checkpoint_mode);
    println!(
        "🤖 Sub-agents: {}",
        if config.config.use_subagents {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!("\n📋 Execution Plan ({} steps):\n", execution_order.len());

    for (idx, step_name) in execution_order.iter().enumerate() {
        let step = config
            .steps
            .iter()
            .find(|s| &s.name == step_name)
            .ok_or_else(|| anyhow::anyhow!("Step '{}' not found in execution plan", step_name))?;

        println!("{}. {} - {}", idx + 1, step.name, step.description);

        if !step.depends_on.is_empty() {
            println!("   Dependencies: {}", step.depends_on.join(", "));
        }

        if let Some(checkpoint) = &step.checkpoint {
            println!("   📍 Checkpoint: {}", checkpoint);
        }

        // Show task type
        match &step.task {
            crate::datum_job::JobTask::Bash { command, .. } => {
                println!("   Type: bash");
                println!("   Command: {}", command);
            }
            crate::datum_job::JobTask::Agent {
                agent_type, prompt, ..
            } => {
                println!("   Type: agent ({})", agent_type);
                println!("   Prompt: {}", prompt.lines().next().unwrap_or("<empty>"));
            }
            crate::datum_job::JobTask::K0mmander { .. } => {
                println!("   Type: k0mmander");
            }
            crate::datum_job::JobTask::Datum { datum, .. } => {
                println!("   Type: datum ({})", datum);
            }
            crate::datum_job::JobTask::Mcp { server, tool, .. } => {
                println!("   Type: mcp ({}/{})", server, tool);
            }
            crate::datum_job::JobTask::Dagu { dag, .. } => {
                println!("   Type: dagu ({})", dag);
            }
        }

        println!();
    }

    if show_dag && config.config.mode == "dag" {
        println!("📊 DAG Visualization:\n");
        print_dag(&config.steps)?;
    }

    Ok(())
}

/// Print DAG visualization
fn print_dag(steps: &[crate::datum_job::JobStep]) -> Result<()> {
    // Simple ASCII DAG visualization
    for step in steps {
        if step.depends_on.is_empty() {
            println!("  [{}]", step.name);
        } else {
            for dep in &step.depends_on {
                println!("  [{}] -> [{}]", dep, step.name);
            }
        }
    }
    Ok(())
}

/// Execute job workflow
async fn run_job(
    path: &str,
    name: &str,
    from_step: Option<&str>,
    to_step: Option<&str>,
    dry_run: bool,
    no_checkpoint: bool,
    resume: bool,
    env_vars: &[String],
) -> Result<()> {
    use crate::datum_job::JobDatum;
    use crate::job_state::{JobState, JobStatus, StepStatus};

    println!("🚀 Starting job: {}", name);

    let datum_path = format!("{}.job.toml", name);
    let datum =
        JobDatum::from_config(&datum_path, path).context(format!("Job '{}' not found", name))?;

    datum.validate()?;

    let config = datum.job_config()?;
    let mut execution_order = datum.execution_order()?;

    // Create or resume job state
    let mut job_state = if resume {
        match JobState::load_latest(path, name) {
            Ok(state) => {
                println!("📂 Resuming from previous run ({})", state.run_id);
                println!("   Started: {}", state.started_at);
                println!("   Status: {:?}", state.status);
                state
            }
            Err(_) => {
                println!("⚠️  No previous run found, starting fresh");
                JobState::new(
                    name.to_string(),
                    config.config.mode.clone(),
                    execution_order.len(),
                )
            }
        }
    } else {
        JobState::new(
            name.to_string(),
            config.config.mode.clone(),
            execution_order.len(),
        )
    };

    // Save initial state
    if !dry_run {
        job_state.save(path)?;
    }

    // Filter by from_step/to_step
    if let Some(from) = from_step {
        if let Some(idx) = execution_order.iter().position(|s| s == from) {
            execution_order = execution_order[idx..].to_vec();
        } else {
            anyhow::bail!("Step '{}' not found", from);
        }
    }

    if let Some(to) = to_step {
        if let Some(idx) = execution_order.iter().position(|s| s == to) {
            execution_order = execution_order[..=idx].to_vec();
        } else {
            anyhow::bail!("Step '{}' not found", to);
        }
    }

    // Parse environment variables
    let mut env_map = config.env.clone();
    for env_var in env_vars {
        if let Some((key, value)) = env_var.split_once('=') {
            env_map.insert(key.to_string(), value.to_string());
        }
    }

    println!("📋 Executing {} steps", execution_order.len());

    if dry_run {
        println!("\n🔍 DRY RUN - No changes will be made\n");
    }

    for (idx, step_name) in execution_order.iter().enumerate() {
        let step = config
            .steps
            .iter()
            .find(|s| &s.name == step_name)
            .ok_or_else(|| anyhow::anyhow!("Step '{}' not found", step_name))?;

        println!("\n[{}/{}] {}", idx + 1, execution_order.len(), step.name);
        println!("   {}", step.description);

        if dry_run {
            println!("   [DRY RUN] Would execute: {:?}", step.task);
            continue;
        }

        // Skip if resuming and step already completed
        if resume {
            if let Some(step_state) = job_state.steps.get(step_name) {
                if step_state.status == StepStatus::Completed {
                    println!("   ⏭️  Skipping (already completed)");
                    continue;
                }
            }
        }

        // Update state: step starting
        job_state.start_step(step_name.clone());
        job_state.status = JobStatus::Running;
        job_state.save(path)?;

        // Execute step
        let step_start = std::time::Instant::now();
        let result = execute_step(path, step, &env_map).await;
        let step_duration = step_start.elapsed();

        match result {
            Ok(_) => {
                println!("   ✅ Success ({}s)", step_duration.as_secs());

                // Update state: step completed
                job_state.complete_step(step_name);
                job_state.save(path)?;

                // Create checkpoint if configured
                if !no_checkpoint
                    && config.config.checkpoint_mode != "off"
                    && (config.config.checkpoint_after_each_step || step.checkpoint.is_some())
                {
                    if let Some(checkpoint_name) = &step.checkpoint {
                        create_checkpoint(
                            path,
                            name,
                            checkpoint_name,
                            config.config.create_git_tag,
                        )
                        .await?;

                        // Record checkpoint in state
                        let git_tag = if config.config.create_git_tag {
                            Some(format!("job/{}/{}", name, checkpoint_name))
                        } else {
                            None
                        };
                        job_state.add_checkpoint(
                            step_name.clone(),
                            checkpoint_name.clone(),
                            git_tag,
                        );
                        job_state.save(path)?;
                    }
                }
            }
            Err(e) => {
                println!("   ❌ Failed: {}", e);

                // Update state: step failed
                job_state.fail_step(step_name, e.to_string());
                job_state.status = JobStatus::Failed;
                job_state.error = Some(e.to_string());
                job_state.save(path)?;

                if config.config.continue_on_failure {
                    println!("   ⚠️  Continuing despite failure...");
                    continue;
                }

                if config.config.rollback_on_failure && !config.rollback.is_empty() {
                    println!("\n🔄 Rolling back...");
                    job_state.status = JobStatus::RollingBack;
                    job_state.save(path)?;

                    for rollback_step in &config.rollback {
                        println!("   Executing rollback: {}", rollback_step.name);
                        if let Err(rollback_err) = execute_step(path, rollback_step, &env_map).await
                        {
                            eprintln!(
                                "⚠️  Rollback step '{}' failed: {}",
                                rollback_step.name, rollback_err
                            );
                        }
                    }

                    job_state.status = JobStatus::RolledBack;
                    job_state.save(path)?;
                }

                return Err(e.context(format!("Step '{}' failed", step_name)));
            }
        }
    }

    println!("\n✨ Job completed successfully!");

    // Update final state
    job_state.status = JobStatus::Completed;
    job_state.completed_at = Some(chrono::Utc::now());
    job_state.save(path)?;

    Ok(())
}

/// Execute a single job step
async fn execute_step(
    path: &str,
    step: &crate::datum_job::JobStep,
    env: &std::collections::HashMap<String, String>,
) -> Result<()> {
    use crate::datum_job::JobTask;

    match &step.task {
        JobTask::Bash {
            command,
            cwd,
            timeout_ms,
            env: step_env,
        } => {
            // Merge environment variables
            let mut combined_env = env.clone();
            combined_env.extend(step_env.clone());

            execute_bash(
                command,
                cwd.as_deref().unwrap_or(path),
                &combined_env,
                *timeout_ms,
            )
            .await
        }
        JobTask::Agent {
            agent_type,
            prompt,
            context_files,
            timeout_ms,
        } => execute_agent(agent_type, prompt, context_files, *timeout_ms).await,
        JobTask::K0mmander { script, cwd } => {
            execute_k0mmander(script, cwd.as_deref().unwrap_or(path)).await
        }
        JobTask::Datum { datum, args } => execute_datum(path, datum, args).await,
        JobTask::Mcp {
            server,
            tool,
            params,
        } => execute_mcp(path, server, tool, params).await,
        JobTask::Dagu { dag, params } => execute_dagu(dag, params).await,
    }
}

/// Execute bash command and display output after completion
async fn execute_bash(
    command: &str,
    cwd: &str,
    env: &std::collections::HashMap<String, String>,
    timeout_ms: Option<u64>,
) -> Result<()> {
    use tokio::process::Command;
    use tokio::time::{Duration, timeout};

    let mut cmd = Command::new("bash");
    cmd.arg("-c")
        .arg(command)
        .current_dir(cwd)
        .envs(env)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    let execution = async {
        let output = cmd.output().await?;

        // Display output after command completes
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.is_empty() {
            for line in stdout.lines() {
                println!("   {}", line);
            }
        }

        if !stderr.is_empty() && !output.status.success() {
            for line in stderr.lines() {
                eprintln!("   {}", line);
            }
        }

        if output.status.success() {
            Ok(())
        } else {
            anyhow::bail!("Command failed with exit code: {}", output.status)
        }
    };

    // Apply timeout if specified
    if let Some(ms) = timeout_ms {
        match timeout(Duration::from_millis(ms), execution).await {
            Ok(result) => result,
            Err(_) => anyhow::bail!("Command timed out after {}ms", ms),
        }
    } else {
        execution.await
    }
}

/// Execute sub-agent task via LangChain agent service
async fn execute_agent(
    agent_type: &str,
    prompt: &str,
    context_files: &[String],
    timeout_ms: Option<u64>,
) -> Result<()> {
    use tokio::process::Command;
    use tokio::time::{Duration, timeout};

    println!("   🤖 Executing LangChain agent: {}", agent_type);

    // Build full prompt with context files
    let mut full_prompt = String::new();

    // Load context files if specified
    if !context_files.is_empty() {
        println!("   📄 Loading context files:");
        for file_path in context_files {
            println!("      - {}", file_path);
            match tokio::fs::read_to_string(file_path).await {
                Ok(content) => {
                    full_prompt.push_str(&format!("\n--- Context from {} ---\n", file_path));
                    full_prompt.push_str(&content);
                    full_prompt.push_str("\n--- End context ---\n\n");
                }
                Err(e) => {
                    eprintln!("   ⚠️  Failed to read {}: {}", file_path, e);
                }
            }
        }
    }

    // Add the actual prompt
    full_prompt.push_str(prompt);

    // Build command to execute LangChain agent
    let mut cmd = Command::new("uv");
    cmd.args(&[
        "run",
        "b00t-langchain",
        "test-agent",
        agent_type,
        &full_prompt,
    ]);

    // Set working directory to langchain-agent
    let langchain_dir = std::env::current_dir()
        .context("Failed to get current directory")?
        .join("langchain-agent");

    if langchain_dir.exists() {
        cmd.current_dir(&langchain_dir);
    } else {
        // Try relative path from _b00t_ root
        let alt_path = std::env::current_dir()?
            .parent()
            .context("No parent directory")?
            .join("langchain-agent");
        if alt_path.exists() {
            cmd.current_dir(&alt_path);
        } else {
            anyhow::bail!(
                "LangChain agent directory not found. Expected: {} or {}",
                langchain_dir.display(),
                alt_path.display()
            );
        }
    }

    // Execute with timeout
    let execution = async {
        let output = cmd
            .output()
            .await
            .context("Failed to execute agent command")?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Print output
        if !stdout.is_empty() {
            for line in stdout.lines() {
                println!("   {}", line);
            }
        }

        if !stderr.is_empty() && !output.status.success() {
            for line in stderr.lines() {
                eprintln!("   {}", line);
            }
        }

        if output.status.success() {
            println!("   ✅ Agent completed successfully");
            Ok(())
        } else {
            anyhow::bail!(
                "Agent failed with exit code: {}",
                output.status.code().unwrap_or(-1)
            )
        }
    };

    // Apply timeout if specified
    let timeout_duration = timeout_ms.unwrap_or(300000); // Default 5 minutes
    match timeout(Duration::from_millis(timeout_duration), execution).await {
        Ok(result) => result,
        Err(_) => anyhow::bail!("Agent timed out after {}ms", timeout_duration),
    }
}

/// Execute k0mmander script
async fn execute_k0mmander(_script: &str, _cwd: &str) -> Result<()> {
    // TODO: Integrate with k0mmander when available
    println!("   ⚠️  K0mmander execution not yet implemented");
    Ok(())
}

/// Execute another datum
async fn execute_datum(path: &str, datum: &str, args: &[String]) -> Result<()> {
    use tokio::process::Command;

    // Determine datum type from filename
    let datum_file = if datum.contains('.') {
        datum.to_string()
    } else {
        // Try to find the datum file
        format!("{}.bash.toml", datum) // Default to bash for now
    };

    println!("   Executing datum: {}", datum_file);

    // For bash datums, execute the script
    if datum_file.ends_with(".bash.toml") {
        // Load the datum and execute its script
        let datum_path = format!("{}", datum_file);
        let (config, _) = crate::get_config(path, &datum_path)
            .map_err(|e| anyhow::anyhow!("Failed to load datum: {}", e))?;

        if let Some(script) = config.b00t.script {
            let mut cmd = Command::new("bash");
            cmd.arg("-c").arg(&script).current_dir(path);

            // Add args as environment variables
            for (idx, arg) in args.iter().enumerate() {
                cmd.env(format!("ARG{}", idx), arg);
            }

            let output = cmd.output().await?;

            if output.status.success() {
                if !output.stdout.is_empty() {
                    println!("   {}", String::from_utf8_lossy(&output.stdout));
                }
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("Datum execution failed: {}", stderr)
            }
        } else {
            anyhow::bail!("Datum '{}' has no script to execute", datum)
        }
    } else {
        anyhow::bail!("Unsupported datum type: {}", datum_file)
    }
}

/// Execute MCP tool
async fn execute_mcp(
    path: &str,
    server: &str,
    tool: &str,
    params: &serde_json::Value,
) -> Result<()> {
    use tokio::process::Command;

    // Use b00t-cli mcp execute command
    let params_json = serde_json::to_string(params)?;

    let output = Command::new(std::env::current_exe()?)
        .args(&["--path", path, "mcp", "execute", server, tool, &params_json])
        .output()
        .await?;

    if output.status.success() {
        if !output.stdout.is_empty() {
            println!("   Output: {}", String::from_utf8_lossy(&output.stdout));
        }
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("MCP execution failed: {}", stderr)
    }
}

/// Execute Dagu DAG
async fn execute_dagu(
    _dag: &str,
    _params: &std::collections::HashMap<String, String>,
) -> Result<()> {
    // TODO: Integrate with Dagu
    println!("   ⚠️  Dagu execution not yet implemented");
    Ok(())
}

/// Create git checkpoint
async fn create_checkpoint(
    path: &str,
    job_name: &str,
    checkpoint_name: &str,
    create_tag: bool,
) -> Result<()> {
    use tokio::process::Command;

    let tag_name = format!("job/{}/{}", job_name, checkpoint_name);

    println!("   📍 Creating checkpoint: {}", tag_name);

    // Commit current state
    Command::new("git")
        .args(&["add", "-A"])
        .current_dir(path)
        .output()
        .await?;

    let commit_msg = format!("Job checkpoint: {} - {}", job_name, checkpoint_name);
    Command::new("git")
        .args(&["commit", "-m", &commit_msg, "--allow-empty"])
        .current_dir(path)
        .output()
        .await?;

    // Create tag if requested
    if create_tag {
        Command::new("git")
            .args(&["tag", "-a", &tag_name, "-m", &commit_msg])
            .current_dir(path)
            .output()
            .await?;
        println!("   🏷️  Tagged: {}", tag_name);
    }

    Ok(())
}

/// Show job status
async fn status_job(path: &str, name: Option<&str>, all: bool, json: bool) -> Result<()> {
    use crate::job_state::JobState;
    use std::fs;

    let state_dir = std::path::PathBuf::from(path).join(".b00t").join("jobs");

    if !state_dir.exists() {
        println!("No job state found");
        return Ok(());
    }

    let mut states = Vec::new();

    if let Some(job_name) = name {
        // Load specific job state
        match JobState::load_latest(path, job_name) {
            Ok(state) => states.push(state),
            Err(e) => {
                println!("❌ Failed to load job state for '{}': {}", job_name, e);
                return Ok(());
            }
        }
    } else if all {
        // Load all job states
        for entry in fs::read_dir(&state_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let job_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(state) = JobState::load_latest(path, &job_name) {
                    states.push(state);
                }
            }
        }
    } else {
        println!("Specify --all to see all jobs or provide a job name");
        return Ok(());
    }

    if json {
        let json_output = serde_json::to_string_pretty(&states)?;
        println!("{}", json_output);
    } else {
        for state in states {
            println!("\n📊 Job: {}", state.job_name);
            println!("   Run ID: {}", state.run_id);
            println!("   Status: {:?}", state.status);
            println!("   Started: {}", state.started_at);
            if let Some(completed) = state.completed_at {
                println!("   Completed: {}", completed);
                let duration = completed - state.started_at;
                println!("   Duration: {}s", duration.num_seconds());
            }
            if let Some(current) = &state.current_step {
                println!("   Current step: {}", current);
            }
            if let Some(error) = &state.error {
                println!("   Error: {}", error);
            }

            println!("\n   Steps:");
            for (step_name, step_state) in &state.steps {
                let status_icon = match step_state.status {
                    crate::job_state::StepStatus::Pending => "⏳",
                    crate::job_state::StepStatus::Running => "🏃",
                    crate::job_state::StepStatus::Completed => "✅",
                    crate::job_state::StepStatus::Failed => "❌",
                    crate::job_state::StepStatus::Skipped => "⏭️ ",
                };
                print!("     {} {}", status_icon, step_name);
                if let Some(duration_ms) = step_state.duration_ms {
                    print!(" ({}s)", duration_ms / 1000);
                }
                if let Some(error) = &step_state.error {
                    print!(" - {}", error);
                }
                println!();
            }

            if !state.checkpoints.is_empty() {
                println!("\n   Checkpoints:");
                for checkpoint in &state.checkpoints {
                    println!(
                        "     📍 {} - {} ({})",
                        checkpoint.step_name, checkpoint.checkpoint_name, checkpoint.created_at
                    );
                    if let Some(tag) = &checkpoint.git_tag {
                        println!("        Tag: {}", tag);
                    }
                }
            }
        }
    }

    Ok(())
}

/// Stop running job
async fn stop_job(_path: &str, _name: Option<&str>, _all: bool) -> Result<()> {
    // TODO: Implement job stopping via IPC
    println!("⚠️  Job stop not yet implemented");
    println!("Future: Send stop signal via IPC/Redis");
    Ok(())
}

/// List job checkpoints
async fn checkpoints_job(path: &str, name: Option<&str>) -> Result<()> {
    use tokio::process::Command;

    let pattern = if let Some(job_name) = name {
        format!("job/{}/", job_name)
    } else {
        "job/".to_string()
    };

    println!("📍 Job checkpoints:\n");

    let output = Command::new("git")
        .args(&["tag", "-l", &format!("{}*", pattern)])
        .current_dir(path)
        .output()
        .await?;

    if output.status.success() {
        let tags = String::from_utf8_lossy(&output.stdout);
        for tag in tags.lines() {
            // Get tag info
            let info_output = Command::new("git")
                .args(&["log", "-1", "--format=%ai - %s", tag])
                .current_dir(path)
                .output()
                .await?;

            if info_output.status.success() {
                let info = String::from_utf8_lossy(&info_output.stdout);
                println!("  {} - {}", tag, info.trim());
            }
        }
    }

    Ok(())
}

/// Create new job definition
async fn create_job(
    path: &str,
    name: &str,
    template: Option<&str>,
    from_dagu: Option<&str>,
) -> Result<()> {
    if let Some(dagu_file) = from_dagu {
        println!("🔄 Importing from Dagu: {}", dagu_file);
        // TODO: Parse Dagu YAML and convert to job TOML
        anyhow::bail!("Dagu import not yet implemented");
    }

    let template_name = template.unwrap_or("sequential");
    let job_path = PathBuf::from(path)
        .join("_b00t_")
        .join(format!("{}.job.toml", name));

    if job_path.exists() {
        anyhow::bail!("Job '{}' already exists", name);
    }

    println!("📝 Creating job: {} (template: {})", name, template_name);

    // TODO: Generate from template
    let template_content = generate_job_template(name, template_name)?;

    std::fs::write(&job_path, template_content)?;

    println!("✨ Created: {}", job_path.display());
    println!("\nNext steps:");
    println!("  1. Edit job definition: {}", job_path.display());
    println!("  2. Review plan: b00t job plan {}", name);
    println!("  3. Execute: b00t job run {}", name);

    Ok(())
}

/// Generate job template
fn generate_job_template(name: &str, template: &str) -> Result<String> {
    match template {
        "sequential" => Ok(format!(
            r#"[b00t]
name = "{name}"
type = "job"
hint = "Sequential workflow with checkpoints"

[b00t.job]
description = "TODO: Describe your workflow"
tags = ["workflow"]

[b00t.job.config]
mode = "sequential"
checkpoint_mode = "auto"
checkpoint_after_each_step = true

[[b00t.job.steps]]
name = "step1"
description = "First step"
checkpoint = "step1-complete"

[b00t.job.steps.task]
type = "bash"
command = "echo 'Step 1'"

[[b00t.job.steps]]
name = "step2"
description = "Second step"
checkpoint = "step2-complete"

[b00t.job.steps.task]
type = "bash"
command = "echo 'Step 2'"
"#,
            name = name
        )),
        _ => anyhow::bail!("Unknown template: {}", template),
    }
}

/// Find all job datums
fn find_job_datums(path: &str) -> Result<Vec<String>> {
    use std::fs;

    let b00t_dir = PathBuf::from(path).join("_b00t_");
    let mut jobs = Vec::new();

    if !b00t_dir.exists() {
        return Ok(jobs);
    }

    for entry in fs::read_dir(b00t_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(filename) = path.file_name() {
            if let Some(name) = filename.to_str() {
                if name.ends_with(".job.toml") {
                    jobs.push(name.trim_end_matches(".job.toml").to_string());
                }
            }
        }
    }

    jobs.sort();
    Ok(jobs)
}

// ============================================================================
// Public API for IPC integration
// ============================================================================

/// Public wrapper for run_job (for job_ipc module)
pub async fn run_job_internal(
    path: &str,
    name: &str,
    from_step: Option<&str>,
    to_step: Option<&str>,
    dry_run: bool,
    no_checkpoint: bool,
    resume: bool,
    env_vars: &[String],
) -> Result<()> {
    run_job(
        path,
        name,
        from_step,
        to_step,
        dry_run,
        no_checkpoint,
        resume,
        env_vars,
    )
    .await
}

/// Get job status as JSON string
pub async fn get_job_status_json(path: &str, name: Option<&str>, all: bool) -> Result<String> {
    use crate::job_state::JobState;
    use std::fs;

    let state_dir = std::path::PathBuf::from(path).join(".b00t").join("jobs");

    if !state_dir.exists() {
        return Ok(serde_json::to_string(&serde_json::json!({
            "jobs": []
        }))?);
    }

    let mut states = Vec::new();

    if let Some(job_name) = name {
        match JobState::load_latest(path, job_name) {
            Ok(state) => states.push(state),
            Err(_) => {
                return Ok(serde_json::to_string(&serde_json::json!({
                    "error": format!("Job '{}' not found", job_name)
                }))?);
            }
        }
    } else if all {
        for entry in fs::read_dir(&state_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                let job_name = entry.file_name().to_string_lossy().to_string();
                if let Ok(state) = JobState::load_latest(path, &job_name) {
                    states.push(state);
                }
            }
        }
    }

    Ok(serde_json::to_string(&serde_json::json!({
        "jobs": states
    }))?)
}

/// Stop job (internal version for IPC)
pub async fn stop_job_internal(path: &str, name: Option<&str>, all: bool) -> Result<()> {
    stop_job(path, name, all).await
}

/// Get job plan as JSON string
pub async fn get_job_plan_json(path: &str, name: &str) -> Result<String> {
    use crate::datum_job::JobDatum;

    let datum_path = format!("{}.job.toml", name);
    let datum =
        JobDatum::from_config(&datum_path, path).context(format!("Job '{}' not found", name))?;

    datum.validate()?;

    let config = datum.job_config()?;
    let execution_order = datum.execution_order()?;

    Ok(serde_json::to_string(&serde_json::json!({
        "job_name": name,
        "description": config.description,
        "mode": config.config.mode,
        "execution_order": execution_order,
        "steps": config.steps,
        "checkpoint_mode": config.config.checkpoint_mode,
    }))?)
}

/// List jobs as JSON string
pub async fn list_jobs_json(path: &str) -> Result<String> {
    let jobs = find_job_datums(path)?;

    Ok(serde_json::to_string(&serde_json::json!({
        "jobs": jobs
    }))?)
}
