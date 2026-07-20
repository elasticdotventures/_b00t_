//! Job executor using apalis for background task processing
//!
//! Executes `.job.toml` workflow definitions using apalis-sqlite backend
//! with git checkpoints for state persistence.

use anyhow::{Context, Result};
use apalis::prelude::*;
use apalis_sqlite::SqlitePool;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::task::JoinSet;

use crate::commands::provider::{BatchJobSpec, ComputeProvider, JobHandle, get_provider};
use crate::datum_job::{JobDatum, JobStep, JobTask};

/// Bash command task for apalis execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BashTaskJob {
    /// Job name for identification
    pub job_name: String,

    /// Step name for checkpoint tracking
    pub step_name: String,

    /// Bash command to execute
    pub command: String,

    /// Working directory (optional)
    pub cwd: Option<String>,

    /// Timeout in milliseconds (optional)
    pub timeout_ms: Option<u64>,

    /// Environment variables
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,

    /// Checkpoint tag name (optional)
    pub checkpoint: Option<String>,

    /// Project root for git operations
    pub project_root: PathBuf,
}

/// Job executor managing apalis workers and SQLite backend
pub struct JobExecutor {
    /// SQLite connection pool
    pool: SqlitePool,

    /// Project root directory
    project_root: PathBuf,

    /// Database file path
    db_path: PathBuf,
}

impl JobExecutor {
    /// Create new job executor with SQLite backend
    pub async fn new(project_root: &Path) -> Result<Self> {
        let jobs_dir = project_root.join(".b00t/jobs");
        std::fs::create_dir_all(&jobs_dir).context("Failed to create .b00t/jobs directory")?;

        let db_path = jobs_dir.join("apalis.db");

        // Use sqlite: URL with absolute path
        let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

        println!("🥾 Initializing job executor with SQLite backend");
        println!("   Database: {}", db_path.display());

        let pool = SqlitePool::connect(&db_url)
            .await
            .context("Failed to connect to SQLite database")?;

        // Setup apalis tables
        // Note: setup() is called on first use of SqliteStorage
        // No explicit setup needed for apalis-sqlite

        Ok(JobExecutor {
            pool,
            project_root: project_root.to_path_buf(),
            db_path,
        })
    }

    /// Load and execute a job from .job.toml file
    pub async fn run_job(&self, job_name: &str) -> Result<()> {
        println!("🚀 Loading job: {}", job_name);

        // Load job datum (from_config expects _b00t_ directory path)
        let b00t_path = self.project_root.join("_b00t_");
        let job_datum = JobDatum::from_config(job_name, b00t_path.to_str().unwrap())
            .context("Failed to load job configuration")?;

        // Validate job
        job_datum.validate().context("Job validation failed")?;

        let job_config = job_datum.job_config()?;

        println!("📋 Job: {}", job_config.description);
        println!("   Mode: {}", job_config.config.mode);
        println!("   Steps: {}", job_config.steps.len());

        // Note: SqliteStorage not used in MVP (synchronous execution)
        // Future enhancement: Use SqliteStorage with worker pool for async execution

        // "sequential" and "parallel" are the two modes this executor supports;
        // "dag" is accepted by `JobDatum::validate()`/`execution_order()` but a
        // real dependency-graph scheduler is explicitly out of scope here (see
        // datum_job.rs docs) — keep bailing on it rather than silently
        // downgrading to sequential.
        match job_config.config.mode.as_str() {
            "sequential" => {
                self.run_steps_sequential(&job_datum.datum.name, &job_config.steps)
                    .await?;
            }
            "parallel" => {
                self.run_steps_parallel(&job_datum.datum.name, &job_config.steps)
                    .await?;
            }
            other => {
                anyhow::bail!(
                    "job_executor supports 'sequential' and 'parallel' modes (got: {})",
                    other
                );
            }
        }

        println!("\n🎉 Job completed successfully");
        Ok(())
    }

    /// Run all steps one at a time, in definition order (today's original behavior).
    async fn run_steps_sequential(&self, job_name: &str, steps: &[JobStep]) -> Result<()> {
        for (idx, step) in steps.iter().enumerate() {
            println!("\n📍 Step {}/{}: {}", idx + 1, steps.len(), step.name);
            println!("   {}", step.description);

            run_step(&self.project_root, job_name, step)
                .await
                .with_context(|| format!("Step '{}' failed", step.name))?;

            println!("   ✅ Step completed");
        }
        Ok(())
    }

    /// Run all steps concurrently in one "wave" and wait for all of them —
    /// not a DAG scheduler (no `depends_on` resolution between steps here,
    /// that's explicitly out of scope; see datum_job.rs's docs).
    async fn run_steps_parallel(&self, job_name: &str, steps: &[JobStep]) -> Result<()> {
        println!(
            "\n⚡ Running {} step(s) concurrently (parallel mode)",
            steps.len()
        );

        let mut set = JoinSet::new();
        for step in steps.iter().cloned() {
            let project_root = self.project_root.clone();
            let job_name = job_name.to_string();
            set.spawn(async move {
                let step_name = step.name.clone();
                let result = run_step(&project_root, &job_name, &step)
                    .await
                    .with_context(|| format!("Step '{}' failed", step_name));
                (step_name, result)
            });
        }

        let mut failures = Vec::new();
        while let Some(joined) = set.join_next().await {
            let (step_name, result) = joined.context("parallel step task panicked")?;
            match result {
                Ok(()) => println!("   ✅ Step '{}' completed", step_name),
                Err(e) => {
                    eprintln!("   ❌ Step '{}' failed: {:#}", step_name, e);
                    failures.push(format!("{}: {:#}", step_name, e));
                }
            }
        }

        if !failures.is_empty() {
            anyhow::bail!(
                "parallel job had {} failing step(s): {}",
                failures.len(),
                failures.join("; ")
            );
        }
        Ok(())
    }

    /// Execute bash task (synchronous for MVP)
    async fn execute_bash_task(&self, task: BashTaskJob) -> Result<()> {
        use duct::cmd;

        println!("   🔧 Executing: {}", task.command);

        // Parse command (simple split for MVP)
        let parts: Vec<&str> = task.command.split_whitespace().collect();
        if parts.is_empty() {
            anyhow::bail!("Empty command");
        }

        let program = parts[0];
        let args = &parts[1..];

        // Build command
        let mut command = cmd(program, args);

        // Set working directory
        if let Some(cwd) = &task.cwd {
            let cwd_path = if Path::new(cwd).is_absolute() {
                PathBuf::from(cwd)
            } else {
                task.project_root.join(cwd)
            };
            command = command.dir(cwd_path);
        } else {
            command = command.dir(&task.project_root);
        }

        // Set environment variables
        for (key, value) in &task.env {
            command = command.env(key, value);
        }

        // Execute command
        let output = command
            .stdout_to_stderr()
            .run()
            .with_context(|| format!("Failed to execute: {}", task.command))?;

        if !output.status.success() {
            anyhow::bail!("Command failed with exit code: {:?}", output.status.code());
        }

        // Create git checkpoint if specified
        if let Some(checkpoint_name) = &task.checkpoint {
            println!("   📌 Creating checkpoint: {}", checkpoint_name);
            self.create_checkpoint(&task.job_name, &task.step_name, checkpoint_name)?;
        }

        Ok(())
    }

    /// Create git checkpoint after successful step
    fn create_checkpoint(
        &self,
        job_name: &str,
        step_name: &str,
        checkpoint_name: &str,
    ) -> Result<()> {
        create_checkpoint_at(&self.project_root, job_name, step_name, checkpoint_name)
    }
}

/// Create git checkpoint after a successful step. Free function (not an
/// `&self` method) so it can be called from steps spawned onto the tokio
/// runtime in `JobExecutor::run_steps_parallel`, which need `'static`-owned
/// data rather than a borrow of `JobExecutor`.
fn create_checkpoint_at(
    project_root: &Path,
    job_name: &str,
    step_name: &str,
    checkpoint_name: &str,
) -> Result<()> {
    use duct::cmd;

    let tag_name = format!("checkpoint/{}/{}", job_name, checkpoint_name);
    let message = format!("Checkpoint: {} - {}", step_name, checkpoint_name);

    // Create git tag
    let result = cmd!("git", "tag", "-a", &tag_name, "-m", &message)
        .dir(project_root)
        .stdout_to_stderr()
        .run();

    match result {
        Ok(_) => {
            println!("   ✅ Checkpoint created: {}", tag_name);
            Ok(())
        }
        Err(e) => {
            // Don't fail job if checkpoint creation fails
            eprintln!("   ⚠️  Failed to create checkpoint: {}", e);
            Ok(())
        }
    }
}

fn resolve_cwd(project_root: &Path, cwd: Option<&str>) -> PathBuf {
    match cwd {
        Some(c) if Path::new(c).is_absolute() => PathBuf::from(c),
        Some(c) => project_root.join(c),
        None => project_root.to_path_buf(),
    }
}

/// Shell out to `program args...` with `cwd`/`env` applied, no checkpoint
/// handling — the caller (`execute_bash_step`/`execute_python_step`) creates
/// a checkpoint once, after its *last* command succeeds.
///
/// `duct`'s `.run()` is a *blocking* synchronous call. Running it directly
/// inside an `async fn` body would, on tokio's default current-thread
/// runtime, serialize tasks spawned by `run_steps_parallel` onto the single
/// worker thread instead of letting them overlap — silently defeating
/// "parallel" mode. Offloading to `spawn_blocking` runs it on tokio's
/// dedicated blocking-thread pool so concurrently-spawned steps genuinely
/// run at the same time, regardless of runtime flavor.
async fn run_shell(
    project_root: &Path,
    cwd: Option<&str>,
    program: &str,
    args: &[String],
    env: &std::collections::HashMap<String, String>,
) -> Result<()> {
    use duct::cmd;

    println!("   🔧 Executing: {} {}", program, args.join(" "));

    let dir = resolve_cwd(project_root, cwd);
    let program = program.to_string();
    let args = args.to_vec();
    let env = env.clone();

    tokio::task::spawn_blocking(move || -> Result<()> {
        let mut command = cmd(&program, &args).dir(dir);
        for (key, value) in &env {
            command = command.env(key, value);
        }

        let output = command
            .stdout_to_stderr()
            .run()
            .with_context(|| format!("Failed to execute: {} {}", program, args.join(" ")))?;

        if !output.status.success() {
            anyhow::bail!("Command failed with exit code: {:?}", output.status.code());
        }

        Ok(())
    })
    .await
    .context("shell task panicked")??;

    Ok(())
}

fn checkpoint_if_needed(project_root: &Path, job_name: &str, step: &JobStep) -> Result<()> {
    if let Some(checkpoint_name) = &step.checkpoint {
        println!("   📌 Creating checkpoint: {}", checkpoint_name);
        create_checkpoint_at(project_root, job_name, &step.name, checkpoint_name)?;
    }
    Ok(())
}

/// Execute `JobTask::Bash` — identical shell-out behavior to the pre-existing
/// (now-unused) `JobExecutor::execute_bash_task`/`BashTaskJob` path.
async fn execute_bash_step(
    project_root: &Path,
    job_name: &str,
    step: &JobStep,
    command: &str,
    cwd: Option<&str>,
    env: &std::collections::HashMap<String, String>,
) -> Result<()> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        anyhow::bail!("Empty command");
    }
    let program = parts[0];
    let args: Vec<String> = parts[1..].iter().map(|s| s.to_string()).collect();

    run_shell(project_root, cwd, program, &args, env).await?;
    checkpoint_if_needed(project_root, job_name, step)
}

/// Execute `JobTask::Python` — pip-installs each `requirements` entry (one
/// `pip install <req>` per entry, no venv/poetry/uv management), then runs
/// `python3 <script>`. Same shell-out mechanics as `execute_bash_step`.
async fn execute_python_step(
    project_root: &Path,
    job_name: &str,
    step: &JobStep,
    script: &str,
    requirements: &[String],
    cwd: Option<&str>,
    env: &std::collections::HashMap<String, String>,
) -> Result<()> {
    for requirement in requirements {
        run_shell(
            project_root,
            cwd,
            "pip",
            &["install".to_string(), requirement.clone()],
            env,
        )
        .await
        .with_context(|| format!("pip install {} failed", requirement))?;
    }

    run_shell(project_root, cwd, "python3", &[script.to_string()], env).await?;

    checkpoint_if_needed(project_root, job_name, step)
}

/// Poll interval / attempt budget for provider job-status polling. Kept
/// small so tests (which use a fake `ComputeProvider` reporting a terminal
/// status immediately) run fast offline. Real cloud jobs (RunPod/HF) take
/// much longer than this budget to finish — this MVP bridge does not yet
/// distinguish "still running, check back later" from "gave up"; it simply
/// errors out after the budget is exhausted.
const PROVIDER_POLL_INTERVAL: Duration = Duration::from_millis(200);
const PROVIDER_MAX_POLLS: u32 = 50;

/// True if `status` looks like a finished (success or failure) job.
///
/// There is no single terminal-status convention shared across providers —
/// `LocalProvider::job_status` returns a raw podman/docker `State.Status`
/// (`running`, `exited`, `dead`, ...; see `commands/provider.rs`),
/// `RunpodProvider::job_status` returns `"pod=<id> status=<DesiredStatus>"`,
/// and `HfProvider::job_status` returns raw `hf jobs inspect` output. This
/// does a permissive substring match against markers observed across all
/// three, rather than inventing a shared enum none of them actually emit.
fn is_terminal_status(status: &str) -> bool {
    let s = status.to_lowercase();
    [
        "exited",
        "dead",
        "complete",
        "completed",
        "succeeded",
        "success",
        "failed",
        "failure",
        "error",
        "cancelled",
        "canceled",
        "terminated",
        "stopped",
    ]
    .iter()
    .any(|marker| s.contains(marker))
}

/// Subset of `is_terminal_status` markers that indicate the job did NOT
/// succeed. Note "exited" alone is deliberately excluded — none of the
/// providers expose an exit code via `job_status`, so a bare "exited" is
/// treated as success rather than guessed at.
fn is_failure_status(status: &str) -> bool {
    let s = status.to_lowercase();
    ["dead", "fail", "error", "cancel", "terminated"]
        .iter()
        .any(|marker| s.contains(marker))
}

async fn poll_until_terminal(provider: &dyn ComputeProvider, handle: &JobHandle) -> Result<String> {
    for _ in 0..PROVIDER_MAX_POLLS {
        let status = provider.job_status(handle).await?;
        if is_terminal_status(&status) {
            return Ok(status);
        }
        tokio::time::sleep(PROVIDER_POLL_INTERVAL).await;
    }
    anyhow::bail!(
        "job {} (provider={}) did not reach a terminal status after {} polls",
        handle.id,
        handle.provider,
        PROVIDER_MAX_POLLS
    );
}

/// Submit `spec` via `provider` and poll until terminal. Split out from
/// `execute_provider_step` (which resolves the provider by name through
/// `get_provider`) so tests can inject a fake `ComputeProvider` directly —
/// `commands/provider.rs` has no pre-existing test-double pattern for the
/// trait (checked its own test module: none use one), so this is a minimal
/// one scoped to this bridge.
async fn dispatch_batch_job(provider: &dyn ComputeProvider, spec: &BatchJobSpec) -> Result<()> {
    let handle = provider
        .submit_batch_job(spec)
        .await
        .context("submit_batch_job failed")?;

    let final_status = poll_until_terminal(provider, &handle).await?;

    if is_failure_status(&final_status) {
        anyhow::bail!(
            "job {} (provider={}) ended in a failure status: {}",
            handle.id,
            handle.provider,
            final_status
        );
    }

    Ok(())
}

/// Dispatch `step` through `ComputeProvider::submit_batch_job` instead of a
/// local shell-out. Requires `step.batch` to be set (the `BatchJobSpec` to
/// submit) — `step.task` is ignored in this path.
async fn execute_provider_step(backend: &str, step: &JobStep) -> Result<()> {
    let spec = step.batch.as_ref().ok_or_else(|| {
        anyhow::anyhow!(
            "step '{}' declares backend '{}' but has no [b00t.job.steps.batch] spec",
            step.name,
            backend
        )
    })?;

    let provider =
        get_provider(backend).with_context(|| format!("resolving backend '{}'", backend))?;

    dispatch_batch_job(provider.as_ref(), spec).await
}

/// Run a single step: dispatch via `ComputeProvider` if `step.backend` is
/// set, otherwise fall back to today's local shell-out behavior (`Bash`/
/// `Python`; other `JobTask` variants remain unsupported, same MVP
/// limitation as before this change).
async fn run_step(project_root: &Path, job_name: &str, step: &JobStep) -> Result<()> {
    if let Some(backend) = &step.backend {
        return execute_provider_step(backend, step).await;
    }

    match &step.task {
        JobTask::Bash {
            command, cwd, env, ..
        } => execute_bash_step(project_root, job_name, step, command, cwd.as_deref(), env).await,
        JobTask::Python {
            script,
            requirements,
            cwd,
            env,
            ..
        } => {
            execute_python_step(
                project_root,
                job_name,
                step,
                script,
                requirements,
                cwd.as_deref(),
                env,
            )
            .await
        }
        _ => {
            println!("   ⚠️  Skipping unsupported task type (MVP limitation)");
            Ok(())
        }
    }
}

/// Async bash task handler for apalis (future use with worker pool)
#[allow(dead_code)]
async fn bash_task_handler(
    task: BashTaskJob,
    _ctx: WorkerContext,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use duct::cmd;

    // Parse command
    let parts: Vec<&str> = task.command.split_whitespace().collect();
    if parts.is_empty() {
        return Err("Empty command".into());
    }

    let program = parts[0];
    let args = &parts[1..];

    // Build and execute command
    let mut command = cmd(program, args);

    if let Some(cwd) = &task.cwd {
        command = command.dir(cwd);
    }

    for (key, value) in &task.env {
        command = command.env(key, value);
    }

    let output = command.stdout_to_stderr().run()?;

    if !output.status.success() {
        return Err(format!("Command failed: {:?}", output.status.code()).into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_job_executor_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let executor = JobExecutor::new(temp_dir.path()).await;
        assert!(executor.is_ok());

        // Verify database file created
        let db_path = temp_dir.path().join(".b00t/jobs/apalis.db");
        assert!(db_path.exists());
    }

    #[test]
    fn test_bash_task_job_serialization() {
        let task = BashTaskJob {
            job_name: "test".to_string(),
            step_name: "step1".to_string(),
            command: "echo hello".to_string(),
            cwd: None,
            timeout_ms: None,
            env: std::collections::HashMap::new(),
            checkpoint: Some("test-checkpoint".to_string()),
            project_root: PathBuf::from("/tmp"),
        };

        let json = serde_json::to_string(&task).unwrap();
        let deserialized: BashTaskJob = serde_json::from_str(&json).unwrap();

        assert_eq!(task.job_name, deserialized.job_name);
        assert_eq!(task.command, deserialized.command);
    }

    #[tokio::test]
    async fn test_simple_job_execution() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();

        // Create _b00t_ directory
        let b00t_dir = project_root.join("_b00t_");
        fs::create_dir_all(&b00t_dir).unwrap();

        // Create a simple test job file
        let job_toml = r#"
[b00t]
name = "test-simple"
type = "job"

[b00t.job]
description = "Simple test job"
tags = ["test"]

[b00t.job.config]
mode = "sequential"
checkpoint_mode = "off"

[[b00t.job.steps]]
name = "echo-test"
description = "Echo test message"

[b00t.job.steps.task]
type = "bash"
command = "echo 'Test passed'"
"#;

        let job_file_path = b00t_dir.join("test-simple.job.toml");
        fs::write(&job_file_path, job_toml).unwrap();

        // Verify file was created
        assert!(
            job_file_path.exists(),
            "Job file should exist at: {}",
            job_file_path.display()
        );
        println!("Created job file at: {}", job_file_path.display());

        // Initialize git repo (required for checkpoints)
        std::process::Command::new("git")
            .args(&["init"])
            .current_dir(project_root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(&["config", "user.name", "Test User"])
            .current_dir(project_root)
            .output()
            .unwrap();

        std::process::Command::new("git")
            .args(&["config", "user.email", "test@example.com"])
            .current_dir(project_root)
            .output()
            .unwrap();

        // Create and run job executor
        let executor = JobExecutor::new(project_root).await.unwrap();

        // Debug: Show paths being used
        println!("Project root: {}", project_root.display());
        println!("_b00t_ path: {}", b00t_dir.display());

        let result = executor.run_job("test-simple").await;

        // Verify job executed successfully
        assert!(result.is_ok(), "Job execution failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_python_task_executes() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();

        let b00t_dir = project_root.join("_b00t_");
        fs::create_dir_all(&b00t_dir).unwrap();

        // Trivial script, empty requirements — keeps this offline/fast, no
        // pip install network call.
        let script_path = project_root.join("hello.py");
        fs::write(&script_path, "print('hello from python task')\n").unwrap();

        let job_toml = format!(
            r#"
[b00t]
name = "test-python"
type = "job"

[b00t.job]
description = "Python test job"
tags = ["test"]

[b00t.job.config]
mode = "sequential"
checkpoint_mode = "off"

[[b00t.job.steps]]
name = "run-python"
description = "Run trivial python script"

[b00t.job.steps.task]
type = "python"
script = "{}"
requirements = []
"#,
            script_path.display()
        );
        fs::write(b00t_dir.join("test-python.job.toml"), job_toml).unwrap();

        let executor = JobExecutor::new(project_root).await.unwrap();
        let result = executor.run_job("test-python").await;

        assert!(result.is_ok(), "Python job failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn test_parallel_mode_runs_concurrently() {
        let temp_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();

        let b00t_dir = project_root.join("_b00t_");
        fs::create_dir_all(&b00t_dir).unwrap();

        // Three 300ms sleeps: sequential would take >=900ms; if steps truly
        // overlap this should finish in well under that bound.
        let job_toml = r#"
[b00t]
name = "test-parallel"
type = "job"

[b00t.job]
description = "Parallel test job"
tags = ["test"]

[b00t.job.config]
mode = "parallel"
checkpoint_mode = "off"

[[b00t.job.steps]]
name = "sleep-1"
description = "sleep step 1"

[b00t.job.steps.task]
type = "bash"
command = "sleep 0.3"

[[b00t.job.steps]]
name = "sleep-2"
description = "sleep step 2"

[b00t.job.steps.task]
type = "bash"
command = "sleep 0.3"

[[b00t.job.steps]]
name = "sleep-3"
description = "sleep step 3"

[b00t.job.steps.task]
type = "bash"
command = "sleep 0.3"
"#;
        fs::write(b00t_dir.join("test-parallel.job.toml"), job_toml).unwrap();

        let executor = JobExecutor::new(project_root).await.unwrap();

        let start = std::time::Instant::now();
        let result = executor.run_job("test-parallel").await;
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "Parallel job failed: {:?}", result.err());
        assert!(
            elapsed < std::time::Duration::from_millis(750),
            "parallel steps did not appear to overlap (took {:?}, expected well under 900ms)",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_sequential_mode_still_serializes() {
        // Guards backward compatibility: the same 3x300ms-sleep job under
        // `mode = "sequential"` must NOT benefit from the parallel-mode
        // speedup — proving the default/existing path is unchanged.
        let temp_dir = tempfile::tempdir().unwrap();
        let project_root = temp_dir.path();

        let b00t_dir = project_root.join("_b00t_");
        fs::create_dir_all(&b00t_dir).unwrap();

        let job_toml = r#"
[b00t]
name = "test-sequential-timing"
type = "job"

[b00t.job]
description = "Sequential timing test job"
tags = ["test"]

[b00t.job.config]
mode = "sequential"
checkpoint_mode = "off"

[[b00t.job.steps]]
name = "sleep-1"
description = "sleep step 1"

[b00t.job.steps.task]
type = "bash"
command = "sleep 0.2"

[[b00t.job.steps]]
name = "sleep-2"
description = "sleep step 2"

[b00t.job.steps.task]
type = "bash"
command = "sleep 0.2"
"#;
        fs::write(b00t_dir.join("test-sequential-timing.job.toml"), job_toml).unwrap();

        let executor = JobExecutor::new(project_root).await.unwrap();

        let start = std::time::Instant::now();
        let result = executor.run_job("test-sequential-timing").await;
        let elapsed = start.elapsed();

        assert!(result.is_ok(), "Sequential job failed: {:?}", result.err());
        assert!(
            elapsed >= std::time::Duration::from_millis(380),
            "sequential steps appear to have overlapped (took {:?}, expected >=400ms)",
            elapsed
        );
    }
}

// ============================================================================
// Provider-dispatch bridge tests
//
// `commands/provider.rs` has no pre-existing test-double pattern for
// `ComputeProvider` (checked its own `#[cfg(test)] mod batch_job_tests`:
// only pure-function argv-builder tests, no mock provider) — `FakeProvider`
// here is a minimal one scoped to this bridge, exercising `dispatch_batch_job`
// / `execute_provider_step` without needing real RunPod/HF/podman access.
// ============================================================================
#[cfg(test)]
mod provider_bridge_tests {
    use super::*;
    use crate::commands::provider::{EndpointConfig, EndpointHandle, TrainingJobSpec};
    use async_trait::async_trait;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    /// Returns a scripted sequence of `job_status` results, one per poll —
    /// lets tests assert both immediate-terminal and poll-until-terminal
    /// behavior without a real provider or real wall-clock job duration.
    struct FakeProvider {
        statuses: Mutex<VecDeque<String>>,
    }

    impl FakeProvider {
        fn with_statuses(statuses: &[&str]) -> Self {
            Self {
                statuses: Mutex::new(statuses.iter().map(|s| s.to_string()).collect()),
            }
        }
    }

    #[async_trait]
    impl ComputeProvider for FakeProvider {
        fn name(&self) -> &str {
            "fake"
        }

        async fn deploy_inference_endpoint(&self, _cfg: &EndpointConfig) -> Result<EndpointHandle> {
            anyhow::bail!("FakeProvider does not support endpoints")
        }

        async fn endpoint_status(&self, _id: &str) -> Result<EndpointHandle> {
            anyhow::bail!("FakeProvider does not support endpoints")
        }

        async fn teardown_endpoint(&self, _id: &str) -> Result<()> {
            anyhow::bail!("FakeProvider does not support endpoints")
        }

        async fn list_endpoints(&self) -> Result<Vec<EndpointHandle>> {
            Ok(vec![])
        }

        async fn submit_training_job(&self, _spec: &TrainingJobSpec) -> Result<JobHandle> {
            anyhow::bail!("FakeProvider does not support training jobs")
        }

        async fn submit_batch_job(&self, _spec: &BatchJobSpec) -> Result<JobHandle> {
            Ok(JobHandle {
                id: "fake-job-1".to_string(),
                provider: "fake".to_string(),
            })
        }

        async fn job_status(&self, _handle: &JobHandle) -> Result<String> {
            let mut statuses = self.statuses.lock().unwrap();
            Ok(statuses.pop_front().unwrap_or_else(|| "exited".to_string()))
        }

        async fn cancel_job(&self, _handle: &JobHandle) -> Result<()> {
            Ok(())
        }

        async fn list_jobs(&self) -> Result<Vec<JobHandle>> {
            Ok(vec![])
        }
    }

    fn sample_spec() -> BatchJobSpec {
        BatchJobSpec {
            image: "fake:latest".to_string(),
            config_path: "/tmp/fake-request.json".to_string(),
            env: std::collections::HashMap::new(),
            flavor: "local-gpu".to_string(),
            timeout_hours: 1.0,
        }
    }

    #[tokio::test]
    async fn dispatch_batch_job_succeeds_on_immediate_terminal_status() {
        let provider = FakeProvider::with_statuses(&["exited"]);
        let result = dispatch_batch_job(&provider, &sample_spec()).await;
        assert!(result.is_ok(), "expected success, got: {:?}", result.err());
    }

    #[tokio::test]
    async fn dispatch_batch_job_polls_until_terminal() {
        // Non-terminal statuses first — proves this actually loops rather
        // than trusting the first `job_status` response.
        let provider = FakeProvider::with_statuses(&["running", "running", "exited"]);
        let result = dispatch_batch_job(&provider, &sample_spec()).await;
        assert!(result.is_ok(), "expected success, got: {:?}", result.err());
    }

    #[tokio::test]
    async fn dispatch_batch_job_fails_on_failure_status() {
        let provider = FakeProvider::with_statuses(&["failed"]);
        let result = dispatch_batch_job(&provider, &sample_spec()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn execute_provider_step_requires_batch_spec() {
        let step = JobStep {
            name: "no-batch".to_string(),
            description: "missing batch spec".to_string(),
            checkpoint: None,
            depends_on: vec![],
            task: JobTask::Bash {
                command: "echo hi".to_string(),
                cwd: None,
                timeout_ms: None,
                env: std::collections::HashMap::new(),
            },
            condition: None,
            artifacts: None,
            cognitive_tier: None,
            output_contract: None,
            backend: Some("local".to_string()),
            batch: None,
        };

        let result = execute_provider_step("local", &step).await;
        let err = result.expect_err("missing batch spec should error");
        assert!(
            err.to_string().contains("no [b00t.job.steps.batch] spec"),
            "unexpected error: {}",
            err
        );
    }

    #[test]
    fn terminal_status_detection() {
        assert!(is_terminal_status("exited"));
        assert!(is_terminal_status("Dead"));
        assert!(is_terminal_status("pod=abc status=COMPLETED"));
        assert!(!is_terminal_status("running"));
        assert!(!is_terminal_status("pending"));
    }

    #[test]
    fn failure_status_detection() {
        assert!(is_failure_status("failed"));
        assert!(is_failure_status("dead"));
        assert!(
            !is_failure_status("exited"),
            "bare 'exited' has no exit code available, treat as success"
        );
        assert!(!is_failure_status("completed"));
    }
}

// ============================================================================
/// Session metadata for git checkpoints
/// Stores session info in git tag messages for resume functionality
// ============================================================================

/// Session metadata to store in git checkpoint tags
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckpointMetadata {
    /// Current session ID
    pub session_id: String,
    /// Current role (from _B00T_ROLE env var)
    pub role: Option<String>,
    /// Number of checkpoints created in this session
    pub checkpoint_count: i64,
    /// List of loaded capabilities with their use counts
    pub capabilities_loaded: Vec<CapabilityInfo>,
}

/// Individual capability info
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CapabilityInfo {
    pub name: String,
    pub use_count: i64,
}

impl JobExecutor {
    /// Create git checkpoint with session metadata
    fn create_checkpoint_with_metadata(
        &self,
        job_name: &str,
        step_name: &str,
        checkpoint_name: &str,
        metadata: &CheckpointMetadata,
    ) -> Result<()> {
        use duct::cmd;

        // Format capabilities as a readable string
        let capabilities_str = if metadata.capabilities_loaded.is_empty() {
            "none".to_string()
        } else {
            metadata
                .capabilities_loaded
                .iter()
                .map(|c| format!("{}:{}", c.name, c.use_count))
                .collect::<Vec<_>>()
                .join(",")
        };

        // Format role (use "none" if not set)
        let role_str = metadata.role.as_deref().unwrap_or("none");

        // Create enriched tag message with session metadata
        let tag_name = format!("checkpoint/{}/{}", job_name, checkpoint_name);
        let message = format!(
            "Checkpoint: {} - {} | session:{} role:{} checkpoint_count:{} capabilities:{}",
            step_name,
            checkpoint_name,
            &metadata.session_id[..8.min(metadata.session_id.len())],
            role_str,
            metadata.checkpoint_count,
            capabilities_str
        );

        // Create git tag
        let result = cmd!("git", "tag", "-a", &tag_name, "-m", &message)
            .dir(&self.project_root)
            .stdout_to_stderr()
            .run();

        match result {
            Ok(_) => {
                println!("   ✅ Checkpoint created: {}", tag_name);
                Ok(())
            }
            Err(e) => {
                // Don't fail job if checkpoint creation fails
                eprintln!("   ⚠️  Failed to create checkpoint: {}", e);
                Ok(())
            }
        }
    }

    /// Resume from a checkpoint - parse tag message and restore session state
    pub fn resume_from_checkpoint(&self, tag_name: &str) -> Result<CheckpointMetadata> {
        use duct::cmd;

        // Get tag message
        let output = cmd!("git", "tag", "-l", "--format=%contents", tag_name)
            .dir(&self.project_root)
            .read()
            .context(format!("Failed to read tag: {}", tag_name))?;

        // Parse metadata from tag message
        // Format: "Checkpoint: step - name | session:xxx role:xxx checkpoint_count:xxx capabilities:xxx"
        let metadata = parse_checkpoint_message(&output)?;

        println!("📂 Resuming from checkpoint: {}", tag_name);
        println!("   Session: {}", metadata.session_id);
        if let Some(ref role) = metadata.role {
            println!("   Role: {}", role);
        }
        println!("   Checkpoint #: {}", metadata.checkpoint_count);
        if !metadata.capabilities_loaded.is_empty() {
            println!(
                "   Capabilities: {}",
                metadata
                    .capabilities_loaded
                    .iter()
                    .map(|c| c.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }

        Ok(metadata)
    }

    /// List all checkpoint tags for a job
    pub fn list_checkpoints(&self, job_name: Option<&str>) -> Result<Vec<CheckpointInfo>> {
        use duct::cmd;

        let pattern = if let Some(name) = job_name {
            format!("checkpoint/{}/*", name)
        } else {
            "checkpoint/*/*".to_string()
        };

        let output = cmd!("git", "tag", "-l", &pattern)
            .dir(&self.project_root)
            .read()
            .context("Failed to list checkpoint tags")?;

        let mut checkpoints = Vec::new();
        for tag in output.lines() {
            if tag.trim().is_empty() {
                continue;
            }

            // Get tag message for metadata
            if let Ok(msg_output) = cmd!("git", "tag", "-l", "--format=%contents", tag)
                .dir(&self.project_root)
                .read()
            {
                if let Ok(metadata) = parse_checkpoint_message(&msg_output) {
                    checkpoints.push(CheckpointInfo {
                        tag_name: tag.to_string(),
                        metadata,
                    });
                }
            }
        }

        Ok(checkpoints)
    }
}

/// Checkpoint info from git tag
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckpointInfo {
    pub tag_name: String,
    pub metadata: CheckpointMetadata,
}

/// Parse checkpoint message to extract metadata
fn parse_checkpoint_message(message: &str) -> Result<CheckpointMetadata> {
    let mut metadata = CheckpointMetadata::default();

    // Extract session_id: after "session:" before " role:" or " checkpoint_count:"
    const SESSION_PREFIX: &str = "session:";
    if let Some(start) = message.find(SESSION_PREFIX) {
        let rest = &message[start + SESSION_PREFIX.len()..];
        let end = rest
            .find(' ')
            .or_else(|| rest.find('|'))
            .unwrap_or(rest.len());
        metadata.session_id = rest[..end].to_string();
    }

    // Extract role: after "role:" before " checkpoint_count:"
    const ROLE_PREFIX: &str = "role:";
    if let Some(start) = message.find(ROLE_PREFIX) {
        let rest = &message[start + ROLE_PREFIX.len()..];
        let end = rest
            .find(' ')
            .or_else(|| rest.find('|'))
            .unwrap_or(rest.len());
        let role = rest[..end].to_string();
        if role != "none" {
            metadata.role = Some(role);
        }
    }

    // Extract checkpoint_count: after "checkpoint_count:" before " capabilities:"
    const CHECKPOINT_COUNT_PREFIX: &str = "checkpoint_count:";
    if let Some(start) = message.find(CHECKPOINT_COUNT_PREFIX) {
        let rest = &message[start + CHECKPOINT_COUNT_PREFIX.len()..];
        let end = rest
            .find(' ')
            .or_else(|| rest.find('|'))
            .unwrap_or(rest.len());
        metadata.checkpoint_count = rest[..end].parse().unwrap_or(0);
    }

    // Extract capabilities: after "capabilities:" to end
    const CAPABILITIES_PREFIX: &str = "capabilities:";
    if let Some(start) = message.find(CAPABILITIES_PREFIX) {
        let rest = &message[start + CAPABILITIES_PREFIX.len()..];
        let capabilities_str = rest.trim();

        if capabilities_str != "none" && !capabilities_str.is_empty() {
            for cap in capabilities_str.split(',') {
                let parts: Vec<&str> = cap.split(':').collect();
                if parts.len() == 2 {
                    metadata.capabilities_loaded.push(CapabilityInfo {
                        name: parts[0].to_string(),
                        use_count: parts[1].parse().unwrap_or(1),
                    });
                } else {
                    metadata.capabilities_loaded.push(CapabilityInfo {
                        name: cap.to_string(),
                        use_count: 1,
                    });
                }
            }
        }
    }

    if metadata.session_id.is_empty() {
        anyhow::bail!("Failed to parse checkpoint message: missing session_id");
    }

    Ok(metadata)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod checkpoint_tests {
    use super::*;

    #[test]
    fn test_parse_checkpoint_message() {
        let message = "Checkpoint: step1 - complete | session:abc123 role:captain checkpoint_count:3 capabilities:git:5,bash:10";

        let metadata = parse_checkpoint_message(message).unwrap();

        assert_eq!(metadata.session_id, "abc123");
        assert_eq!(metadata.role, Some("captain".to_string()));
        assert_eq!(metadata.checkpoint_count, 3);
        assert_eq!(metadata.capabilities_loaded.len(), 2);
        assert_eq!(metadata.capabilities_loaded[0].name, "git");
        assert_eq!(metadata.capabilities_loaded[0].use_count, 5);
        assert_eq!(metadata.capabilities_loaded[1].name, "bash");
        assert_eq!(metadata.capabilities_loaded[1].use_count, 10);
    }

    #[test]
    fn test_parse_checkpoint_message_no_role() {
        let message = "Checkpoint: step1 - complete | session:abc123 role:none checkpoint_count:1 capabilities:none";

        let metadata = parse_checkpoint_message(message).unwrap();

        assert_eq!(metadata.session_id, "abc123");
        assert!(metadata.role.is_none());
        assert_eq!(metadata.checkpoint_count, 1);
        assert!(metadata.capabilities_loaded.is_empty());
    }
}
